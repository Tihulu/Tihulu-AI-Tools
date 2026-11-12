//! Phase 2: a transport-neutral *typed* boundary from Jan's untrusted MCP
//! request through a real app adapter, read-only Registry and authorized Host.
//! No actual Registry, Jan, GPU, network, SDR or process adapter is installed.
//!
//! Stable IDs are not filesystem paths, endpoints, JWTs or credentials. A
//! future production host ingress must derive trusted app/client identities
//! from verified auth and never accept HostRoute as caller-supplied evidence.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::bridge::{
    AppAdapter, Arguments, BridgeError, Capability, ToolRequest, ToolResponse,
};
use crate::lifecycle::{
    CancellationToken, LifecycleSupervisor, Ownership, ResourceHandle, ResourceKind,
};

fn valid_identifier(value: &str) -> bool {
    value.len() <= 128
        && value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':')
        })
        && !value.contains("..")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryError {
    InvalidIdentifier,
    MissingRegistryReference,
    NotCompatible,
    HostNotAuthorized,
    InvalidHostRoute,
    PrepareFailed,
    DispatchFailed,
}

macro_rules! typed_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(String);
        impl $name {
            pub fn new(raw: impl Into<String>) -> Result<Self, BoundaryError> {
                let raw = raw.into();
                if !valid_identifier(&raw) {
                    return Err(BoundaryError::InvalidIdentifier);
                }
                Ok(Self(raw))
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}

typed_id!(AppId);
typed_id!(ClientId);
typed_id!(OwnerSessionId);
typed_id!(RegistryModelId);
typed_id!(HostId);
typed_id!(HostModelId);
typed_id!(JobId);

/// Trusted host-derived request identity: future MCP transport must bind client
/// to an authenticated session and not accept these fields from chat JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallerIdentity {
    pub client: ClientId,
    pub app: AppId,
    pub owner_session: OwnerSessionId,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TaskKind {
    Chat,
    Embeddings,
    Transcription,
    Translation,
    Vision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryOwner {
    AiTools,
    JanDesktop,
    Ollama,
    HuggingFace,
    UserExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryVerification {
    /// Merely observed; never proof that a model can run on a given host.
    Candidate,
    RuntimeVerified,
    Incompatible,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryRecord {
    pub id: RegistryModelId,
    pub owner: RegistryOwner,
    pub verification: RegistryVerification,
    pub supported_tasks: BTreeSet<TaskKind>,
}

/// All methods are read-only. The trusted Registry authorizes app-specific
/// access before returning records. Results MUST NOT contain foreign paths,
/// local API tokens, Jan chats or user-private model file content.
pub trait RegistryReader: Send + Sync {
    fn list_visible(&self, app: &AppId) -> Vec<RegistryRecord>;
    fn resolve_visible(&self, app: &AppId, model: &RegistryModelId)
        -> Option<RegistryRecord>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRuntimeOwner {
    HostOwned,
    BorrowedExternal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostRoute {
    /// Host model ID is independent of the Registry reference and Pi path.
    pub host: HostId,
    pub host_model: HostModelId,
    pub registry_model: RegistryModelId,
    pub task: TaskKind,
    pub runtime_owner: HostRuntimeOwner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostTask {
    pub task: TaskKind,
    pub registry_model: RegistryModelId,
    pub host: HostId,
    pub job: JobId,
}

/// Must be implemented by a trusted, authenticated host-client backend.
/// authorize_route MUST check host pairing, client/app identity, per-app
/// task/model grants, compatibility and quota; no unverified caller evidence.
/// prepare_job MUST have no I/O or runtime resource allocation: create
/// only an inert, owner-bound cancellation handle.
/// Once tracked, dispatch_job may open a client-owned host lease.
/// The handle MUST cancel/close only the scoped client lease, never unload
/// a model by itself or terminate an independent GPU host or Jan/Ollama.
pub trait HostGateway: Send + Sync {
    fn authorize_route(
        &self,
        caller: &CallerIdentity,
        request: &HostTask,
    ) -> Result<HostRoute, BoundaryError>;

    fn prepare_job(
        &self,
        caller: &CallerIdentity,
        route: &HostRoute,
        job: &JobId,
    ) -> Result<Arc<dyn ResourceHandle>, BoundaryError>;

    fn dispatch_job(
        &self,
        caller: &CallerIdentity,
        route: &HostRoute,
        job: &JobId,
        cancellation: &CancellationToken,
    ) -> Result<(), BoundaryError>;
}

/// Explicitly registered by trusted TIHULU host code only after real adapter
/// availability/capability verification. Never auto-created from model paths.
pub struct TypedAppAdapter {
    app: AppId,
    available: bool,
    advertised: BTreeSet<Capability>,
    registry: Arc<dyn RegistryReader>,
    host: Arc<dyn HostGateway>,
}

impl TypedAppAdapter {
    pub fn new(
        app: AppId,
        available: bool,
        verified_capabilities: impl IntoIterator<Item = Capability>,
        registry: Arc<dyn RegistryReader>,
        host: Arc<dyn HostGateway>,
    ) -> Self {
        let advertised = verified_capabilities
            .into_iter()
            .filter(|c| {
                matches!(
                    c,
                    Capability::ListModels
                        | Capability::FindModelForTask
                        | Capability::GetAppStatus
                        | Capability::RequestMediaTask
                )
            })
            .collect();
        Self {
            app,
            available,
            advertised,
            registry,
            host,
        }
    }

    fn visible_records(&self, task: Option<TaskKind>) -> Vec<RegistryRecord> {
        let mut records = self.registry.list_visible(&self.app);
        if let Some(task) = task {
            records.retain(|r| r.supported_tasks.contains(&task));
        }
        records
    }

    /// Bounded, path-free output. This is only a mock presentation; future
    /// MCP JSON-RPC must use a reviewed structured schema and size limit.
    fn summarize_refs(records: &[RegistryRecord]) -> String {
        const MAX_VISIBLE: usize = 16;
        records
            .iter()
            .take(MAX_VISIBLE)
            .map(|r| {
                format!(
                    "{} ({:?}, {:?})",
                    r.id.as_str(),
                    r.owner,
                    r.verification
                )
            })
            .collect::<Vec<_>>()
            .join("; ")
    }

    fn start_host_task(
        &self,
        caller: &CallerIdentity,
        request: &HostTask,
        cancellation: &CancellationToken,
        lifecycle: &mut LifecycleSupervisor,
    ) -> Result<ToolResponse, BridgeError> {
        let session_id = caller.owner_session.as_str();
        let record = self
            .registry
            .resolve_visible(&self.app, &request.registry_model)
            .ok_or(BridgeError::AdapterFailure)?;
        if record.id != request.registry_model
            || matches!(
                record.verification,
                RegistryVerification::Missing | RegistryVerification::Incompatible
            )
            || !record.supported_tasks.contains(&request.task)
        {
            return Err(BridgeError::AdapterFailure);
        }

        // Candidate status is NOT proof of runnable local inference. A
        // remote route requires independently verified host-side authority.
        let route = self
            .host
            .authorize_route(caller, request)
            .map_err(|_| BridgeError::AdapterFailure)?;
        if route.host != request.host
            || route.registry_model != request.registry_model
            || route.task != request.task
        {
            return Err(BridgeError::AdapterFailure);
        }
        if cancellation.is_cancelled() {
            return Err(BridgeError::Cancelled);
        }

        // Non-allocating reservation precedes host dispatch. On STOP the
        // supervisor will invoke this handle's scoped lease close, not a
        // broad host termination or a second unload authority.
        let handle = self
            .host
            .prepare_job(caller, &route, &request.job)
            .map_err(|_| BridgeError::AdapterFailure)?;
        lifecycle
            .track(
                session_id,
                Ownership::AppOwned,
                ResourceKind::RemoteModelLease,
                cancellation.clone(),
                handle.clone(),
            )
            .map_err(|_| BridgeError::SessionStopped)?;
        if cancellation.is_cancelled() {
            handle.request_cancel();
            return Err(BridgeError::Cancelled);
        }
        if self
            .host
            .dispatch_job(caller, &route, &request.job, cancellation)
            .is_err()
        {
            handle.request_cancel();
            return Err(BridgeError::AdapterFailure);
        }
        Ok(ToolResponse {
            summary: "approved host task accepted; scoped lease owned by app session".into(),
        })
    }
}

impl AppAdapter for TypedAppAdapter {
    fn app_id(&self) -> &str {
        self.app.as_str()
    }

    fn is_available(&self) -> bool {
        self.available
    }

    fn capabilities(&self) -> Vec<Capability> {
        self.advertised.iter().copied().collect()
    }

    fn invoke(
        &self,
        client: &str,
        request: &ToolRequest,
        cancellation: &CancellationToken,
        lifecycle: &mut LifecycleSupervisor,
        session_id: &str,
    ) -> Result<ToolResponse, BridgeError> {
        if request.app_id != self.app.as_str() {
            return Err(BridgeError::InvalidArguments);
        }
        if cancellation.is_cancelled() {
            return Err(BridgeError::Cancelled);
        }
        match (&request.capability, &request.arguments) {
            (Capability::ListModels, Arguments::None) => {
                let records = self.visible_records(None);
                Ok(ToolResponse {
                    summary: format!(
                        "{} registered references (not runtime proof): {}",
                        records.len(),
                        Self::summarize_refs(&records)
                    ),
                })
            }
            (Capability::FindModelForTask, Arguments::FindModelsForTask(task)) => {
                let records = self.visible_records(Some(*task));
                let verified = records
                    .iter()
                    .filter(|r| r.verification == RegistryVerification::RuntimeVerified)
                    .count();
                Ok(ToolResponse {
                    summary: format!(
                        "{} task candidates; {} registry runtime-verified; no host execution inferred: {}",
                        records.len(),
                        verified,
                        Self::summarize_refs(&records)
                    ),
                })
            }
            (Capability::GetAppStatus, Arguments::None) => Ok(ToolResponse {
                summary: "registered app adapter available (mock contract only)".into(),
            }),
            (Capability::RequestMediaTask, Arguments::HostTask(task)) => {
                let caller = CallerIdentity {
                    client: ClientId::new(client).map_err(|_| BridgeError::InvalidIdentifier)?,
                    app: self.app.clone(),
                    owner_session: OwnerSessionId::new(session_id)
                        .map_err(|_| BridgeError::InvalidIdentifier)?,
                };
                self.start_host_task(&caller, task, cancellation, lifecycle)
            }
            _ => Err(BridgeError::InvalidArguments),
        }
    }
}
