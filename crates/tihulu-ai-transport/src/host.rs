//! Deterministic in-memory host supervisor. No GPU allocations or network sockets.
//! Host-administrative methods are NOT exposed as remote RPCs.
use crate::protocol::*;
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_JOB_RECORDS: usize = 10_000;
const MAX_AUDIT_EVENTS: usize = 1_024;
const MAX_LEDGER_EVENTS: usize = 10_000;
const MAX_LEASE_IDENTITIES: usize = 10_000;
const MAX_LEDGER_RESOURCE_IDENTITIES: usize = 10_000;

#[derive(Clone, Debug)]
struct PairedClient {
    cert_sha256: String,
}
#[derive(Clone, Debug)]
struct ModelState {
    registration: ModelRegistration,
    loaded_by_us: bool,
    load_count: u64,
    unload_count: u64,
    generation: u64,
    awaiting_ledger_ack: bool,
    /// PR #12 ResourceId values cannot be recycled after a final release.
    ledger_resource_spent: bool,
}
#[derive(Clone, Debug)]
struct LeaseRecord {
    client: ClientId,
    app: AppId,
    app_session: AppSessionId,
    task: TaskId,
    model: ModelId,
    ledger_owner: Option<LedgerOwnerId>,
    status: LeaseStatus,
    expires_at_ms: u64,
    close_started_ms: Option<u64>,
}
#[derive(Clone, Debug)]
struct JobRecord {
    client: ClientId,
    app: AppId,
    app_session: AppSessionId,
    task: TaskId,
    spec: RequestSpec,
    status: JobStatus,
    cancellation_at_ms: Option<u64>,
}

/// Synthetic host state machine. Model VRAM is accounting, NOT measured memory.
/// A real backend must join/cancel tracked workers and confirm context disposal.
#[derive(Debug)]
pub struct MockComputeHost {
    id: HostId,
    server_cert_sha256: String,
    lease_ttl_ms: u64,
    grace_ms: u64,
    max_pending_jobs: usize,
    status: HostStatus,
    lifecycle_authority: LifecycleAuthority,
    /// Host-local mapping supplied by the authoritative coordinator/registry adapter.
    ledger_models: HashMap<ModelId, LedgerRuntimeBinding>,
    /// Core PR #12 forbids numeric ResourceId reuse for this ledger lifetime.
    used_ledger_resources: HashSet<LedgerResourceId>,
    used_owned_runtime_ids: HashSet<RuntimeId>,
    ledger_owners: HashMap<(ClientId, AppId, AppSessionId), LedgerOwnerId>,
    /// At-least-once delivery: retain events until the trusted adapter acks them.
    ledger_events: VecDeque<LedgerLeaseEvent>,
    next_event_sequence: u64,
    /// Immutable tombstones prevent lease-ID reuse and cross-client replay after close.
    used_lease_ids: HashMap<LeaseId, (ClientId, AppId, AppSessionId, TaskId)>,
    stopped_app_sessions: HashSet<(ClientId, AppId, AppSessionId)>,
    stopped_tasks: HashSet<(ClientId, AppId, AppSessionId, TaskId)>,
    paired: HashMap<ClientId, PairedClient>,
    grants: HashMap<(ClientId, AppId), AppGrant>,
    models: HashMap<ModelId, ModelState>,
    leases: HashMap<LeaseId, LeaseRecord>,
    jobs: HashMap<RequestId, JobRecord>,
    audit: VecDeque<AuditEvent>,
}

impl MockComputeHost {
    pub fn new(
        id: HostId,
        server_cert_sha256: String,
        lease_ttl_ms: u64,
        grace_ms: u64,
        max_pending_jobs: usize,
    ) -> Result<Self, Error> {
        if !valid_sha256_hex(&server_cert_sha256) || lease_ttl_ms == 0
            || grace_ms == 0 || max_pending_jobs == 0
        {
            return Err(Error::InvalidConfiguration);
        }
        Ok(Self {
            id, server_cert_sha256, lease_ttl_ms, grace_ms, max_pending_jobs,
            status: HostStatus::Accepting,
            lifecycle_authority: LifecycleAuthority::MockLocal,
            ledger_models: HashMap::new(),
            used_ledger_resources: HashSet::new(), used_owned_runtime_ids: HashSet::new(),
            ledger_owners: HashMap::new(),
            ledger_events: VecDeque::new(), next_event_sequence: 0,
            used_lease_ids: HashMap::new(),
            stopped_app_sessions: HashSet::new(), stopped_tasks: HashSet::new(),
            paired: HashMap::new(), grants: HashMap::new(), models: HashMap::new(),
            leases: HashMap::new(), jobs: HashMap::new(), audit: VecDeque::new(),
        })
    }

