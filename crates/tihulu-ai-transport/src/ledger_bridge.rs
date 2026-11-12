//! Host-local bridge to the ACTUAL core ResourceLedger.
//!
//! ResourceLedger is the *only* authority that may emit model CleanupRequest.
//! This adapter holds model/lease correlation metadata and MOCK cleanup
//! observations, but cannot decide a model should unload by reference counting.
//! No sockets, real workers, GPU queries, model loading or credentials are used.
use crate::host::MockComputeHost;
use crate::protocol::{
    AppId, AppSessionId, ClientId, Error, HostId, LeaseId, LedgerLeaseAction, LedgerLeaseEvent,
    LedgerOwnerId, LedgerResourceId, LedgerRuntimeBinding, LifecycleAuthority, ModelId,
    RuntimeOwnership,
};
use std::collections::{HashMap, HashSet};
use tihulu_ai_core::resource_lifecycle::{
    CleanupRequest, LifecycleError, OwnerId, Provenance, ResourceId, ResourceKind, ResourceLedger,
    ResourcePhase,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    Host(Error),
    Core(LifecycleError),
    WrongAuthority,
    UnregisteredRuntime,
    InconsistentIdentity,
    DuplicateReceipt,
    NoCleanupRequest,
    BorrowedCannotUnload,
}

impl From<Error> for BridgeError {
    fn from(value: Error) -> Self {
        Self::Host(value)
    }
}

impl From<LifecycleError> for BridgeError {
    fn from(value: LifecycleError) -> Self {
        Self::Core(value)
    }
}

