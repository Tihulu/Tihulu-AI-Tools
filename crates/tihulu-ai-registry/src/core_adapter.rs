//! Opt-in delegation to coordinator ResourceLedger (PR #12 / sandbox #17).
//! No independent count, unload policy, filesystem or runtime side effects.

use crate::leases::{
    ContextAuthority, LedgerCleanupRequest, LedgerOwnerRef, LedgerResourceKind, LedgerResourceRef,
    ModelRuntimeBinding, ResourceLedgerPort,
};
use tihulu_ai_core::{
    CleanupRequest, LifecycleError, OwnerId, Provenance, ResourceId, ResourceKind, ResourceLedger,
};

impl ResourceLedgerPort for ResourceLedger {
    type Error = LifecycleError;

    fn register_model_context(
        &mut self,
        initial_owner: LedgerOwnerRef,
        binding: &ModelRuntimeBinding,
    ) -> Result<(), Self::Error> {
        let provenance = match &binding.authority {
            ContextAuthority::ManagedShared => Provenance::SharedManaged,
            ContextAuthority::ManagedExclusive(owner) => Provenance::Exclusive(OwnerId(owner.0)),
            ContextAuthority::BorrowedExternal(_) => Provenance::BorrowedExternal,
        };
        ResourceLedger::register(
            self,
            OwnerId(initial_owner.0),
            ResourceId(binding.resource.0),
            ResourceKind::ModelContext,
            provenance,
        )
    }

    fn attach(
        &mut self,
        owner: LedgerOwnerRef,
        resource: LedgerResourceRef,
    ) -> Result<(), Self::Error> {
        ResourceLedger::attach(self, OwnerId(owner.0), ResourceId(resource.0))
    }

    fn release_one(
        &mut self,
        owner: LedgerOwnerRef,
        resource: LedgerResourceRef,
    ) -> Result<Option<LedgerCleanupRequest>, Self::Error> {
        ResourceLedger::release_one(self, OwnerId(owner.0), ResourceId(resource.0))
            .map(|cleanup| cleanup.map(map_cleanup))
    }

    fn stop_owner(
        &mut self,
        owner: LedgerOwnerRef,
    ) -> Result<Vec<LedgerCleanupRequest>, Self::Error> {
        ResourceLedger::stop_owner(self, OwnerId(owner.0))
            .map(|requests| requests.into_iter().map(map_cleanup).collect())
    }

    fn active_lease_count(&self, resource: LedgerResourceRef) -> Option<usize> {
        ResourceLedger::active_lease_count(self, ResourceId(resource.0))
    }

    fn complete_cleanup(
        &mut self,
        resource: LedgerResourceRef,
        succeeded: bool,
    ) -> Result<(), Self::Error> {
        ResourceLedger::complete_cleanup(self, ResourceId(resource.0), succeeded)
    }
}