    fn log(
        &mut self, at_ms: u64, action: AuditAction,
        client: Option<ClientId>, app: Option<AppId>, model: Option<ModelId>,
        lease: Option<LeaseId>, request: Option<RequestId>, detail: &'static str,
    ) {
        if self.audit.len() == MAX_AUDIT_EVENTS { self.audit.pop_front(); }
        self.audit.push_back(AuditEvent {
            at_ms, action, client, app, model, lease, request, detail,
        });
    }

    pub fn audit(&self) -> &VecDeque<AuditEvent> { &self.audit }
    pub fn lifecycle_authority(&self) -> LifecycleAuthority { self.lifecycle_authority }
    /// Switch only before any lease/model load; coordinator mode forbids local unloading.
    pub fn set_lifecycle_authority(&mut self, authority: LifecycleAuthority)
        -> Result<(), Error>
    {
        if !self.used_lease_ids.is_empty()
            || self.models.values().any(|state| state.loaded_by_us)
        {
            return Err(Error::InvalidConfiguration);
        }
        self.lifecycle_authority = authority;
        Ok(())
    }

    /// Host-trusted mapping to the registry's stable model reference and core
    /// ResourceLedger's numeric ResourceId. Two models cannot alias one resource.
    pub fn bind_ledger_runtime(&mut self, binding: LedgerRuntimeBinding)
        -> Result<(), Error>
    {
        if !self.models.contains_key(&binding.model) {
            return Err(Error::UnknownModel);
        }
        if self.models.get(&binding.model).is_some_and(|m| m.loaded_by_us)
            || self.ledger_models.contains_key(&binding.model)
            || self.used_ledger_resources.contains(&binding.resource)
            || self.used_ledger_resources.len() >= MAX_LEDGER_RESOURCE_IDENTITIES
            || (self.models.get(&binding.model).is_some_and(|m| {
                m.registration.ownership == RuntimeOwnership::HostOwned
            }) && self.used_owned_runtime_ids.contains(&binding.runtime))
            || self.ledger_models.values().any(|b| b.runtime == binding.runtime)
        {
            return Err(Error::IdentifierConflict);
        }
        self.used_ledger_resources.insert(binding.resource);
        if self.models.get(&binding.model).is_some_and(|m| {
            m.registration.ownership == RuntimeOwnership::HostOwned
        }) {
            self.used_owned_runtime_ids.insert(binding.runtime.clone());
        }
        self.ledger_models.insert(binding.model.clone(), binding);
        Ok(())
    }

    /// Trusted coordinator only. PR #12 disallows reuse of ResourceId even after
    /// a successful unload or the final BorrowedExternal reference is removed.
    /// Assign a NEW resource (and a new owned runtime instance on reload)
    /// before another lease; never recycle an old resource or accept stale ACKs.
    pub fn rotate_ledger_runtime(
        &mut self, new_binding: LedgerRuntimeBinding,
    ) -> Result<(), Error> {
        if self.lifecycle_authority != LifecycleAuthority::CoordinatorLedger {
            return Err(Error::InvalidConfiguration);
        }
        let model = new_binding.model.clone();
        let state = self.models.get(&model).ok_or(Error::UnknownModel)?;
        if !state.ledger_resource_spent || state.loaded_by_us
            || state.awaiting_ledger_ack || self.model_lease_count(&model) != 0
            || self.ledger_events.iter().any(|event| event.model == model)
        {
            return Err(Error::LedgerCleanupPending);
        }
        self.ledger_models.get(&model).ok_or(Error::LedgerBindingRequired)?;
        if self.used_ledger_resources.contains(&new_binding.resource)
            || self.used_ledger_resources.len() >= MAX_LEDGER_RESOURCE_IDENTITIES
            || (state.registration.ownership == RuntimeOwnership::HostOwned
                && self.used_owned_runtime_ids.contains(&new_binding.runtime))
            || self.ledger_models.iter().any(|(other_model, binding)| {
                other_model != &model && binding.runtime == new_binding.runtime
            })
        {
            return Err(Error::IdentifierConflict);
        }
        self.used_ledger_resources.insert(new_binding.resource);
        if state.registration.ownership == RuntimeOwnership::HostOwned {
            self.used_owned_runtime_ids.insert(new_binding.runtime.clone());
        }
        self.ledger_models.insert(model.clone(), new_binding);
        if let Some(state) = self.models.get_mut(&model) {
            state.ledger_resource_spent = false;
        }
        Ok(())
    }

    /// The coordinator alone assigns per-app-launch OwnerId from its ledger.
    /// Device identity, app identity and app-session ID together are the key.
    pub fn bind_ledger_owner(
        &mut self, client: ClientId, app: AppId, session: AppSessionId,
        owner: LedgerOwnerId,
    ) -> Result<(), Error> {
        if !self.paired.contains_key(&client)
            || !self.grants.contains_key(&(client.clone(), app.clone()))
        {
            return Err(Error::PermissionDenied);
        }
        let key = (client, app, session);
        if self.ledger_owners.get(&key) == Some(&owner) { return Ok(()); }
        if self.ledger_owners.contains_key(&key)
            || self.ledger_owners.iter().any(|(k, id)| *id == owner && k != &key)
        {
            return Err(Error::IdentifierConflict);
        }
        self.ledger_owners.insert(key, owner);
        Ok(())
    }

