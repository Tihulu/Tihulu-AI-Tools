//! Pure, ownership-aware resource and model lease state machine.
//!
//! This module produces cleanup *requests*. It does not touch GPU, RAM, SDR,
//! OS processes, Jan, Ollama or filesystem resources itself. Each adapter must
//! perform and acknowledge the requested cleanup for an owned resource.
//! Borrowed external resources never generate a termination/unload request.

use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OwnerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ResourceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnerPhase {
    Running,
    Stopping,
    Stopped,
}

/// Describes who has the authority to unload/close the actual resource.
///
/// A model file owned by Jan and a separate TIHULU-owned loaded context for
/// that file must have distinct ResourceId values and ownership labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// Only this owner may acquire leases; cleanup after their final lease.
    Exclusive(OwnerId),
    /// TIHULU-managed runtime shared by multiple authorized owner sessions.
    SharedManaged,
    /// Pre-existing Jan/Ollama/readsb/other-app service; release only our
    /// client references, never terminate/unload the service itself.
    BorrowedExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
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
pub enum ResourcePhase {
    Active,
    CleanupPending,
    CleanupFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupRequest {
    pub resource: ResourceId,
    pub kind: ResourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleError {
    UnknownOwner,
    DuplicateOwner,
    OwnerNotRunning,
    DuplicateResource,
    UnknownResource,
    WrongOwner,
    NotHeld,
    ExclusiveResource,
    UnavailableResource,
    CounterOverflow,
    CleanupNotPending,
    CleanupNotFailed,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for LifecycleError {}

#[derive(Debug)]
struct Entry {
    kind: ResourceKind,
    provenance: Provenance,
    phase: ResourcePhase,
    /// One owner may hold multiple independent task references.
    holders: HashMap<OwnerId, usize>,
}

/// A single-process pure state machine. Actual scheduling/locking is the
/// responsibility of the caller, who must serialize calls or guard this value
/// with a mutex. It does not claim model unloading has happened until an
/// adapter calls `complete_cleanup`.
#[derive(Debug, Default)]
pub struct ResourceLedger {
    owners: HashMap<OwnerId, OwnerPhase>,
    resources: HashMap<ResourceId, Entry>,
    /// Resource IDs are never recycled within a ledger lifetime. An old
    /// asynchronous cleanup acknowledgment cannot affect a later resource.
    used_resource_ids: HashSet<ResourceId>,
}

impl ResourceLedger {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_owner(&mut self, owner: OwnerId) -> Result<(), LifecycleError> {
        if self.owners.contains_key(&owner) {
            return Err(LifecycleError::DuplicateOwner);
        }
        self.owners.insert(owner, OwnerPhase::Running);
        Ok(())
    }

    pub fn owner_phase(&self, owner: OwnerId) -> Option<OwnerPhase> {
        self.owners.get(&owner).copied()
    }

    fn require_running(&self, owner: OwnerId) -> Result<(), LifecycleError> {
        match self.owner_phase(owner) {
            Some(OwnerPhase::Running) => Ok(()),
            Some(_) => Err(LifecycleError::OwnerNotRunning),
            None => Err(LifecycleError::UnknownOwner),
        }
    }

    /// Register an app-owned, TIHULU shared, or *borrowed* resource.
    ///
    /// Only approved adapters may call this, after obtaining the real handle
    /// through their own permission checks. Foreign services stay Borrowed.
    pub fn register(
        &mut self,
        owner: OwnerId,
        resource: ResourceId,
        kind: ResourceKind,
        provenance: Provenance,
    ) -> Result<(), LifecycleError> {
        self.require_running(owner)?;
        if matches!(provenance, Provenance::Exclusive(exclusive) if exclusive != owner) {
            return Err(LifecycleError::WrongOwner);
        }
        if self.used_resource_ids.contains(&resource) {
            return Err(LifecycleError::DuplicateResource);
        }
        let mut holders = HashMap::new();
        holders.insert(owner, 1);
        self.resources.insert(
            resource,
            Entry {
                kind,
                provenance,
                phase: ResourcePhase::Active,
                holders,
            },
        );
        self.used_resource_ids.insert(resource);
        Ok(())
    }

    /// Acquire a single additional reference to a shared or own resource.
    /// An exclusive resource cannot be leased to another owner.
    pub fn attach(
        &mut self,
        owner: OwnerId,
        resource: ResourceId,
    ) -> Result<(), LifecycleError> {
        self.require_running(owner)?;
        let entry = self
            .resources
            .get_mut(&resource)
            .ok_or(LifecycleError::UnknownResource)?;
        if entry.phase != ResourcePhase::Active {
            return Err(LifecycleError::UnavailableResource);
        }
        if matches!(entry.provenance, Provenance::Exclusive(exclusive) if exclusive != owner) {
            return Err(LifecycleError::ExclusiveResource);
        }
        let count = entry.holders.entry(owner).or_default();
        *count = count.checked_add(1).ok_or(LifecycleError::CounterOverflow)?;
        Ok(())
    }

    /// Release one reference when a task finishes, without stopping the owner
    /// itself. Returns an unload/close request only for last owned references.
    pub fn release_one(
        &mut self,
        owner: OwnerId,
        resource: ResourceId,
    ) -> Result<Option<CleanupRequest>, LifecycleError> {
        if !self.owners.contains_key(&owner) {
            return Err(LifecycleError::UnknownOwner);
        }
        let entry = self
            .resources
            .get_mut(&resource)
            .ok_or(LifecycleError::UnknownResource)?;
        if entry.phase != ResourcePhase::Active {
            return Err(LifecycleError::UnavailableResource);
        }
        let count = entry.holders.get_mut(&owner).ok_or(LifecycleError::NotHeld)?;
        *count -= 1;
        if *count == 0 {
            entry.holders.remove(&owner);
        }
        Ok(self.after_last_release(resource))
    }

    /// Idempotent stop: reject future work before releasing every reference
    /// owned by this session. Return only cleanup requests for *owned* resources.
    ///
    /// The caller must execute cleanup requests with its backend, then report
    /// success/failure with `complete_cleanup`. Physical memory release cannot
    /// be asserted just because this method returned.
    pub fn stop_owner(
        &mut self,
        owner: OwnerId,
    ) -> Result<Vec<CleanupRequest>, LifecycleError> {
        let phase = self.owners.get_mut(&owner).ok_or(LifecycleError::UnknownOwner)?;
        if *phase != OwnerPhase::Running {
            return Ok(Vec::new());
        }
        *phase = OwnerPhase::Stopping;

        let mut ids: Vec<ResourceId> = self
            .resources
            .iter()
            .filter(|(_, entry)| entry.holders.contains_key(&owner))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();

        let mut pending = Vec::new();
        for resource in ids {
            if let Some(entry) = self.resources.get_mut(&resource) {
                // A complete owner shutdown releases all per-task references.
                entry.holders.remove(&owner);
                if let Some(request) = self.after_last_release(resource) {
                    pending.push(request);
                }
            }
        }
        self.owners.insert(owner, OwnerPhase::Stopped);
        Ok(pending)
    }

    fn after_last_release(&mut self, resource: ResourceId) -> Option<CleanupRequest> {
        let entry = self.resources.get_mut(&resource)?;
        if !entry.holders.is_empty() {
            return None;
        }
        if entry.provenance == Provenance::BorrowedExternal {
            // Remove our client reference; do NOT unload/kill external service.
            self.resources.remove(&resource);
            return None;
        }
        if entry.phase != ResourcePhase::Active {
            return None;
        }
        entry.phase = ResourcePhase::CleanupPending;
        Some(CleanupRequest {
            resource,
            kind: entry.kind,
        })
    }

    /// An adapter acknowledges physical/backend cleanup. Failed resources
    /// remain visible for diagnostics and bounded retry by an authorized owner.
    pub fn complete_cleanup(
        &mut self,
        resource: ResourceId,
        succeeded: bool,
    ) -> Result<(), LifecycleError> {
        let entry = self
            .resources
            .get_mut(&resource)
            .ok_or(LifecycleError::UnknownResource)?;
        if entry.phase != ResourcePhase::CleanupPending {
            return Err(LifecycleError::CleanupNotPending);
        }
        if succeeded {
            self.resources.remove(&resource);
        } else {
            entry.phase = ResourcePhase::CleanupFailed;
        }
        Ok(())
    }

    /// Ask the same resource adapter to retry one previously failed cleanup;
    /// never retries cleanup of a borrowed external service.
    pub fn retry_cleanup(
        &mut self,
        resource: ResourceId,
    ) -> Result<CleanupRequest, LifecycleError> {
        let entry = self
            .resources
            .get_mut(&resource)
            .ok_or(LifecycleError::UnknownResource)?;
        if entry.phase != ResourcePhase::CleanupFailed {
            return Err(LifecycleError::CleanupNotFailed);
        }
        entry.phase = ResourcePhase::CleanupPending;
        Ok(CleanupRequest {
            resource,
            kind: entry.kind,
        })
    }

    pub fn resource_phase(&self, resource: ResourceId) -> Option<ResourcePhase> {
        self.resources.get(&resource).map(|e| e.phase)
    }

    pub fn active_lease_count(&self, resource: ResourceId) -> Option<usize> {
        self.resources
            .get(&resource)
            .map(|e| e.holders.values().sum())
    }

    pub fn pending_cleanup_count(&self) -> usize {
        self.resources
            .values()
            .filter(|e| e.phase == ResourcePhase::CleanupPending)
            .count()
    }

    pub fn failed_cleanup_count(&self) -> usize {
        self.resources
            .values()
            .filter(|e| e.phase == ResourcePhase::CleanupFailed)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const A: OwnerId = OwnerId(1);
    const B: OwnerId = OwnerId(2);
    const MODEL: ResourceId = ResourceId(100);
    const SDR: ResourceId = ResourceId(200);

    fn ledger_with_two_owners() -> ResourceLedger {
        let mut manager = ResourceLedger::new();
        manager.open_owner(A).unwrap();
        manager.open_owner(B).unwrap();
        manager
    }

    #[test]
    fn shared_model_unloads_only_on_last_lease() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, MODEL, ResourceKind::ModelContext, Provenance::SharedManaged)
            .unwrap();
        ledger.attach(B, MODEL).unwrap();

        assert!(ledger.stop_owner(A).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(MODEL), Some(1));
        assert_eq!(
            ledger.stop_owner(B).unwrap(),
            vec![CleanupRequest {
                resource: MODEL,
                kind: ResourceKind::ModelContext,
            }]
        );
        assert_eq!(ledger.pending_cleanup_count(), 1);
        ledger.complete_cleanup(MODEL, true).unwrap();
        assert_eq!(ledger.resource_phase(MODEL), None);
    }

    #[test]
    fn one_owner_can_have_multiple_task_references() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, MODEL, ResourceKind::ModelContext, Provenance::SharedManaged)
            .unwrap();
        ledger.attach(A, MODEL).unwrap();
        ledger.attach(B, MODEL).unwrap();
        assert_eq!(ledger.active_lease_count(MODEL), Some(3));
        assert_eq!(ledger.release_one(A, MODEL).unwrap(), None);
        assert_eq!(ledger.active_lease_count(MODEL), Some(2));
        assert!(ledger.stop_owner(A).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(MODEL), Some(1));
        assert_eq!(ledger.stop_owner(B).unwrap().len(), 1);
    }