fn map_cleanup(request: CleanupRequest) -> LedgerCleanupRequest {
    let kind = match request.kind {
        ResourceKind::ModelContext => LedgerResourceKind::ModelContext,
        ResourceKind::GpuAllocation => LedgerResourceKind::GpuAllocation,
        ResourceKind::AudioRamBuffer => LedgerResourceKind::AudioRamBuffer,
        ResourceKind::AiWorker => LedgerResourceKind::AiWorker,
        ResourceKind::SdrHandle => LedgerResourceKind::SdrHandle,
        ResourceKind::NetworkSession => LedgerResourceKind::NetworkSession,
        ResourceKind::RemoteInferenceLease => LedgerResourceKind::RemoteInferenceLease,
        ResourceKind::TemporaryFile => LedgerResourceKind::TemporaryFile,
    };
    LedgerCleanupRequest {
        resource: LedgerResourceRef(request.resource.0),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leases::{
        attach_task, register_initial_task, stop_app, stop_app_with_receipts, AppSessionId,
        ConsumerTask, RuntimeInstanceId, TaskRelease, TaskSessionId,
    };
    use crate::registry::{ModelId, ModelOwner, SourceAdapter};

    fn task(app: &str, name: &str) -> ConsumerTask {
        ConsumerTask {
            app_session: AppSessionId(app.into()),
            task_session: TaskSessionId(name.into()),
        }
    }

    fn model(
        id: u64,
        source_owner: ModelOwner,
        authority: ContextAuthority,
    ) -> ModelRuntimeBinding {
        ModelRuntimeBinding {
            model_id: ModelId::Endpoint {
                adapter: SourceAdapter::LocalInferenceApi,
                owner: source_owner,
                base_url: "http://localhost:11434".into(),
                served_model_id: "model:synthetic".into(),
            },
            runtime_instance: RuntimeInstanceId(format!("synthetic-instance-{id}")),
            resource: LedgerResourceRef(id),
            authority,
        }
    }

    #[test]
    fn core_two_apps_share_until_last_client() {
        let mut ledger = ResourceLedger::new();
        ledger.open_owner(OwnerId(41)).unwrap();
        ledger.open_owner(OwnerId(42)).unwrap();
        let model = model(910, ModelOwner::JanDesktop, ContextAuthority::ManagedShared);
        register_initial_task(&mut ledger, &model, LedgerOwnerRef(41), task("media", "asr"))
            .unwrap();
        attach_task(&mut ledger, &model, LedgerOwnerRef(42), task("text", "asr")).unwrap();
        assert_eq!(ledger.active_lease_count(ResourceId(910)), Some(2));
        assert!(stop_app(&mut ledger, LedgerOwnerRef(41)).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(ResourceId(910)), Some(1));
        assert_eq!(
            stop_app(&mut ledger, LedgerOwnerRef(42)).unwrap(),
            vec![LedgerCleanupRequest {
                resource: LedgerResourceRef(910),
                kind: LedgerResourceKind::ModelContext,
            }]
        );
        assert!(stop_app(&mut ledger, LedgerOwnerRef(42)).unwrap().is_empty());
        assert_eq!(ledger.pending_cleanup_count(), 1);
        ledger.complete_cleanup(ResourceId(910), true).unwrap();
        assert_eq!(ledger.active_lease_count(ResourceId(910)), None);
    }

    #[test]
    fn core_borrowed_jan_and_ollama_cannot_trigger_unload() {
        let mut ledger = ResourceLedger::new();
        ledger.open_owner(OwnerId(51)).unwrap();
        for (resource, owner) in [(920, ModelOwner::JanDesktop), (921, ModelOwner::Ollama)] {
            let binding = model(
                resource,
                owner.clone(),
                ContextAuthority::BorrowedExternal(owner),
            );
            register_initial_task(
                &mut ledger,
                &binding,
                LedgerOwnerRef(51),
                task("app", "borrowed"),
            )
            .unwrap();
        }
        assert!(stop_app(&mut ledger, LedgerOwnerRef(51)).unwrap().is_empty());
        assert_eq!(ledger.pending_cleanup_count(), 0);
        assert!(stop_app(&mut ledger, LedgerOwnerRef(51)).unwrap().is_empty());
    }

    #[test]
    fn core_start_quit_interleavings_and_duplicate_stop() {
        let mut ledger = ResourceLedger::new();
        ledger.open_owner(OwnerId(61)).unwrap();
        let model = model(930, ModelOwner::AiTools, ContextAuthority::ManagedShared);

        // Stop-first serial ordering: no late start.
        assert!(stop_app(&mut ledger, LedgerOwnerRef(61)).unwrap().is_empty());
        assert_eq!(
            register_initial_task(&mut ledger, &model, LedgerOwnerRef(61), task("app", "late"))
                .err(),
            Some(LifecycleError::OwnerNotRunning)
        );

        // Start-first serial ordering: one cleanup only.
        ledger.open_owner(OwnerId(62)).unwrap();
        let mut receipt = register_initial_task(
            &mut ledger,
            &model,
            LedgerOwnerRef(62),
            task("new-app", "active"),
        )
        .unwrap();
        let requests = stop_app_with_receipts(
            &mut ledger,
            LedgerOwnerRef(62),
            std::slice::from_mut(&mut receipt),
        )
        .unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(receipt.release(&mut ledger).unwrap(), TaskRelease::AlreadyReleased);
        assert!(stop_app(&mut ledger, LedgerOwnerRef(62)).unwrap().is_empty());
        assert_eq!(ledger.pending_cleanup_count(), 1);
        assert_eq!(
            attach_task(&mut ledger, &model, LedgerOwnerRef(62), task("new-app", "late"))
                .err(),
            Some(LifecycleError::OwnerNotRunning)
        );
    }
}