    pub fn pending_ledger_events(&self) -> &VecDeque<LedgerLeaseEvent> {
        &self.ledger_events
    }
    /// Acknowledges one *delivered* event, not runtime cleanup. Coordinator
    /// must deduplicate by host+sequence in a durable authoritative adapter.
    pub fn acknowledge_ledger_event(&mut self, sequence: u64, now_ms: u64) -> bool {
        // No out-of-order acknowledgements: acquire must be applied before
        // release in the coordinator ResourceLedger. Retried ACK is idempotent.
        if self.ledger_events.front().is_some_and(|e| e.sequence == sequence) {
            self.ledger_events.pop_front();
            self.update_drained(now_ms);
            return true;
        }
        false
    }
    /// Invoked only after core ResourceLedger issued CleanupRequest, the owned
    /// backend verified unload, and complete_cleanup(resource, true) succeeded.
    /// The generation fences stale late acknowledgements against a reload.
    pub fn acknowledge_ledger_unload(
        &mut self, model: &ModelId, resource: LedgerResourceId,
        generation: u64, succeeded: bool, now_ms: u64,
    ) -> Result<(), Error> {
        if self.lifecycle_authority != LifecycleAuthority::CoordinatorLedger {
            return Err(Error::LedgerAckRejected);
        }
        let binding = self.ledger_models.get(model).ok_or(Error::LedgerBindingRequired)?;
        if binding.resource != resource || self.model_lease_count(model) != 0
            || self.ledger_events.iter().any(|event| event.model == *model)
        {
            // First confirm every acquire/release reached the authoritative ledger.
            // An old/unprocessed event must not silently be bypassed by an unload ACK.
            return Err(Error::LedgerAckRejected);
        }
        let state = self.models.get_mut(model).ok_or(Error::UnknownModel)?;
        if !state.awaiting_ledger_ack || state.generation != generation {
            return Err(Error::LedgerAckRejected);
        }
        if succeeded {
            state.awaiting_ledger_ack = false;
            if state.loaded_by_us {
                state.loaded_by_us = false;
                state.unload_count += 1;
                self.log(now_ms, AuditAction::ModelUnloaded, None, None, Some(model.clone()),
                    None, None, "mock received authoritative ledger/backend unload ack");
            }
            self.update_drained(now_ms);
        }
        Ok(())
    }
    pub fn model_generation(&self, model: &ModelId) -> Option<u64> {
        self.models.get(model).map(|state| state.generation)
    }
    pub fn awaiting_ledger_unload(&self, model: &ModelId) -> bool {
        self.models.get(model).is_some_and(|state| state.awaiting_ledger_ack)
    }

    fn emit_ledger_event(&mut self, action: LedgerLeaseAction, id: &LeaseId, lease: &LeaseRecord) {
        if self.lifecycle_authority != LifecycleAuthority::CoordinatorLedger { return; }
        let binding = self.ledger_models.get(&lease.model)
            .expect("ledger binding required before opening a lease");
        let owner = lease.ledger_owner.expect("ledger owner required before opening a lease");
        // Admission reserves two event slots and fences u64 sequence exhaustion.
        self.next_event_sequence += 1;
        self.ledger_events.push_back(LedgerLeaseEvent {
            sequence: self.next_event_sequence, action, host: self.id.clone(),
            client: lease.client.clone(), app: lease.app.clone(),
            app_session: lease.app_session.clone(), task: lease.task.clone(),
            model: lease.model.clone(), runtime: binding.runtime.clone(),
            owner, resource: binding.resource, lease: id.clone(),
        });
    }
    pub fn status(&self) -> HostStatus { self.status }
    pub fn lease_count(&self) -> usize { self.leases.len() }
    pub fn total_owned_vram_mib(&self) -> u64 {
        self.models.values().filter(|m| m.loaded_by_us)
            .map(|m| m.registration.estimated_vram_mib).sum()
    }
    pub fn model_load_counts(&self, id: &ModelId) -> Option<(u64, u64)> {
        self.models.get(id).map(|m| (m.load_count, m.unload_count))
    }
    pub fn model_lease_count(&self, id: &ModelId) -> usize {
        self.leases.values().filter(|lease| &lease.model == id).count()
    }
    pub fn lease_status(&self, id: &LeaseId) -> Option<LeaseStatus> {
        self.leases.get(id).map(|lease| lease.status)
    }
    pub fn job_status(&self, id: &RequestId) -> Option<JobStatus> {
        self.jobs.get(id).map(|job| job.status)
    }