#[derive(Clone, Debug)]
struct RuntimeDefinition {
    binding: LedgerRuntimeBinding,
    provenance: Provenance,
    estimated_owned_ram_mib: u64,
    estimated_owned_vram_mib: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupTicket {
    /// Only a core ResourceLedger::CleanupRequest can create this ticket.
    pub resource: LedgerResourceId,
    pub model: ModelId,
    pub generation: u64,
    pub expected_ram_mib: u64,
    pub expected_vram_mib: u64,
    pub retry_number: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockCleanupObservation {
    /// These are synthetic numbers injected by a test, NOT real memory readings.
    pub joined_owned_workers: bool,
    pub disposed_owned_context: bool,
    pub remaining_attributable_ram_mib: u64,
    pub remaining_attributable_vram_mib: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MockCleanupFailure {
    WorkerNotJoined,
    OwnedContextStillResident,
    RemainingRam(u64),
    RemainingVram(u64),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MockCleanupReport {
    pub resource: LedgerResourceId,
    pub model: ModelId,
    pub retry_number: u32,
    pub succeeded_in_fixture: bool,
    pub failures: Vec<MockCleanupFailure>,
    pub observed_remaining_ram_mib: u64,
    pub observed_remaining_vram_mib: u64,
}

impl MockCleanupObservation {
    fn failures(&self) -> Vec<MockCleanupFailure> {
        let mut failures = Vec::new();
        if !self.joined_owned_workers {
            failures.push(MockCleanupFailure::WorkerNotJoined);
        }
        if !self.disposed_owned_context {
            failures.push(MockCleanupFailure::OwnedContextStillResident);
        }
        if self.remaining_attributable_ram_mib != 0 {
            failures.push(MockCleanupFailure::RemainingRam(
                self.remaining_attributable_ram_mib,
            ));
        }
        if self.remaining_attributable_vram_mib != 0 {
            failures.push(MockCleanupFailure::RemainingVram(
                self.remaining_attributable_vram_mib,
            ));
        }
        failures
    }
}

/// In-memory adapter that uses the real coordinator core, not a parallel
/// reference-count/cleanup state machine. Each instance models one host-side
/// ledger authority; production must persist event receipts before acknowledgments.
pub struct AuthoritativeMockLedgerBridge {
    ledger: ResourceLedger,
    runtimes: HashMap<LedgerResourceId, RuntimeDefinition>,
    registered_resources: HashSet<LedgerResourceId>,
    active_lease_correlations: HashMap<LeaseId, (HostId, LedgerResourceId, LedgerOwnerId)>,
    pending: HashMap<LedgerResourceId, CleanupTicket>,
    failed: HashMap<LedgerResourceId, CleanupTicket>,
    reports: Vec<MockCleanupReport>,
    cleanup_request_counts: HashMap<LedgerResourceId, u64>,
}

impl Default for AuthoritativeMockLedgerBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl AuthoritativeMockLedgerBridge {
    pub fn new() -> Self {
        Self {
            ledger: ResourceLedger::new(),
            runtimes: HashMap::new(),
            registered_resources: HashSet::new(),
            active_lease_correlations: HashMap::new(),
            pending: HashMap::new(),
            failed: HashMap::new(),
            reports: Vec::new(),
            cleanup_request_counts: HashMap::new(),
        }
    }

    /// Admin-only local binding. The core owner ID is independent of device
    /// and application display IDs; a new application launch gets a new ID.
    pub fn register_app_owner(
        &mut self,
        host: &mut MockComputeHost,
        client: ClientId,
        app: AppId,
        session: AppSessionId,
        owner: LedgerOwnerId,
    ) -> Result<(), BridgeError> {
        Self::require_authority(host)?;
        if self.ledger.owner_phase(OwnerId(owner.0)).is_some() {
            return Err(BridgeError::Core(LifecycleError::DuplicateOwner));
        }
        host.bind_ledger_owner(client, app, session, owner)?;
        self.ledger.open_owner(OwnerId(owner.0))?;
        Ok(())
    }

    /// Declare runtime provenance once from a trusted host-local inventory.
    /// The host's registration must agree: a borrowed model can never be
    /// mislabelled host-owned merely to obtain a cleanup request.
    pub fn bind_runtime(
        &mut self,
        host: &mut MockComputeHost,
        binding: LedgerRuntimeBinding,
        ownership: RuntimeOwnership,
        estimated_owned_ram_mib: u64,
        estimated_owned_vram_mib: u64,
    ) -> Result<(), BridgeError> {
        Self::require_authority(host)?;
        if host.model_ownership(&binding.model) != Some(ownership) {
            return Err(BridgeError::InconsistentIdentity);
        }
        if self.runtimes.contains_key(&binding.resource) {
            return Err(BridgeError::DuplicateReceipt);
        }
        host.bind_ledger_runtime(binding.clone())?;
        self.runtimes.insert(
            binding.resource,
            RuntimeDefinition {
                binding,
                provenance: match ownership {
                    RuntimeOwnership::HostOwned => Provenance::SharedManaged,
                    RuntimeOwnership::BorrowedExternal => Provenance::BorrowedExternal,
                },
                estimated_owned_ram_mib: if ownership == RuntimeOwnership::HostOwned {
                    estimated_owned_ram_mib
                } else {
                    0
                },
                estimated_owned_vram_mib: if ownership == RuntimeOwnership::HostOwned {
                    estimated_owned_vram_mib
                } else {
                    0
                },
            },
        );
        Ok(())
    }

    /// A new owned context must use fresh runtime+numeric ResourceId; a borrowed
    /// external provider may keep its runtime ID but needs a new ledger epoch.
    pub fn rotate_runtime(
        &mut self,
        host: &mut MockComputeHost,
        binding: LedgerRuntimeBinding,
        ownership: RuntimeOwnership,
        estimated_owned_ram_mib: u64,
        estimated_owned_vram_mib: u64,
    ) -> Result<(), BridgeError> {
        Self::require_authority(host)?;
        if host.model_ownership(&binding.model) != Some(ownership) {
            return Err(BridgeError::InconsistentIdentity);
        }
        if self.runtimes.contains_key(&binding.resource) {
            return Err(BridgeError::DuplicateReceipt);
        }
        host.rotate_ledger_runtime(binding.clone())?;
        self.runtimes.insert(
            binding.resource,
            RuntimeDefinition {
                binding,
                provenance: match ownership {
                    RuntimeOwnership::HostOwned => Provenance::SharedManaged,
                    RuntimeOwnership::BorrowedExternal => Provenance::BorrowedExternal,
                },
                estimated_owned_ram_mib: if ownership == RuntimeOwnership::HostOwned {
                    estimated_owned_ram_mib
                } else {
                    0
                },
                estimated_owned_vram_mib: if ownership == RuntimeOwnership::HostOwned {
                    estimated_owned_vram_mib
                } else {
                    0
                },
            },
        );
        Ok(())
    }

    fn require_authority(host: &MockComputeHost) -> Result<(), BridgeError> {
        if host.lifecycle_authority() != LifecycleAuthority::CoordinatorLedger {
            return Err(BridgeError::WrongAuthority);
        }
        Ok(())
    }

    /// Drain ordered host lease events into core. The bridge DOES NOT derive
    /// cleanup from a host counter; only the core release_one return may request it.
    pub fn consume_host_events(
        &mut self,
        host: &mut MockComputeHost,
        now_ms: u64,
    ) -> Result<usize, BridgeError> {
        Self::require_authority(host)?;
        let mut consumed = 0;
        while let Some(event) = host.pending_ledger_events().front().cloned() {
            self.apply_event(host.host_id(), &event, host.model_generation(&event.model))?;
            if !host.acknowledge_ledger_event(event.sequence, now_ms) {
                return Err(BridgeError::InconsistentIdentity);
            }
            consumed += 1;
        }
        Ok(consumed)
    }

    fn apply_event(
        &mut self,
        expected_host: &HostId,
        event: &LedgerLeaseEvent,
        generation: Option<u64>,
    ) -> Result<(), BridgeError> {
        if &event.host != expected_host {
            return Err(BridgeError::InconsistentIdentity);
        }
        let definition = self
            .runtimes
            .get(&event.resource)
            .ok_or(BridgeError::UnregisteredRuntime)?
            .clone();
        if definition.binding.model != event.model
            || definition.binding.runtime != event.runtime
            || definition.binding.resource != event.resource
        {
            return Err(BridgeError::InconsistentIdentity);
        }
        let correlation = (event.host.clone(), event.resource, event.owner);
        let resource = ResourceId(event.resource.0);
        let owner = OwnerId(event.owner.0);
        match event.action {
            LedgerLeaseAction::Acquired => {
                if self.active_lease_correlations.contains_key(&event.lease) {
                    return Err(BridgeError::DuplicateReceipt);
                }
                if self.registered_resources.contains(&event.resource) {
                    self.ledger.attach(owner, resource)?;
                } else {
                    self.ledger.register(
                        owner,
                        resource,
                        ResourceKind::ModelContext,
                        definition.provenance,
                    )?;
                    self.registered_resources.insert(event.resource);
                }
                self.active_lease_correlations
                    .insert(event.lease.clone(), correlation);
            }
            LedgerLeaseAction::Released => {
                if self.active_lease_correlations.get(&event.lease) != Some(&correlation) {
                    return Err(BridgeError::InconsistentIdentity);
                }
                let cleanup = self.ledger.release_one(owner, resource)?;
                self.active_lease_correlations.remove(&event.lease);
                if let Some(CleanupRequest {
                    resource,
                    kind: ResourceKind::ModelContext,
                }) = cleanup
                {
                    if definition.provenance == Provenance::BorrowedExternal {
                        return Err(BridgeError::BorrowedCannotUnload);
                    }
                    let ticket = CleanupTicket {
                        resource: event.resource,
                        model: event.model.clone(),
                        generation: generation.ok_or(BridgeError::InconsistentIdentity)?,
                        expected_ram_mib: definition.estimated_owned_ram_mib,
                        expected_vram_mib: definition.estimated_owned_vram_mib,
                        retry_number: 0,
                    };
                    if self.pending.insert(event.resource, ticket).is_some() {
                        return Err(BridgeError::DuplicateReceipt);
                    }
                    *self
                        .cleanup_request_counts
                        .entry(event.resource)
                        .or_default() += 1;
                    // The request is core-authoritative and must be completed
                    // by a future trusted backend, never by this host counter.
                    debug_assert_eq!(resource, ResourceId(event.resource.0));
                } else if cleanup.is_some() {
                    return Err(BridgeError::InconsistentIdentity);
                }
            }
        }
        Ok(())
    }

    pub fn cleanup_request_count(&self, resource: LedgerResourceId) -> u64 {
        self.cleanup_request_counts
            .get(&resource)
            .copied()
            .unwrap_or(0)
    }

    pub fn pending_cleanup(&self, resource: LedgerResourceId) -> Option<&CleanupTicket> {
        self.pending.get(&resource)
    }

    pub fn cleanup_phase(&self, resource: LedgerResourceId) -> Option<ResourcePhase> {
        self.ledger.resource_phase(ResourceId(resource.0))
    }

    pub fn active_core_leases(&self, resource: LedgerResourceId) -> Option<usize> {
        self.ledger.active_lease_count(ResourceId(resource.0))
    }

    pub fn reports(&self) -> &[MockCleanupReport] {
        &self.reports
    }

    /// MOCK backend completion. A *real* driver adapter must independently
    /// prove owned worker join, context disposal and attributable RAM/VRAM
    /// readings before issuing the equivalent local acknowledgment.
    pub fn finish_mock_cleanup(
        &mut self,
        host: &mut MockComputeHost,
        resource: LedgerResourceId,
        observation: MockCleanupObservation,
        now_ms: u64,
    ) -> Result<MockCleanupReport, BridgeError> {
        Self::require_authority(host)?;
        let ticket = self
            .pending
            .remove(&resource)
            .ok_or(BridgeError::NoCleanupRequest)?;
        let failures = observation.failures();
        let ok = failures.is_empty();
        self.ledger.complete_cleanup(ResourceId(resource.0), ok)?;
        host.acknowledge_ledger_unload(&ticket.model, resource, ticket.generation, ok, now_ms)?;
        let report = MockCleanupReport {
            resource,
            model: ticket.model.clone(),
            retry_number: ticket.retry_number,
            succeeded_in_fixture: ok,
            failures,
            observed_remaining_ram_mib: observation.remaining_attributable_ram_mib,
            observed_remaining_vram_mib: observation.remaining_attributable_vram_mib,
        };
        if !ok {
            self.failed.insert(resource, ticket);
        }
        self.reports.push(report.clone());
        Ok(report)
    }

    /// An unsuccessful cleanup remains visible. Only core.retry_cleanup
    /// authorizes a new attempt; it does not create a second first-release.
    pub fn retry_failed_cleanup(
        &mut self,
        resource: LedgerResourceId,
    ) -> Result<CleanupTicket, BridgeError> {
        let mut ticket = self
            .failed
            .remove(&resource)
            .ok_or(BridgeError::NoCleanupRequest)?;
        self.ledger.retry_cleanup(ResourceId(resource.0))?;
        ticket.retry_number += 1;
        self.pending.insert(resource, ticket.clone());
        Ok(ticket)
    }
}