    #[test]
    fn quit_releases_ai_worker_gpu_allocation_and_sdr_handle() {
        let mut ledger = ResourceLedger::new();
        ledger.open_owner(A).unwrap();
        let resources = [
            (ResourceId(10), ResourceKind::AiWorker),
            (ResourceId(11), ResourceKind::GpuAllocation),
            (ResourceId(12), ResourceKind::AudioRamBuffer),
            (SDR, ResourceKind::SdrHandle),
        ];
        for (resource, kind) in resources {
            ledger
                .register(A, resource, kind, Provenance::Exclusive(A))
                .unwrap();
        }
        let requests = ledger.stop_owner(A).unwrap();
        assert_eq!(requests.len(), resources.len());
        assert_eq!(ledger.owner_phase(A), Some(OwnerPhase::Stopped));
        assert_eq!(ledger.pending_cleanup_count(), resources.len());
        for (id, _) in resources {
            ledger.complete_cleanup(id, true).unwrap();
        }
        assert_eq!(ledger.pending_cleanup_count(), 0);
    }

    #[test]
    fn borrowed_jan_or_readsb_is_never_terminated() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, SDR, ResourceKind::SdrHandle, Provenance::BorrowedExternal)
            .unwrap();
        ledger.attach(B, SDR).unwrap();
        assert!(ledger.stop_owner(A).unwrap().is_empty());
        assert_eq!(ledger.active_lease_count(SDR), Some(1));
        assert!(ledger.stop_owner(B).unwrap().is_empty());
        assert_eq!(ledger.resource_phase(SDR), None);
        assert_eq!(ledger.pending_cleanup_count(), 0);
    }

    #[test]
    fn exclusive_sdr_cannot_be_taken_by_second_app() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, SDR, ResourceKind::SdrHandle, Provenance::Exclusive(A))
            .unwrap();
        assert_eq!(
            ledger.attach(B, SDR),
            Err(LifecycleError::ExclusiveResource)
        );
        assert_eq!(ledger.stop_owner(A).unwrap().len(), 1);
    }

    #[test]
    fn stop_is_idempotent_and_blocks_new_jobs() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, MODEL, ResourceKind::ModelContext, Provenance::Exclusive(A))
            .unwrap();
        assert_eq!(ledger.stop_owner(A).unwrap().len(), 1);
        assert!(ledger.stop_owner(A).unwrap().is_empty());
        assert_eq!(
            ledger.attach(A, MODEL),
            Err(LifecycleError::OwnerNotRunning)
        );
        assert_eq!(
            ledger.register(
                A,
                ResourceId(201),
                ResourceKind::AiWorker,
                Provenance::Exclusive(A)
            ),
            Err(LifecycleError::OwnerNotRunning)
        );
        assert_eq!(ledger.pending_cleanup_count(), 1);
    }

    #[test]
    fn failed_cleanup_remains_reported_for_explicit_retry() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, MODEL, ResourceKind::ModelContext, Provenance::Exclusive(A))
            .unwrap();
        ledger.stop_owner(A).unwrap();
        ledger.complete_cleanup(MODEL, false).unwrap();
        assert_eq!(ledger.failed_cleanup_count(), 1);
        assert_eq!(
            ledger.retry_cleanup(MODEL).unwrap(),
            CleanupRequest {
                resource: MODEL,
                kind: ResourceKind::ModelContext
            }
        );
        ledger.complete_cleanup(MODEL, true).unwrap();
        assert_eq!(ledger.failed_cleanup_count(), 0);
    }

    #[test]
    fn resource_id_reuse_cannot_accept_stale_cleanup_ack() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, MODEL, ResourceKind::AiWorker, Provenance::Exclusive(A))
            .unwrap();
        ledger.stop_owner(A).unwrap();
        ledger.complete_cleanup(MODEL, true).unwrap();
        assert_eq!(
            ledger.register(B, MODEL, ResourceKind::AiWorker, Provenance::Exclusive(B)),
            Err(LifecycleError::DuplicateResource)
        );
        assert_eq!(
            ledger.complete_cleanup(MODEL, true),
            Err(LifecycleError::UnknownResource)
        );
    }

    #[test]
    fn cannot_attach_to_resource_during_cleanup() {
        let mut ledger = ledger_with_two_owners();
        ledger
            .register(A, MODEL, ResourceKind::ModelContext, Provenance::SharedManaged)
            .unwrap();
        ledger.stop_owner(A).unwrap();
        assert_eq!(
            ledger.attach(B, MODEL),
            Err(LifecycleError::UnavailableResource)
        );
    }

    #[test]
    fn stop_requests_are_deterministically_sorted() {
        let mut ledger = ledger_with_two_owners();
        for id in [99, 3, 10, 1] {
            ledger
                .register(
                    A,
                    ResourceId(id),
                    ResourceKind::AiWorker,
                    Provenance::Exclusive(A),
                )
                .unwrap();
        }
        let ids: Vec<_> = ledger
            .stop_owner(A)
            .unwrap()
            .into_iter()
            .map(|req| req.resource.0)
            .collect();
        assert_eq!(ids, vec![1, 3, 10, 99]);
    }
}