    /// Host-local administrator operation: the UI approved the exact TLS client pin
    /// over an out-of-band verification channel. No pair code or credentials logged.
    pub fn pair_client(
        &mut self, approval: PairingApproval, now_ms: u64,
    ) -> Result<(), Error> {
        if !approval.user_approved || !valid_sha256_hex(&approval.client_cert_sha256)
        {
            return Err(Error::PairingRequired);
        }
        if let Some(existing) = self.paired.get(&approval.client) {
            if existing.cert_sha256 != approval.client_cert_sha256 {
                return Err(Error::IdentifierConflict);
            }
        }
        let client = approval.client;
        self.paired.insert(client.clone(), PairedClient {
            cert_sha256: approval.client_cert_sha256,
        });
        self.log(now_ms, AuditAction::Paired, Some(client), None, None, None, None,
            "user-approved certificate pin");
        Ok(())
    }

    /// Host-local administrator operation, never a client-editable grant.
    pub fn set_grant(
        &mut self, client: ClientId, app: AppId, grant: AppGrant, now_ms: u64,
    ) -> Result<(), Error> {
        grant.validate()?;
        if !self.paired.contains_key(&client) { return Err(Error::PairingRequired); }
        if grant.allowed_models.iter().any(|id| !self.models.contains_key(id)) {
            return Err(Error::UnknownModel);
        }
        // A permission downgrade cannot leave an already running forbidden
        // workload behind. Scope the drain to this device+app, not the host.
        let leased: Vec<_> = self.leases.iter().filter(|(_, lease)| {
            lease.client == client && lease.app == app
        }).map(|(id, lease)| (id.clone(), lease.model.clone())).collect();
        let over_count = leased.len() > grant.max_leases;
        let changed_jobs = self.jobs.values().any(|job| {
            job.client == client && job.app == app && !job.status.terminal()
                && (!grant.allowed_tasks.contains(&job.spec.task)
                    || job.spec.payload_bytes > grant.max_payload_bytes)
        });
        let forbidden: Vec<_> = leased.iter().filter(|(_, model)| {
            !grant.allowed_models.contains(model)
                || self.models.get(model).is_some_and(|state| {
                    state.registration.ownership == RuntimeOwnership::HostOwned
                        && state.registration.estimated_vram_mib > grant.max_model_vram_mib
                })
        }).map(|(id, _)| id.clone()).collect();
        self.grants.insert((client.clone(), app.clone()), grant);
        if over_count || changed_jobs {
            for (id, _) in leased { self.close_internal(&id, now_ms); }
        } else {
            for id in forbidden { self.close_internal(&id, now_ms); }
        }
        self.log(now_ms, AuditAction::Granted, Some(client), Some(app), None, None, None,
            "host policy grant");
        Ok(())
    }

    /// Host-local inventory registration, never accept raw client filesystem paths.
    pub fn register_model(&mut self, registration: ModelRegistration)
        -> Result<(), Error>
    {
        if self.models.contains_key(&registration.id) {
            return Err(Error::IdentifierConflict);
        }
        self.models.insert(registration.id.clone(), ModelState {
            registration, loaded_by_us: false, load_count: 0, unload_count: 0,
            generation: 0, awaiting_ledger_ack: false, ledger_resource_spent: false,
        });
        Ok(())
    }

    fn authenticate(&self, peer: &PeerEvidence) -> Result<(), Error> {
        if peer.security != ChannelSecurity::VerifiedMutualTls
            || peer.route == NetworkRoute::PublicInternet
        {
            return Err(Error::UnverifiedChannel);
        }
        if peer.host != self.id || peer.server_cert_sha256 != self.server_cert_sha256 {
            return Err(Error::IdentityMismatch);
        }
        if peer.app_authentication != AppAuthentication::VerifiedScopedCredential {
            return Err(Error::UnverifiedApplication);
        }
        let pair = self.paired.get(&peer.client).ok_or(Error::PairingRequired)?;
        if pair.cert_sha256 != peer.client_cert_sha256 {
            return Err(Error::IdentityMismatch);
        }
        if !self.grants.contains_key(&(peer.client.clone(), peer.app.clone())) {
            return Err(Error::PermissionDenied);
        }
        Ok(())
    }

    fn grant(&self, peer: &PeerEvidence) -> Result<&AppGrant, Error> {
        self.authenticate(peer)?;
        self.grants.get(&(peer.client.clone(), peer.app.clone()))
            .ok_or(Error::PermissionDenied)
    }

