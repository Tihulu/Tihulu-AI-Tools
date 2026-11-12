//! Phase 2 ResourceLedger handoff. This module contains NO runtime lease
//! registry, reference counter, unloading policy or backend cleanup executor.
//!
//! ResourceLedger in coordinator Draft PR #12 is the **only authority** for
//! active references, stop phases and generation of cleanup requests.
//! This crate supplies typed discovery/task references and a narrow adapter
//! port. A production port must delegate one-to-one to core ResourceLedger.

use crate::registry::{ModelId, ModelOwner};

/// Numeric IDs are assigned and de-duplicated by the *coordinator ledger*.
/// They must not be hashed from a model path or re-used after unload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LedgerOwnerRef(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LedgerResourceRef(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AppSessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskSessionId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeInstanceId(pub String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerTask {
    pub app_session: AppSessionId,
    pub task_session: TaskSessionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextAuthority {
    /// Maps to core Provenance::SharedManaged.
    ManagedShared,
    /// Maps to core Provenance::Exclusive(core OwnerId(owner.0)).
    ManagedExclusive(LedgerOwnerRef),
    /// Maps to core Provenance::BorrowedExternal; the provider's runtime
    /// must not be stopped or unloaded by this application.
    BorrowedExternal(ModelOwner),
}

/// Binding is a reference, not a loaded model or a grant to use it.
/// Trusted host/app authorization must precede registration/attachment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRuntimeBinding {
    /// Stable SOURCE identity. Multiple runtime instances can use one model.
    pub model_id: ModelId,
    /// One actual loaded runtime context, supplied by its authorized backend.
    pub runtime_instance: RuntimeInstanceId,
    /// Core ResourceId, unique for the ledger lifetime.
    pub resource: LedgerResourceRef,
    pub authority: ContextAuthority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerResourceKind {
    ModelContext,
    GpuAllocation,
    AudioRamBuffer,
    AiWorker,
    SdrHandle,
    NetworkSession,
    RemoteInferenceLease,
    TemporaryFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LedgerCleanupRequest {
    pub resource: LedgerResourceRef,
    pub kind: LedgerResourceKind,
}

/// Exact semantic port over core ResourceLedger (PR #12).
///
/// Implementors must map:
/// - register_model_context => ledger.register(owner, resource,
///   ResourceKind::ModelContext, provenance), which CREATES the first lease.
/// - attach => ledger.attach(owner, resource), creating one more lease.
/// - release_one => ledger.release_one(owner, resource), which ALONE decides
///   when the last lease generates a CleanupRequest.
/// - stop_owner => ledger.stop_owner(owner), atomically rejecting new attaches
///   and returning cleanup requests for the whole app session.
/// - complete_cleanup => ledger.complete_cleanup(resource, succeeded).
///
/// The implementation MUST NOT maintain a second runtime count, invent an
/// additional last-lease unload event, interpret a missing borrowed resource
/// as permission to kill a provider, or process cleanup requests twice.
/// Feature-gated direct forwarding to core is in core_adapter.rs; it
/// compiles only on integration branches that contain PR #12's core API.
pub trait ResourceLedgerPort {
    type Error;

    fn register_model_context(
        &mut self,
        initial_owner: LedgerOwnerRef,
        binding: &ModelRuntimeBinding,
    ) -> Result<(), Self::Error>;

    fn attach(
        &mut self,
        owner: LedgerOwnerRef,
        resource: LedgerResourceRef,
    ) -> Result<(), Self::Error>;

    fn release_one(
        &mut self,
        owner: LedgerOwnerRef,
        resource: LedgerResourceRef,
    ) -> Result<Option<LedgerCleanupRequest>, Self::Error>;

    fn stop_owner(
        &mut self,
        owner: LedgerOwnerRef,
    ) -> Result<Vec<LedgerCleanupRequest>, Self::Error>;

    fn active_lease_count(&self, resource: LedgerResourceRef) -> Option<usize>;

    fn complete_cleanup(
        &mut self,
        resource: LedgerResourceRef,
        succeeded: bool,
    ) -> Result<(), Self::Error>;
}

/// A client-held single-use receipt; **not** another lease authority.
/// The bool only prevents the *same client receipt* being submitted twice.
/// All active reference accounting and unload decisions belong to the port.
#[derive(Debug)]
pub struct TaskLeaseReceipt {
    pub consumer: ConsumerTask,
    pub model_id: ModelId,
    pub runtime_instance: RuntimeInstanceId,
    pub owner: LedgerOwnerRef,
    pub resource: LedgerResourceRef,
    released: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskRelease {
    AlreadyReleased,
    /// Request is emitted by ResourceLedger, never by the registry.
    Delegated(Option<LedgerCleanupRequest>),
}

impl TaskLeaseReceipt {
    /// Does not invent a cleanup request. A caller must send the returned
    /// request to its owned backend and acknowledge actual cleanup on ledger.
    ///
    /// After app-wide stop, the caller must retire all this owner's receipts
    /// using stop_app_with_receipts: core stop_owner already released them.
    pub fn release<L: ResourceLedgerPort>(
        &mut self,
        ledger: &mut L,
    ) -> Result<TaskRelease, L::Error> {
        if self.released {
            return Ok(TaskRelease::AlreadyReleased);
        }
        let cleanup = ledger.release_one(self.owner, self.resource)?;
        self.released = true;
        Ok(TaskRelease::Delegated(cleanup))
    }
}

/// The existing loaded model is registered ONCE with core; its registering
/// owner receives the initial lease as part of ledger.register (PR #12).
/// Do not call ledger.attach again for that same initial task.
pub fn register_initial_task<L: ResourceLedgerPort>(
    ledger: &mut L,
    binding: &ModelRuntimeBinding,
    owner: LedgerOwnerRef,
    consumer: ConsumerTask,
) -> Result<TaskLeaseReceipt, L::Error> {
    ledger.register_model_context(owner, binding)?;
    Ok(receipt(binding, owner, consumer))
}

/// Every subsequent task contributes exactly one core ledger attachment.
pub fn attach_task<L: ResourceLedgerPort>(
    ledger: &mut L,
    binding: &ModelRuntimeBinding,
    owner: LedgerOwnerRef,
    consumer: ConsumerTask,
) -> Result<TaskLeaseReceipt, L::Error> {
    ledger.attach(owner, binding.resource)?;
    Ok(receipt(binding, owner, consumer))
}

/// Whole-app STOP is solely a core ledger decision. It can also include
/// cleanup for workers, sockets, SDR and RAM tracked by the same ledger.
/// Never filter out non-model requests here.
pub fn stop_app<L: ResourceLedgerPort>(
    ledger: &mut L,
    owner: LedgerOwnerRef,
) -> Result<Vec<LedgerCleanupRequest>, L::Error> {
    ledger.stop_owner(owner)
}

/// Serialized app-wide STOP with caller-owned task receipts.
///
/// Delegates exactly one Stop to the coordinator ledger. On success, only
/// receipts belonging to this owner are retired. No local lease counter or
/// unload decision is introduced, and other consumers' receipts stay live.
/// The caller must serialize task attach/release and this operation around
/// the SAME authoritative ledger; a plain receipt is not a cross-thread lock.
pub fn stop_app_with_receipts<L: ResourceLedgerPort>(
    ledger: &mut L,
    owner: LedgerOwnerRef,
    receipts: &mut [TaskLeaseReceipt],
) -> Result<Vec<LedgerCleanupRequest>, L::Error> {
    let cleanup = ledger.stop_owner(owner)?;
    for receipt in receipts {
        if receipt.owner == owner {
            receipt.released = true;
        }
    }
    Ok(cleanup)
}

fn receipt(
    binding: &ModelRuntimeBinding,
    owner: LedgerOwnerRef,
    consumer: ConsumerTask,
) -> TaskLeaseReceipt {
    TaskLeaseReceipt {
        consumer,
        model_id: binding.model_id.clone(),
        runtime_instance: binding.runtime_instance.clone(),
        owner,
        resource: binding.resource,
        released: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ArtifactObservation, FileFormat, SourceAdapter};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum FakeError {
        UnknownOrStoppedOwner,
        DuplicateResource,
        ResourceUnavailable,
        NotHeld,
        NotPending,
    }

    #[derive(Debug)]
    struct FakeContext {
        authority: ContextAuthority,
        holders: BTreeMap<LedgerOwnerRef, usize>,
        pending: bool,
    }

    /// Synthetic PR #12 semantics only. NOT a second production ledger.
    #[derive(Debug, Default)]
    struct MockResourceLedger {
        owners: BTreeMap<LedgerOwnerRef, bool>,
        contexts: BTreeMap<LedgerResourceRef, FakeContext>,
        used_ids: BTreeSet<LedgerResourceRef>,
    }

    impl MockResourceLedger {
        fn open_owner(&mut self, owner: LedgerOwnerRef) {
            self.owners.insert(owner, true);
        }

        fn ensure_running(&self, owner: LedgerOwnerRef) -> Result<(), FakeError> {
            if self.owners.get(&owner) == Some(&true) {
                Ok(())
            } else {
                Err(FakeError::UnknownOrStoppedOwner)
            }
        }

        fn last_release(&mut self, resource: LedgerResourceRef) -> Option<LedgerCleanupRequest> {
            let item = self.contexts.get_mut(&resource)?;
            if !item.holders.is_empty() || item.pending {
                return None;
            }
            if matches!(&item.authority, ContextAuthority::BorrowedExternal(_)) {
                self.contexts.remove(&resource);
                return None;
            }
            item.pending = true;
            Some(LedgerCleanupRequest {
                resource,
                kind: LedgerResourceKind::ModelContext,
            })
        }
    }

    impl ResourceLedgerPort for MockResourceLedger {
        type Error = FakeError;

        fn register_model_context(
            &mut self,
            initial_owner: LedgerOwnerRef,
            binding: &ModelRuntimeBinding,
        ) -> Result<(), Self::Error> {
            self.ensure_running(initial_owner)?;
            if self.used_ids.contains(&binding.resource) {
                return Err(FakeError::DuplicateResource);
            }
            if matches!(
                &binding.authority,
                ContextAuthority::ManagedExclusive(only) if *only != initial_owner
            ) {
                return Err(FakeError::ResourceUnavailable);
            }
            self.used_ids.insert(binding.resource);
            let mut holders = BTreeMap::new();
            holders.insert(initial_owner, 1);
            self.contexts.insert(
                binding.resource,
                FakeContext {
                    authority: binding.authority.clone(),
                    holders,
                    pending: false,
                },
            );
            Ok(())
        }

        fn attach(
            &mut self,
            owner: LedgerOwnerRef,
            resource: LedgerResourceRef,
        ) -> Result<(), Self::Error> {
            self.ensure_running(owner)?;
            let item = self
                .contexts
                .get_mut(&resource)
                .ok_or(FakeError::ResourceUnavailable)?;
            if item.pending
                || matches!(
                    &item.authority,
                    ContextAuthority::ManagedExclusive(only) if *only != owner
                )
            {
                return Err(FakeError::ResourceUnavailable);
            }
            let count = item.holders.entry(owner).or_insert(0);
            *count += 1;
            Ok(())
        }

        fn release_one(
            &mut self,
            owner: LedgerOwnerRef,
            resource: LedgerResourceRef,
        ) -> Result<Option<LedgerCleanupRequest>, Self::Error> {
            let item = self
                .contexts
                .get_mut(&resource)
                .ok_or(FakeError::ResourceUnavailable)?;
            if item.pending {
                return Err(FakeError::ResourceUnavailable);
            }
            let count = item.holders.get_mut(&owner).ok_or(FakeError::NotHeld)?;
            *count -= 1;
            if *count == 0 {
                item.holders.remove(&owner);
            }
            Ok(self.last_release(resource))
        }

        fn stop_owner(
            &mut self,
            owner: LedgerOwnerRef,
        ) -> Result<Vec<LedgerCleanupRequest>, Self::Error> {
            let running = self
                .owners
                .get_mut(&owner)
                .ok_or(FakeError::UnknownOrStoppedOwner)?;
            if !*running {
                return Ok(Vec::new());
            }
            *running = false;
            let keys: Vec<_> = self.contexts.keys().copied().collect();
            let mut cleanup = Vec::new();
            for key in keys {
                let removed = self
                    .contexts
                    .get_mut(&key)
                    .is_some_and(|item| item.holders.remove(&owner).is_some());
                if removed {
                    if let Some(request) = self.last_release(key) {
                        cleanup.push(request);
                    }
                }
            }
            Ok(cleanup)
        }

        fn active_lease_count(&self, resource: LedgerResourceRef) -> Option<usize> {
            self.contexts
                .get(&resource)
                .map(|item| item.holders.values().sum())
        }

        fn complete_cleanup(
            &mut self,
            resource: LedgerResourceRef,
            succeeded: bool,
        ) -> Result<(), Self::Error> {
            let item = self
                .contexts
                .get_mut(&resource)
                .ok_or(FakeError::ResourceUnavailable)?;
            if !item.pending {
                return Err(FakeError::NotPending);
            }
            if succeeded {
                self.contexts.remove(&resource);
            }
            Ok(())
        }
    }

    fn binding(
        resource: u64,
        file_owner: ModelOwner,
        authority: ContextAuthority,
    ) -> ModelRuntimeBinding {
        let artifact = ArtifactObservation {
            source_path: PathBuf::from("/synthetic/external/model.gguf"),
            canonical_path: PathBuf::from("/synthetic/external/model.gguf"),
            owner: file_owner,
            adapter: SourceAdapter::CustomFolder,
            format_hint: FileFormat::Gguf,
            size_bytes: 16,
        };
        ModelRuntimeBinding {
            model_id: ModelId::from_artifact(&artifact),
            runtime_instance: RuntimeInstanceId(format!("runtime-{resource}")),
            resource: LedgerResourceRef(resource),
            authority,
        }
    }

    fn consumer(app: &str, task: &str) -> ConsumerTask {
        ConsumerTask {
            app_session: AppSessionId(app.to_owned()),
            task_session: TaskSessionId(task.to_owned()),
        }
    }

    #[test]
    fn l03_two_apps_share_model_without_first_app_unload() {
        let mut ledger = MockResourceLedger::default();
        let first = LedgerOwnerRef(1);
        let second = LedgerOwnerRef(2);
        ledger.open_owner(first);
        ledger.open_owner(second);
        let model = binding(101, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        let initial =
            register_initial_task(&mut ledger, &model, first, consumer("media", "asr")).unwrap();
        let another = attach_task(&mut ledger, &model, second, consumer("text", "asr")).unwrap();
        assert_eq!(initial.model_id, another.model_id);
        assert_eq!(ledger.active_lease_count(model.resource), Some(2));
        assert!(stop_app(&mut ledger, first).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(model.resource), Some(1));
        assert_eq!(
            stop_app(&mut ledger, second).unwrap(),
            vec![LedgerCleanupRequest {
                resource: model.resource,
                kind: LedgerResourceKind::ModelContext
            }]
        );
    }

    #[test]
    fn l04_final_managed_task_releases_one_cleanup_and_ack_only_after_unload() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(3);
        ledger.open_owner(owner);
        let model = binding(
            102,
            ModelOwner::HuggingFace,
            ContextAuthority::ManagedShared,
        );
        let mut first =
            register_initial_task(&mut ledger, &model, owner, consumer("app", "advisor")).unwrap();
        let mut other = attach_task(&mut ledger, &model, owner, consumer("app", "agent")).unwrap();
        assert_eq!(
            first.release(&mut ledger).unwrap(),
            TaskRelease::Delegated(None)
        );
        assert_eq!(
            other.release(&mut ledger).unwrap(),
            TaskRelease::Delegated(Some(LedgerCleanupRequest {
                resource: model.resource,
                kind: LedgerResourceKind::ModelContext
            }))
        );
        assert_eq!(ledger.active_lease_count(model.resource), Some(0));
        // Last release generates only a REQUEST; cleanup remains pending.
        assert_eq!(
            other.release(&mut ledger).unwrap(),
            TaskRelease::AlreadyReleased
        );
        assert!(stop_app(&mut ledger, owner).unwrap().is_empty());
        ledger.complete_cleanup(model.resource, true).unwrap();
        assert_eq!(ledger.active_lease_count(model.resource), None);
    }

    #[test]
    fn l07_borrowed_jan_and_ollama_survive_stop_and_duplicate_stop() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(4);
        ledger.open_owner(owner);
        let jan = binding(
            103,
            ModelOwner::JanDesktop,
            ContextAuthority::BorrowedExternal(ModelOwner::JanDesktop),
        );
        let ollama = binding(
            104,
            ModelOwner::Ollama,
            ContextAuthority::BorrowedExternal(ModelOwner::Ollama),
        );
        register_initial_task(&mut ledger, &jan, owner, consumer("app", "jan")).unwrap();
        register_initial_task(&mut ledger, &ollama, owner, consumer("app", "ollama")).unwrap();
        assert!(stop_app(&mut ledger, owner).unwrap().is_empty());
        assert!(stop_app(&mut ledger, owner).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(jan.resource), None);
        assert_eq!(ledger.active_lease_count(ollama.resource), None);
    }

    #[test]
    fn l15_independent_models_on_same_host_are_distinct_resources() {
        let mut ledger = MockResourceLedger::default();
        let a = LedgerOwnerRef(5);
        let b = LedgerOwnerRef(6);
        ledger.open_owner(a);
        ledger.open_owner(b);
        let one = binding(105, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        let two = binding(106, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        register_initial_task(&mut ledger, &one, a, consumer("one", "job")).unwrap();
        register_initial_task(&mut ledger, &two, b, consumer("two", "job")).unwrap();
        assert_eq!(one.model_id, two.model_id); // same file, two contexts
        assert_ne!(one.resource, two.resource); // core IDs are distinct
        assert_eq!(stop_app(&mut ledger, a).unwrap().len(), 1);
        assert_eq!(ledger.active_lease_count(two.resource), Some(1));
    }

    #[test]
    fn duplicate_stop_blocks_new_lease_and_does_not_reissue_cleanup() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(7);
        ledger.open_owner(owner);
        let model = binding(
            107,
            ModelOwner::AiTools,
            ContextAuthority::ManagedExclusive(owner),
        );
        register_initial_task(&mut ledger, &model, owner, consumer("app", "job")).unwrap();
        let first = stop_app(&mut ledger, owner).unwrap();
        assert_eq!(first.len(), 1);
        assert!(stop_app(&mut ledger, owner).unwrap().is_empty());
        assert_eq!(
            attach_task(&mut ledger, &model, owner, consumer("app", "late")).err(),
            Some(FakeError::UnknownOrStoppedOwner)
        );
    }

    #[test]
    fn stop_first_then_start_is_denied_and_same_owner_receipts_retired() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(80);
        let other = LedgerOwnerRef(81);
        ledger.open_owner(owner);
        ledger.open_owner(other);
        let binding = binding(180, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        let mut one =
            register_initial_task(&mut ledger, &binding, owner, consumer("app-80", "task-a"))
                .unwrap();
        let mut two =
            attach_task(&mut ledger, &binding, other, consumer("app-81", "task-b")).unwrap();
        let results =
            stop_app_with_receipts(&mut ledger, owner, std::slice::from_mut(&mut one)).unwrap();
        assert!(results.is_empty());
        assert_eq!(
            one.release(&mut ledger).unwrap(),
            TaskRelease::AlreadyReleased
        );
        assert_eq!(ledger.active_lease_count(binding.resource), Some(1));
        assert_eq!(
            attach_task(&mut ledger, &binding, owner, consumer("app-80", "late")).err(),
            Some(FakeError::UnknownOrStoppedOwner),
        );
        assert_eq!(
            two.release(&mut ledger).unwrap(),
            TaskRelease::Delegated(Some(LedgerCleanupRequest {
                resource: binding.resource,
                kind: LedgerResourceKind::ModelContext,
            })),
        );
    }

    #[test]
    fn duplicate_app_stop_does_not_create_new_cleanup() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(82);
        ledger.open_owner(owner);
        let binding = binding(182, ModelOwner::AiTools, ContextAuthority::ManagedShared);
        let mut receipt =
            register_initial_task(&mut ledger, &binding, owner, consumer("app-82", "task-a"))
                .unwrap();
        assert_eq!(
            stop_app_with_receipts(&mut ledger, owner, std::slice::from_mut(&mut receipt))
                .unwrap()
                .len(),
            1
        );
        assert!(stop_app(&mut ledger, owner).unwrap().is_empty());
        assert_eq!(
            receipt.release(&mut ledger).unwrap(),
            TaskRelease::AlreadyReleased
        );
        assert_eq!(ledger.active_lease_count(binding.resource), Some(0));
    }

    #[test]
    fn external_jan_file_path_remains_an_unmodified_reference() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(83);
        ledger.open_owner(owner);
        let binding = binding(183, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        let source_path = match &binding.model_id {
            ModelId::Artifact {
                owner,
                source_path,
                canonical_path,
                ..
            } => {
                assert_eq!(owner, &ModelOwner::JanDesktop);
                assert_eq!(source_path, canonical_path);
                source_path.clone()
            }
            ModelId::Endpoint { .. } => panic!("expected file reference"),
        };
        let mut receipt =
            register_initial_task(&mut ledger, &binding, owner, consumer("app-83", "job")).unwrap();
        let cleanup = receipt.release(&mut ledger).unwrap();
        assert!(matches!(cleanup, TaskRelease::Delegated(Some(_))));
        assert!(matches!(
            &receipt.model_id,
            ModelId::Artifact {
                owner: ModelOwner::JanDesktop,
                ..
            }
        ));
        assert_eq!(
            source_path.to_string_lossy(),
            "/synthetic/external/model.gguf"
        );
        assert_eq!(ledger.active_lease_count(binding.resource), Some(0));
    }

    #[test]
    fn stopping_unrelated_owner_cannot_emit_foreign_cleanup() {
        let mut ledger = MockResourceLedger::default();
        let first = LedgerOwnerRef(84);
        let unrelated = LedgerOwnerRef(85);
        ledger.open_owner(first);
        ledger.open_owner(unrelated);
        let binding = binding(184, ModelOwner::AiTools, ContextAuthority::ManagedShared);
        register_initial_task(&mut ledger, &binding, first, consumer("app-84", "task-a")).unwrap();
        assert!(stop_app(&mut ledger, unrelated).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(binding.resource), Some(1));
        assert_eq!(stop_app(&mut ledger, first).unwrap().len(), 1);
    }

    #[test]
    fn borrowed_runtime_and_external_file_have_separate_owners() {
        let mut ledger = MockResourceLedger::default();
        let owner = LedgerOwnerRef(8);
        ledger.open_owner(owner);
        // An external Jan model file can be loaded in a TIHULU-owned worker.
        // The worker is unloadable; the Jan model FILE remains external.
        let model = binding(108, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        let mut task =
            register_initial_task(&mut ledger, &model, owner, consumer("app", "task")).unwrap();
        assert!(matches!(
            &task.model_id,
            ModelId::Artifact {
                owner: ModelOwner::JanDesktop,
                ..
            }
        ));
        assert!(matches!(
            task.release(&mut ledger).unwrap(),
            TaskRelease::Delegated(Some(_))
        ));
        ledger.complete_cleanup(model.resource, true).unwrap();
        assert_eq!(ledger.active_lease_count(model.resource), None);
    }
}
