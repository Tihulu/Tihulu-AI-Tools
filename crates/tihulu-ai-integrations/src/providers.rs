//! Opt-in Jan Desktop and Ollama *contracts*, not live connections.
//!
//! There is deliberately no HTTP client, MCP listener, provider configuration
//! writer, process supervisor, model loader or foreign-process shutdown API.
//! Trusted host code must authenticate callers and collect explicit user consent
//! before constructing grants. The ports below are exercised only by inert
//! in-memory fake servers in this milestone.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use crate::lifecycle::{
    CancellationToken, LifecycleSupervisor, Ownership, ResourceHandle, ResourceKind, SessionState,
    ShutdownReason, ShutdownReport,
};
use crate::typed::{AppId, ClientId, JobId, OwnerSessionId, RegistryModelId, TaskKind};

const MAX_DISCOVERED_MODELS: usize = 128;
const MAX_GRANTS: usize = 128;
const MAX_APPROVALS: usize = 256;
const MAX_JOBS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProviderKind {
    JanDesktop,
    Ollama,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderStatus {
    /// No user permission; the provider was not probed.
    AuthorizationRequired,
    /// An authorized status probe says the provider cannot be reached.
    Unavailable,
    /// Read-only discovery and approved request handling are possible.
    Ready,
    /// Metadata may be readable, but execution is not considered safe.
    Degraded,
    /// The application session has been stopped.
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderError {
    AuthorizationRequired,
    InvalidGrant,
    InvalidRequest,
    InvalidApproval,
    ApprovalRequired,
    SessionStopped,
    SessionCollision,
    ProviderUnavailable,
    ProviderMismatch,
    ModelNotGranted,
    ModelNotVerified,
    TaskNotGranted,
    RequestTooLarge,
    DuplicateJob,
    UnknownJob,
    BackendFailed,
    CapacityExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderCaller {
    pub client: ClientId,
    pub app: AppId,
    pub session: OwnerSessionId,
}

/// Validated opaque provider model label; not a path, HTTP URL or credential.
/// The Registry's *separate* stable model reference remains authoritative.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProviderModelId(String);

impl ProviderModelId {
    pub fn new(value: impl Into<String>) -> Result<Self, ProviderError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || value.contains("..")
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return Err(ProviderError::InvalidRequest);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelEvidence {
    /// A provider response advertised this model; not proof of runnable inference.
    ListedOnly,
    /// A future separately trusted runtime adapter has verified compatibility.
    VerifiedRunnable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderModel {
    pub provider: ProviderKind,
    pub registry_model: RegistryModelId,
    pub provider_model: ProviderModelId,
    pub tasks: BTreeSet<TaskKind>,
    pub evidence: ModelEvidence,
}

/// The read-only port must be implemented by a future separately authorized,
/// bounded provider adapter. This crate ships no network implementation.
/// Implementations MUST strip paths, tokens, transcripts and private metadata.
pub trait ProviderDiscoveryPort: Send + Sync {
    fn status(&self, provider: ProviderKind) -> ProviderStatus;
    fn list_models(&self, provider: ProviderKind) -> Result<Vec<ProviderModel>, ProviderError>;
}

/// Owns *only* a client's particular request/stream, never a Jan/Ollama
/// process, foreign GPU context, model cache or system service.
/// prepare_owned_request is inert: it cannot open sockets, allocate models,
/// start a process or change a provider configuration. An owned handle is
/// registered before dispatch_mock is permitted to submit a request.
///
/// There is no implementation of this port for a real Jan/Ollama installation.
pub trait ProviderExecutionPort: Send + Sync {
    fn prepare_owned_request(
        &self,
        caller: &ProviderCaller,
        action: &ProviderAction,
    ) -> Result<Arc<dyn ResourceHandle>, ProviderError>;

    fn dispatch_mock(
        &self,
        caller: &ProviderCaller,
        action: &ProviderAction,
        cancellation: &CancellationToken,
    ) -> Result<(), ProviderError>;
}

/// The user must authorize a *specific* provider and app/session before even
/// a status probe. No wildcard model permission is represented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderGrant {
    pub read_only_discovery: bool,
    pub allowed_models: BTreeSet<RegistryModelId>,
    pub allowed_tasks: BTreeSet<TaskKind>,
    pub max_request_bytes: u64,
}

impl ProviderGrant {
    fn allows(&self, action: &ProviderAction) -> Result<(), ProviderError> {
        if !self.allowed_models.contains(&action.registry_model) {
            return Err(ProviderError::ModelNotGranted);
        }
        if !self.allowed_tasks.contains(&action.task) {
            return Err(ProviderError::TaskNotGranted);
        }
        if action.input_bytes == 0 || action.input_bytes > self.max_request_bytes {
            return Err(ProviderError::RequestTooLarge);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAction {
    pub provider: ProviderKind,
    pub registry_model: RegistryModelId,
    pub provider_model: ProviderModelId,
    pub task: TaskKind,
    pub job: JobId,
    /// The metadata size only. No prompt, transcript or payload is retained.
    pub input_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ActionTicket(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Accepted,
    Cancelling,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobReceipt {
    pub job: JobId,
    pub provider: ProviderKind,
    pub state: JobState,
}

struct TrackedJob {
    provider: ProviderKind,
    cancellation: CancellationToken,
    handle: Arc<dyn ResourceHandle>,
    state: JobState,
}

/// In-memory policy and cancellation bookkeeping only. The Rust host is the
/// trust boundary: MCP JSON/Jan/LLM content MUST NOT call approve_connection,
/// approve_action or authorize another caller's stop. Session IDs must be
/// authenticated and unique across independent app/client lifetimes.
pub struct ProviderController<D: ProviderDiscoveryPort, E: ProviderExecutionPort> {
    discovery: D,
    execution: E,
    sessions: BTreeMap<OwnerSessionId, ProviderCaller>,
    grants: BTreeMap<(ProviderCaller, ProviderKind), ProviderGrant>,
    approvals: BTreeMap<ActionTicket, (ProviderCaller, ProviderAction)>,
    jobs: BTreeMap<(ProviderCaller, JobId), TrackedJob>,
    next_ticket: u64,
}

impl<D: ProviderDiscoveryPort, E: ProviderExecutionPort> ProviderController<D, E> {
    pub fn new(discovery: D, execution: E) -> Self {
        Self {
            discovery,
            execution,
            sessions: BTreeMap::new(),
            grants: BTreeMap::new(),
            approvals: BTreeMap::new(),
            jobs: BTreeMap::new(),
            next_ticket: 0,
        }
    }

    fn require_running(
        &self,
        caller: &ProviderCaller,
        lifecycle: &LifecycleSupervisor,
    ) -> Result<(), ProviderError> {
        if lifecycle.state(caller.session.as_str()) != Some(SessionState::Running) {
            return Err(ProviderError::SessionStopped);
        }
        if self.sessions.get(&caller.session) != Some(caller) {
            return Err(ProviderError::AuthorizationRequired);
        }
        Ok(())
    }

    fn require_grant(
        &self,
        caller: &ProviderCaller,
        provider: ProviderKind,
    ) -> Result<&ProviderGrant, ProviderError> {
        self.grants
            .get(&(caller.clone(), provider))
            .ok_or(ProviderError::AuthorizationRequired)
    }

    /// TRUSTED HOST ONLY: the host UI must have just recorded explicit user
    /// consent. "false" must not probe, connect to or enumerate the provider.
    /// The caller identity is derived from authenticated host-side session
    /// records, not from an MCP tool argument.
    pub fn approve_connection(
        &mut self,
        caller: ProviderCaller,
        provider: ProviderKind,
        user_approved: bool,
        grant: ProviderGrant,
        lifecycle: &LifecycleSupervisor,
    ) -> Result<(), ProviderError> {
        if !user_approved {
            return Err(ProviderError::AuthorizationRequired);
        }
        if lifecycle.state(caller.session.as_str()) != Some(SessionState::Running) {
            return Err(ProviderError::SessionStopped);
        }
        if grant.max_request_bytes == 0 {
            return Err(ProviderError::InvalidGrant);
        }
        if self.grants.len() >= MAX_GRANTS && !self.grants.contains_key(&(caller.clone(), provider))
        {
            return Err(ProviderError::CapacityExceeded);
        }
        if let Some(existing) = self.sessions.get(&caller.session) {
            if existing != &caller {
                return Err(ProviderError::SessionCollision);
            }
        } else if self.sessions.len() >= MAX_GRANTS {
            return Err(ProviderError::CapacityExceeded);
        }
        // Renewing provider consent invalidates approvals from its previous
        // grant generation; one-shot actions cannot survive policy changes.
        self.approvals.retain(|_, (approved_caller, action)| {
            approved_caller != &caller || action.provider != provider
        });
        self.sessions.insert(caller.session.clone(), caller.clone());
        self.grants.insert((caller, provider), grant);
        // Deliberately no provider status probe and no connection here.
        Ok(())
    }

    /// Returns AuthorizationRequired without probing an unauthorized provider.
    pub fn provider_status(
        &self,
        caller: &ProviderCaller,
        provider: ProviderKind,
        lifecycle: &LifecycleSupervisor,
    ) -> ProviderStatus {
        if lifecycle.state(caller.session.as_str()) != Some(SessionState::Running) {
            return ProviderStatus::Stopped;
        }
        if self.sessions.get(&caller.session) != Some(caller)
            || !self.grants.contains_key(&(caller.clone(), provider))
        {
            return ProviderStatus::AuthorizationRequired;
        }
        self.discovery.status(provider)
    }

    pub fn list_models(
        &self,
        caller: &ProviderCaller,
        provider: ProviderKind,
        lifecycle: &LifecycleSupervisor,
    ) -> Result<Vec<ProviderModel>, ProviderError> {
        self.require_running(caller, lifecycle)?;
        if !self.require_grant(caller, provider)?.read_only_discovery {
            return Err(ProviderError::AuthorizationRequired);
        }
        match self.discovery.status(provider) {
            ProviderStatus::Ready | ProviderStatus::Degraded => {}
            _ => return Err(ProviderError::ProviderUnavailable),
        }
        let models = self.discovery.list_models(provider)?;
        if models.len() > MAX_DISCOVERED_MODELS {
            return Err(ProviderError::CapacityExceeded);
        }
        let mut seen = BTreeSet::new();
        for model in &models {
            if model.provider != provider || model.tasks.is_empty() {
                return Err(ProviderError::ProviderMismatch);
            }
            if !seen.insert(model.registry_model.clone()) {
                // A Registry reference must not point to two distinct provider
                // entries in the same discovery response.
                return Err(ProviderError::ProviderMismatch);
            }
        }
        Ok(models)
    }

    /// TRUSTED HOST ONLY: one approval is bound to the entire immutable action
    /// and authenticated client/app/session, not just to a provider name.
    pub fn approve_action(
        &mut self,
        caller: &ProviderCaller,
        action: &ProviderAction,
        user_approved: bool,
        lifecycle: &LifecycleSupervisor,
    ) -> Result<ActionTicket, ProviderError> {
        if !user_approved {
            return Err(ProviderError::AuthorizationRequired);
        }
        self.require_running(caller, lifecycle)?;
        self.require_grant(caller, action.provider)?
            .allows(action)?;
        if self.approvals.len() >= MAX_APPROVALS {
            return Err(ProviderError::CapacityExceeded);
        }
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or(ProviderError::CapacityExceeded)?;
        let ticket = ActionTicket(self.next_ticket);
        self.approvals
            .insert(ticket, (caller.clone(), action.clone()));
        Ok(ticket)
    }

    /// The only submit contract. In this milestone dispatch_mock is an offline
    /// fake-server call, never an inference request to Jan/Ollama.
    pub fn submit_mock(
        &mut self,
        caller: &ProviderCaller,
        action: &ProviderAction,
        ticket: Option<ActionTicket>,
        lifecycle: &mut LifecycleSupervisor,
        cancellation: CancellationToken,
    ) -> Result<JobReceipt, ProviderError> {
        self.require_running(caller, lifecycle)?;
        let grant = self.require_grant(caller, action.provider)?;
        grant.allows(action)?;
        let ticket = ticket.ok_or(ProviderError::ApprovalRequired)?;
        match self.approvals.get(&ticket) {
            Some((approved_caller, approved_action))
                if approved_caller == caller && approved_action == action => {}
            _ => return Err(ProviderError::InvalidApproval),
        }
        // Consume before any backend lookup or side effect. No retry/replay.
        self.approvals.remove(&ticket);
        if cancellation.is_cancelled() {
            return Err(ProviderError::SessionStopped);
        }
        if self.discovery.status(action.provider) != ProviderStatus::Ready {
            return Err(ProviderError::ProviderUnavailable);
        }
        let models = self.list_models(caller, action.provider, lifecycle)?;
        let matched = models.iter().find(|model| {
            model.registry_model == action.registry_model
                && model.provider_model == action.provider_model
                && model.provider == action.provider
                && model.tasks.contains(&action.task)
        });
        match matched {
            Some(model) if model.evidence == ModelEvidence::VerifiedRunnable => {}
            _ => return Err(ProviderError::ModelNotVerified),
        }
        let key = (caller.clone(), action.job.clone());
        if self.jobs.contains_key(&key) {
            return Err(ProviderError::DuplicateJob);
        }
        if self.jobs.len() >= MAX_JOBS {
            return Err(ProviderError::CapacityExceeded);
        }

        // Handle creation is inert. Track the *client-owned request* BEFORE
        // backend dispatch so the host can cancel the handle on app Stop.
        let handle = self.execution.prepare_owned_request(caller, action)?;
        lifecycle
            .track(
                caller.session.as_str(),
                Ownership::AppOwned,
                ResourceKind::McpTask,
                cancellation.clone(),
                handle.clone(),
            )
            .map_err(|_| ProviderError::SessionStopped)?;
        if cancellation.is_cancelled() {
            handle.request_cancel();
            return Err(ProviderError::SessionStopped);
        }
        if let Err(error) = self.execution.dispatch_mock(caller, action, &cancellation) {
            cancellation.cancel();
            handle.request_cancel();
            return Err(error);
        }
        self.jobs.insert(
            key,
            TrackedJob {
                provider: action.provider,
                cancellation,
                handle,
                state: JobState::Accepted,
            },
        );
        Ok(JobReceipt {
            job: action.job.clone(),
            provider: action.provider,
            state: JobState::Accepted,
        })
    }

    /// An authenticated caller can cancel only their own specific request,
    /// not another app/client or the borrowed provider process.
    pub fn cancel_owned(
        &mut self,
        caller: &ProviderCaller,
        job: &JobId,
    ) -> Result<bool, ProviderError> {
        let record = self
            .jobs
            .get_mut(&(caller.clone(), job.clone()))
            .ok_or(ProviderError::UnknownJob)?;
        if record.state == JobState::Cancelling {
            return Ok(false);
        }
        record.state = JobState::Cancelling;
        record.cancellation.cancel();
        record.handle.request_cancel();
        Ok(true)
    }

    /// Revocation stops admission and *this provider's* owned requests.
    /// It does not close Jan/Ollama, other provider grants or other sessions.
    pub fn revoke_provider(&mut self, caller: &ProviderCaller, provider: ProviderKind) {
        self.grants.remove(&(caller.clone(), provider));
        self.approvals
            .retain(|_, (owner, action)| owner != caller || action.provider != provider);
        for ((owner, _), job) in &mut self.jobs {
            if owner == caller && job.provider == provider {
                job.state = JobState::Cancelling;
                job.cancellation.cancel();
                job.handle.request_cancel();
            }
        }
    }

    /// STOP/QUIT belongs to the session owner. Existing independent Jan and
    /// Ollama instances remain untouched; only tracked request handles close.
    pub fn stop_session(
        &mut self,
        caller: &ProviderCaller,
        reason: ShutdownReason,
        lifecycle: &mut LifecycleSupervisor,
        grace: Duration,
    ) -> Result<ShutdownReport, ProviderError> {
        if self.sessions.get(&caller.session) != Some(caller) {
            return Err(ProviderError::AuthorizationRequired);
        }
        self.grants.retain(|(owner, _), _| owner != caller);
        self.approvals.retain(|_, (owner, _)| owner != caller);
        let report = lifecycle
            .shutdown(caller.session.as_str(), reason, grace)
            .map_err(|_| ProviderError::SessionStopped)?;
        if report.state == SessionState::Stopped {
            self.jobs.retain(|(owner, _), _| owner != caller);
            // Keep a bounded owner/session tombstone for idempotent Stop,
            // preventing reuse of a previously authenticated session ID.
        }
        Ok(report)
    }

    pub fn active_mock_jobs(&self, caller: &ProviderCaller) -> usize {
        self.jobs
            .keys()
            .filter(|(owner, _)| owner == caller)
            .count()
    }
}