    pub fn open_lease(
        &mut self, peer: &PeerEvidence, id: LeaseId, model: ModelId, now_ms: u64,
    ) -> Result<(), Error> {
        self.tick(now_ms);
        if self.status != HostStatus::Accepting { return Err(Error::ShuttingDown); }
        let grant = self.grant(peer)?;
        let session = (peer.client.clone(), peer.app.clone(), peer.app_session.clone());
        if self.stopped_app_sessions.contains(&session)
            || self.stopped_tasks.contains(&(session.0, session.1, session.2, peer.task.clone()))
        {
            return Err(Error::LeaseExpired);
        }
        if !grant.allowed_models.contains(&model) { return Err(Error::PermissionDenied); }
        let reg = self.models.get(&model).ok_or(Error::UnknownModel)?.registration.clone();
        if !reg.location.executable() { return Err(Error::StagingNotVerified); }
        if reg.ownership == RuntimeOwnership::HostOwned
            && reg.estimated_vram_mib > grant.max_model_vram_mib
        {
            return Err(Error::QuotaExceeded);
        }
        if let Some(owner) = self.used_lease_ids.get(&id) {
            let expected = (peer.client.clone(), peer.app.clone(),
                peer.app_session.clone(), peer.task.clone());
            if owner != &expected { return Err(Error::NotOwner); }
            if let Some(existing) = self.leases.get(&id) {
                if existing.model == model && existing.status == LeaseStatus::Active {
                    // Retry after lost response is the SAME lease, never a second load.
                    return Ok(());
                }
                return Err(Error::IdentifierConflict);
            }
            // Tombstones are never reused, even after a TTL expiry or Quit.
            return Err(Error::LeaseExpired);
        }
        if self.used_lease_ids.len() >= MAX_LEASE_IDENTITIES {
            return Err(Error::RecordCapacityReached);
        }
        let ledger_owner = if self.lifecycle_authority == LifecycleAuthority::CoordinatorLedger {
            if !self.ledger_models.contains_key(&model) {
                return Err(Error::LedgerBindingRequired);
            }
            let key = (peer.client.clone(), peer.app.clone(), peer.app_session.clone());
            let owner = *self.ledger_owners.get(&key).ok_or(Error::LedgerBindingRequired)?;
            if self.ledger_events.len().saturating_add(
                self.leases.len().saturating_mul(2)).saturating_add(2) > MAX_LEDGER_EVENTS
                || self.next_event_sequence > u64::MAX - 2
            {
                return Err(Error::RecordCapacityReached);
            }
            Some(owner)
        } else { None };
        if self.models.get(&model).is_some_and(|m| m.awaiting_ledger_ack) {
            return Err(Error::LedgerCleanupPending);
        }
        if self.lifecycle_authority == LifecycleAuthority::CoordinatorLedger
            && self.models.get(&model).is_some_and(|m| m.ledger_resource_spent)
        {
            return Err(Error::LedgerBindingRequired);
        }
        let count = self.leases.values().filter(|lease| {
            lease.client == peer.client && lease.app == peer.app
        }).count();
        if count >= grant.max_leases { return Err(Error::QuotaExceeded); }
        let state = self.models.get_mut(&model).ok_or(Error::UnknownModel)?;
        let first_load = reg.ownership == RuntimeOwnership::HostOwned && !state.loaded_by_us;
        if first_load {
            let next_generation = state.generation.checked_add(1)
                .ok_or(Error::RecordCapacityReached)?;
            state.generation = next_generation;
            state.loaded_by_us = true;
            state.load_count += 1;
        }
        let record = LeaseRecord {
            client: peer.client.clone(), app: peer.app.clone(),
            app_session: peer.app_session.clone(), task: peer.task.clone(),
            model: model.clone(), ledger_owner,
            status: LeaseStatus::Active, expires_at_ms: now_ms.saturating_add(self.lease_ttl_ms),
            close_started_ms: None,
        };
        self.leases.insert(id.clone(), record.clone());
        self.used_lease_ids.insert(id.clone(), (
            peer.client.clone(), peer.app.clone(), peer.app_session.clone(), peer.task.clone()
        ));
        self.emit_ledger_event(LedgerLeaseAction::Acquired, &id, &record);
        if first_load {
            self.log(now_ms, AuditAction::ModelLoaded, None, None, Some(model.clone()),
                None, None, "synthetic host-owned runtime load");
        }
        self.log(now_ms, AuditAction::LeaseOpened, Some(peer.client.clone()),
            Some(peer.app.clone()), Some(model), Some(id), None, "per-client model lease");
        Ok(())
    }

    pub fn heartbeat(
        &mut self, peer: &PeerEvidence, id: &LeaseId, now_ms: u64,
    ) -> Result<(), Error> {
        self.tick(now_ms);
        self.authenticate(peer)?;
        let lease = self.leases.get_mut(id).ok_or(Error::UnknownLease)?;
        if lease.client != peer.client || lease.app != peer.app
            || lease.app_session != peer.app_session || lease.task != peer.task
        {
            return Err(Error::NotOwner);
        }
        if lease.status != LeaseStatus::Active { return Err(Error::LeaseExpired); }
        lease.expires_at_ms = now_ms.saturating_add(self.lease_ttl_ms);
        self.log(now_ms, AuditAction::LeaseRenewed, Some(peer.client.clone()),
            Some(peer.app.clone()), None, Some(id.clone()), None, "heartbeat");
        Ok(())
    }

