//! Deterministic in-memory host supervisor. No GPU allocations or network sockets.
//! Host-administrative methods are NOT exposed as remote RPCs.
use crate::protocol::*;
use std::collections::{HashMap, HashSet, VecDeque};

const MAX_JOB_RECORDS: usize = 10_000;
const MAX_AUDIT_EVENTS: usize = 1_024;

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
}
#[derive(Clone, Debug)]
struct LeaseRecord {
    client: ClientId,
    app: AppId,
    model: ModelId,
    status: LeaseStatus,
    expires_at_ms: u64,
    close_started_ms: Option<u64>,
}
#[derive(Clone, Debug)]
struct JobRecord {
    client: ClientId,
    app: AppId,
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
        self.grants.insert((client.clone(), app.clone()), grant);
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
        if !grant.allowed_models.contains(&model) { return Err(Error::PermissionDenied); }
        let reg = self.models.get(&model).ok_or(Error::UnknownModel)?.registration.clone();
        if !reg.location.executable() { return Err(Error::StagingNotVerified); }
        if reg.ownership == RuntimeOwnership::HostOwned
            && reg.estimated_vram_mib > grant.max_model_vram_mib
        {
            return Err(Error::QuotaExceeded);
        }
        if self.leases.contains_key(&id) { return Err(Error::IdentifierConflict); }
        let count = self.leases.values().filter(|lease| {
            lease.client == peer.client && lease.app == peer.app
        }).count();
        if count >= grant.max_leases { return Err(Error::QuotaExceeded); }
        let state = self.models.get_mut(&model).ok_or(Error::UnknownModel)?;
        let first_load = reg.ownership == RuntimeOwnership::HostOwned && !state.loaded_by_us;
        if first_load {
            state.loaded_by_us = true;
            state.load_count += 1;
        }
        self.leases.insert(id.clone(), LeaseRecord {
            client: peer.client.clone(), app: peer.app.clone(), model: model.clone(),
            status: LeaseStatus::Active, expires_at_ms: now_ms.saturating_add(self.lease_ttl_ms),
            close_started_ms: None,
        });
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
        if lease.client != peer.client || lease.app != peer.app {
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
                && existing.spec == spec { return Ok(existing.status); }
            return Err(Error::IdentifierConflict);
        }
        if self.status != HostStatus::Accepting { return Err(Error::ShuttingDown); }
        if spec.payload_bytes > grant.max_payload_bytes {
            return Err(Error::RequestTooLarge);
        }
        if !grant.allowed_tasks.contains(&spec.task) { return Err(Error::PermissionDenied); }
        let lease = self.leases.get(&spec.lease).ok_or(Error::UnknownLease)?;
        if lease.client != peer.client || lease.app != peer.app {
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
            client: peer.client.clone(), app: peer.app.clone(), spec: spec.clone(),
            status: JobStatus::Queued, cancellation_at_ms: None,
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
        if job.client != peer.client || job.app != peer.app {
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
        if let Some(lease) = self.leases.get(id) {
            if lease.client != peer.client || lease.app != peer.app {
                return Err(Error::NotOwner);
            }
            self.close_internal(id, now_ms);
        }
        // Idempotent release: already released by this session is a no-op.
        Ok(())
    }

    /// Trusted transport callback on a closed socket, or client Quit. The host is preserved.
    /// Abrupt process death without a close callback is recovered by lease TTL.
    pub fn close_client(&mut self, client: &ClientId, now_ms: u64) {
        let ids: Vec<_> = self.leases.iter().filter(|(_, lease)| &lease.client == client)
            .map(|(id, _)| id.clone()).collect();
        for id in ids { self.close_internal(&id, now_ms); }
    }

    /// Client-app Quit or Hard Stop; does NOT close another app on the same Pi.
    pub fn close_app(&mut self, peer: &PeerEvidence, now_ms: u64) -> Result<(), Error> {
        self.tick(now_ms);
        self.authenticate(peer)?;
        let ids: Vec<_> = self.leases.iter().filter(|(_, lease)| {
            lease.client == peer.client && lease.app == peer.app
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
            job.status == JobStatus::Cancelling && job.cancellation_at_ms.is_some_and(|t| {
                now_ms >= t.saturating_add(self.grace_ms)
            })
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
                self.log(now_ms, AuditAction::LeaseClosed, Some(lease.client),
                    Some(lease.app), Some(lease.model), Some(id), None,
                    "client-owned resources fully released in mock");
            }
        }
        for model in affected {
            if self.leases.values().any(|lease| lease.model == model) { continue; }
            if let Some(state) = self.models.get_mut(&model) {
                if state.loaded_by_us {
                    state.loaded_by_us = false;
                    state.unload_count += 1;
                    self.log(now_ms, AuditAction::ModelUnloaded, None, None,
                        Some(model), None, None, "last owned lease: mock context unload");
                }
            }
        }
        if self.status == HostStatus::Draining && self.leases.is_empty() {
            self.status = HostStatus::Drained;
            self.log(now_ms, AuditAction::HostDrained, None, None, None, None, None,
                "all mock leases drained, not a real process exit");
        }
    }
}