    pub fn submit(
        &mut self, peer: &PeerEvidence, spec: RequestSpec, now_ms: u64,
    ) -> Result<JobStatus, Error> {
        self.tick(now_ms);
        let grant = self.grant(peer)?;
        if let Some(existing) = self.jobs.get(&spec.id) {
            if existing.client == peer.client && existing.app == peer.app
                && existing.app_session == peer.app_session && existing.task == peer.task
                && existing.spec == spec { return Ok(existing.status); }
            return Err(Error::IdentifierConflict);
        }
        if self.status != HostStatus::Accepting { return Err(Error::ShuttingDown); }
        if spec.payload_bytes > grant.max_payload_bytes {
            return Err(Error::RequestTooLarge);
        }
        if !grant.allowed_tasks.contains(&spec.task) { return Err(Error::PermissionDenied); }
        let lease = self.leases.get(&spec.lease).ok_or(Error::UnknownLease)?;
        if lease.client != peer.client || lease.app != peer.app
            || lease.app_session != peer.app_session || lease.task != peer.task
        {
            return Err(Error::NotOwner);
        }
        if lease.status != LeaseStatus::Active { return Err(Error::LeaseExpired); }
        if !grant.allowed_models.contains(&lease.model) {
            return Err(Error::PermissionDenied);
        }
        if self.jobs.len() >= MAX_JOB_RECORDS {
            return Err(Error::RecordCapacityReached);
        }
        let active = self.jobs.values().filter(|job| {
            job.client == peer.client && job.app == peer.app && !job.status.terminal()
        }).count();
        if active >= grant.max_nonterminal_jobs { return Err(Error::QuotaExceeded); }
        let queued = self.jobs.values().filter(|job| job.status == JobStatus::Queued).count();
        if queued >= self.max_pending_jobs { return Err(Error::QuotaExceeded); }
        self.jobs.insert(spec.id.clone(), JobRecord {
            client: peer.client.clone(), app: peer.app.clone(),
            app_session: peer.app_session.clone(), task: peer.task.clone(),
            spec: spec.clone(), status: JobStatus::Queued, cancellation_at_ms: None,
        });
        self.log(now_ms, AuditAction::JobQueued, Some(peer.client.clone()),
            Some(peer.app.clone()), None, Some(spec.lease), Some(spec.id), "queued");
        Ok(JobStatus::Queued)
    }

    /// Host executor only; deterministic lexicographic ordering in this fake queue.
    pub fn start_next_job(&mut self, now_ms: u64) -> Option<RequestId> {
        self.tick(now_ms);
        if self.status != HostStatus::Accepting { return None; }
        let next = self.jobs.iter().filter(|(_, j)| j.status == JobStatus::Queued)
            .map(|(id, _)| id.clone()).min()?;
        let job = self.jobs.get_mut(&next)?;
        job.status = JobStatus::Running;
        self.log(now_ms, AuditAction::JobStarted, None, None, None, None,
            Some(next.clone()), "synthetic worker started");
        Some(next)
    }

    /// Host executor only. A late success cannot resurrect a cancelled job.
    pub fn finish_job(
        &mut self, id: &RequestId, success: bool, now_ms: u64,
    ) -> Result<JobStatus, Error> {
        self.tick(now_ms);
        let job = self.jobs.get_mut(id).ok_or(Error::UnknownRequest)?;
        if job.status.terminal() { return Ok(job.status); }
        job.status = if job.status == JobStatus::Cancelling {
            JobStatus::Cancelled
        } else if success { JobStatus::Succeeded } else { JobStatus::Failed };
        let result = job.status;
        self.log(now_ms, AuditAction::JobFinished, None, None, None, None,
            Some(id.clone()), "synthetic backend completion");
        self.finalize_closed(now_ms);
        Ok(result)
    }

    fn cancel_internal(&mut self, id: &RequestId, now_ms: u64) {
        let Some(job) = self.jobs.get_mut(id) else { return; };
        if job.status.terminal() || job.status == JobStatus::Cancelling { return; }
        let queued = job.status == JobStatus::Queued;
        job.status = if queued { JobStatus::Cancelled } else { JobStatus::Cancelling };
        job.cancellation_at_ms = Some(now_ms);
        self.log(now_ms,
            if queued { AuditAction::JobCancelled }
                else { AuditAction::JobCancellationRequested },
            None, None, None, None, Some(id.clone()), "scoped request cancellation");
    }

    pub fn cancel_request(
        &mut self, peer: &PeerEvidence, id: &RequestId, now_ms: u64,
    ) -> Result<JobStatus, Error> {
        self.tick(now_ms);
        self.authenticate(peer)?;
        let job = self.jobs.get(id).ok_or(Error::UnknownRequest)?;
        if job.client != peer.client || job.app != peer.app
            || job.app_session != peer.app_session || job.task != peer.task
        {
            return Err(Error::NotOwner);
        }
        self.cancel_internal(id, now_ms);
        self.finalize_closed(now_ms);
        self.job_status(id).ok_or(Error::UnknownRequest)
    }

    fn close_internal(&mut self, id: &LeaseId, now_ms: u64) {
        let Some(lease) = self.leases.get_mut(id) else { return; };
        if lease.status == LeaseStatus::Closing { return; }
        lease.status = LeaseStatus::Closing;
        lease.close_started_ms = Some(now_ms);
        let client = lease.client.clone();
        let app = lease.app.clone();
        let model = lease.model.clone();
        self.log(now_ms, AuditAction::LeaseClosing, Some(client), Some(app),
            Some(model), Some(id.clone()), None, "reject new work; cancel and drain");
        let ids: Vec<_> = self.jobs.iter().filter(|(_, job)| job.spec.lease == *id
            && !job.status.terminal()).map(|(id, _)| id.clone()).collect();
        for request in ids { self.cancel_internal(&request, now_ms); }
        self.finalize_closed(now_ms);
    }

    pub fn close_lease(
        &mut self, peer: &PeerEvidence, id: &LeaseId, now_ms: u64,
    ) -> Result<(), Error> {
        self.tick(now_ms);
        self.authenticate(peer)?;
        let owner = self.used_lease_ids.get(id).ok_or(Error::UnknownLease)?;
        if owner != &(peer.client.clone(), peer.app.clone(),
            peer.app_session.clone(), peer.task.clone())
        {
            return Err(Error::NotOwner);
        }
        if self.leases.contains_key(id) {
            self.close_internal(id, now_ms);
        }
        // Idempotent only for the original lease owner, including after teardown.
        Ok(())
    }

    /// Trusted *device-level* shutdown/revocation callback only. This releases all
    /// apps on that device; a single app/socket close MUST call close_app/close_lease
    /// or allow its own lease TTL to expire, to preserve another app on the same Pi.
    /// Abrupt process death without a verified callback is recovered by lease TTL.
    pub fn close_client(&mut self, client: &ClientId, now_ms: u64) {
        let ids: Vec<_> = self.leases.iter().filter(|(_, lease)| &lease.client == client)
            .map(|(id, _)| id.clone()).collect();
        for id in ids { self.close_internal(&id, now_ms); }
        let scopes: Vec<_> = self.used_lease_ids.values().filter(|scope| &scope.0 == client)
            .map(|scope| (scope.0.clone(), scope.1.clone(), scope.2.clone()))
            .collect();
        self.stopped_app_sessions.extend(scopes);
    }

    /// Client-app Quit or Hard Stop; does NOT close another app on the same Pi.
    pub fn close_app(&mut self, peer: &PeerEvidence, now_ms: u64) -> Result<(), Error> {
        self.tick(now_ms);
        self.authenticate(peer)?;
        self.stopped_app_sessions.insert((
            peer.client.clone(), peer.app.clone(), peer.app_session.clone()
        ));
        let ids: Vec<_> = self.leases.iter().filter(|(_, lease)| {
            lease.client == peer.client && lease.app == peer.app
                && lease.app_session == peer.app_session
        }).map(|(id, _)| id.clone()).collect();
        for id in ids { self.close_internal(&id, now_ms); }
        Ok(())
    }

    /// One task session's STOP, preserving other Advisor/Agent tasks and apps.
    pub fn close_task(&mut self, peer: &PeerEvidence, now_ms: u64) -> Result<(), Error> {
        self.tick(now_ms);
        self.authenticate(peer)?;
        self.stopped_tasks.insert((
            peer.client.clone(), peer.app.clone(),
            peer.app_session.clone(), peer.task.clone()
        ));
        let ids: Vec<_> = self.leases.iter().filter(|(_, lease)| {
            lease.client == peer.client && lease.app == peer.app
                && lease.app_session == peer.app_session && lease.task == peer.task
        }).map(|(id, _)| id.clone()).collect();
        for id in ids { self.close_internal(&id, now_ms); }
        Ok(())
    }

    /// Host-local policy revocation, cancels only this app's affected leases.
    pub fn revoke_app(&mut self, client: &ClientId, app: &AppId, now_ms: u64) {
        self.grants.remove(&(client.clone(), app.clone()));
        let ids: Vec<_> = self.leases.iter().filter(|(_, lease)| {
            &lease.client == client && &lease.app == app
        }).map(|(id, _)| id.clone()).collect();
        for id in ids { self.close_internal(&id, now_ms); }
        let old_scopes: Vec<_> = self.used_lease_ids.values().filter(|scope| {
            &scope.0 == client && &scope.1 == app
        }).map(|scope| (scope.0.clone(), scope.1.clone(), scope.2.clone())).collect();
        self.stopped_app_sessions.extend(old_scopes);
        self.log(now_ms, AuditAction::Revoked, Some(client.clone()),
            Some(app.clone()), None, None, None, "host revoked one application grant");
    }

    /// Explicit host-owner command (Quit All TIHULU-Owned Services); not UI Quit.
    /// Real implementation must warn clients and supervise only owned workers.
    pub fn begin_host_drain(&mut self, now_ms: u64) {
        if self.status != HostStatus::Accepting { return; }
        self.status = HostStatus::Draining;
        self.log(now_ms, AuditAction::HostDraining, None, None, None, None, None,
            "host-owner requested drain");
        let ids: Vec<_> = self.leases.keys().cloned().collect();
        for id in ids { self.close_internal(&id, now_ms); }
        self.finalize_closed(now_ms);
    }

    /// Host timer. Expire orphan leases and force logical completion after grace.
    /// An actual executor MUST still prove its owned worker was joined/terminated.
    pub fn tick(&mut self, now_ms: u64) {
        let expired: Vec<_> = self.leases.iter().filter(|(_, lease)| {
            lease.status == LeaseStatus::Active && now_ms >= lease.expires_at_ms
        }).map(|(id, _)| id.clone()).collect();
        for id in expired { self.close_internal(&id, now_ms); }

        let forced: Vec<_> = self.jobs.iter().filter(|(_, job)| {
            job.status == JobStatus::Cancelling
                && (job.cancellation_at_ms.is_some_and(|t| {
                    now_ms >= t.saturating_add(self.grace_ms)
                }) || self.leases.get(&job.spec.lease).and_then(|lease| {
                    lease.close_started_ms
                }).is_some_and(|t| now_ms >= t.saturating_add(self.grace_ms)))
        }).map(|(id, _)| id.clone()).collect();
        for id in forced {
            if let Some(job) = self.jobs.get_mut(&id) {
                job.status = JobStatus::Cancelled;
            }
            self.log(now_ms, AuditAction::ForceCancelled, None, None, None, None,
                Some(id), "mock bounded drain: backend termination NOT proven");
        }
        self.finalize_closed(now_ms);
    }

    fn finalize_closed(&mut self, now_ms: u64) {
        let releasable: Vec<_> = self.leases.iter().filter(|(id, lease)| {
            lease.status == LeaseStatus::Closing && !self.jobs.values().any(|job| {
                job.spec.lease == **id && !job.status.terminal()
            })
        }).map(|(id, _)| id.clone()).collect();

        let mut affected = HashSet::new();
        for id in releasable {
            if let Some(lease) = self.leases.remove(&id) {
                affected.insert(lease.model.clone());
                self.emit_ledger_event(LedgerLeaseAction::Released, &id, &lease);
                self.log(now_ms, AuditAction::LeaseClosed, Some(lease.client),
                    Some(lease.app), Some(lease.model), Some(id), None,
                    "lease ended; ledger event queued if coordinator-managed");
            }
        }
        for model in affected {
            if self.leases.values().any(|lease| lease.model == model) { continue; }
            if let Some(state) = self.models.get_mut(&model) {
                if self.lifecycle_authority == LifecycleAuthority::CoordinatorLedger {
                    state.ledger_resource_spent = true;
                }
                if state.loaded_by_us {
                    if self.lifecycle_authority == LifecycleAuthority::CoordinatorLedger {
                        // The core ledger, not this host mock or Registry, decides
                        // whether the final release produces a CleanupRequest.
                        state.awaiting_ledger_ack = true;
                    } else {
                        state.loaded_by_us = false;
                        state.unload_count += 1;
                        self.log(now_ms, AuditAction::ModelUnloaded, None, None,
                            Some(model), None, None, "standalone mock last-lease unload");
                    }
                }
            }
        }
        self.update_drained(now_ms);
    }

    fn update_drained(&mut self, now_ms: u64) {
        if self.status != HostStatus::Draining || !self.leases.is_empty() { return; }
        if self.lifecycle_authority == LifecycleAuthority::CoordinatorLedger
            && (!self.ledger_events.is_empty()
                || self.models.values().any(|m| m.awaiting_ledger_ack))
        {
            // Logical lease release is not confirmation of physical GPU unload.
            return;
        }
        self.status = HostStatus::Drained;
        self.log(now_ms, AuditAction::HostDrained, None, None, None, None, None,
            "all mock lease and ledger cleanup acknowledgments completed");
    }
}
