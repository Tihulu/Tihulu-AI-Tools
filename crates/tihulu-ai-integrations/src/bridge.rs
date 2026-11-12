//! Transport-neutral authorization core. A future MCP transport must never
//! expose direct access to registration, grants or owner approvals.

use std::collections::{BTreeMap, BTreeSet};

use crate::lifecycle::{CancellationToken, LifecycleSupervisor, SessionState};
use crate::typed::{HostTask, TaskKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    ListApps,
    ListModels,
    FindModelForTask,
    GetAppStatus,
    GetSatellitePasses,
    PreviewModelBinding,
    RequestModelBinding,
    RequestMediaTask,
    CancelOwnedJob,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    ReadOnly,
    ExplicitAction,
}

impl Capability {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ListApps => "tihulu.list_apps",
            Self::ListModels => "tihulu.list_models",
            Self::FindModelForTask => "tihulu.find_model_for_task",
            Self::GetAppStatus => "tihulu.get_app_status",
            Self::GetSatellitePasses => "tihulu.get_satellite_passes",
            Self::PreviewModelBinding => "tihulu.preview_model_binding",
            Self::RequestModelBinding => "tihulu.request_model_binding",
            Self::RequestMediaTask => "tihulu.request_media_task",
            Self::CancelOwnedJob => "tihulu.cancel_owned_job",
        }
    }

    pub const fn access(self) -> Access {
        match self {
            Self::RequestModelBinding | Self::RequestMediaTask | Self::CancelOwnedJob => {
                Access::ExplicitAction
            }
            _ => Access::ReadOnly,
        }
    }
}

/// Typed arguments only; none can contain an executable, shell command or
/// unrestricted filesystem path. Adapters must validate semantic IDs too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arguments {
    None,
    TaskId(String),
    ModelId(String),
    OwnedJobId(String),
    FindModelsForTask(TaskKind),
    HostTask(HostTask),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRequest {
    pub app_id: String,
    pub capability: Capability,
    pub arguments: Arguments,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResponse {
    /// Adapter-produced, redacted result; do not include secrets/raw logs.
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeError {
    InvalidIdentifier,
    DuplicateAdapter,
    AdapterNotRegistered,
    AdapterUnavailable,
    CapabilityUnavailable,
    NotGranted,
    ApprovalRequired,
    InvalidApproval,
    InvalidArguments,
    Cancelled,
    SessionStopped,
    AdapterFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDescription {
    pub name: &'static str,
    pub access: Access,
    /// A single MCP tool name may serve several independently granted apps.
    pub allowed_app_ids: Vec<String>,
}

/// In production, only a verified application adapter should be registered.
/// A mock advertising a capability is appropriate solely for tests.
pub trait AppAdapter: Send + Sync {
    fn app_id(&self) -> &str;
    fn is_available(&self) -> bool;
    fn capabilities(&self) -> Vec<Capability>;
    fn invoke(
        &self,
        client: &str,
        request: &ToolRequest,
        cancellation: &CancellationToken,
        lifecycle: &mut LifecycleSupervisor,
        session_id: &str,
    ) -> Result<ToolResponse, BridgeError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ApprovalTicket(u64);

#[derive(Debug, Default)]
pub struct PermissionGate {
    grants: BTreeSet<(String, String, Capability)>,
    one_shot: BTreeMap<ApprovalTicket, (String, String, ToolRequest)>,
    next_ticket: u64,
}

impl PermissionGate {
    /// TRUSTED HOST API: user-granted client/app/tool scope. This function must
    /// NOT be exposed to Jan, an LLM, the MCP transport or an untrusted app.
    pub fn grant(
        &mut self,
        client: &str,
        app: &str,
        capability: Capability,
    ) -> Result<(), BridgeError> {
        if !safe_id(client) || !safe_id(app) {
            return Err(BridgeError::InvalidIdentifier);
        }
        self.grants
            .insert((client.to_owned(), app.to_owned(), capability));
        Ok(())
    }

    pub fn revoke_client(&mut self, client: &str) {
        self.grants.retain(|(c, _, _)| c != client);
        self.one_shot.retain(|_, (c, _, _)| c != client);
    }

    pub fn has_grant(&self, client: &str, app: &str, capability: Capability) -> bool {
        self.grants
            .contains(&(client.to_owned(), app.to_owned(), capability))
    }

    /// TRUSTED HOST API: call only after a separate owner confirmation for
    /// this exact request. Never offer this method as an MCP tool.
    pub fn owner_approved_action(
        &mut self,
        client: &str,
        request: &ToolRequest,
        owner_session: &str,
    ) -> Result<ApprovalTicket, BridgeError> {
        if !safe_id(owner_session) {
            return Err(BridgeError::InvalidIdentifier);
        }
        if request.capability.access() != Access::ExplicitAction
            || !self.has_grant(client, &request.app_id, request.capability)
        {
            return Err(BridgeError::NotGranted);
        }
        self.next_ticket = self
            .next_ticket
            .checked_add(1)
            .ok_or(BridgeError::InvalidApproval)?;
        let ticket = ApprovalTicket(self.next_ticket);
        self.one_shot.insert(
            ticket,
            (client.to_owned(), owner_session.to_owned(), request.clone()),
        );
        Ok(ticket)
    }

    fn authorize(
        &mut self,
        client: &str,
        request: &ToolRequest,
        ticket: Option<ApprovalTicket>,
        owner_session: &str,
    ) -> Result<(), BridgeError> {
        if !self.has_grant(client, &request.app_id, request.capability) {
            return Err(BridgeError::NotGranted);
        }
        if request.capability.access() == Access::ReadOnly {
            return Ok(());
        }
        let ticket = ticket.ok_or(BridgeError::ApprovalRequired)?;
        match self.one_shot.get(&ticket) {
            Some((approved_client, approved_session, approved_request))
                if approved_client == client
                    && approved_session == owner_session
                    && approved_request == request =>
            {
                self.one_shot.remove(&ticket);
                Ok(())
            }
            _ => Err(BridgeError::InvalidApproval),
        }
    }
}

pub struct IntegrationBridge {
    adapters: BTreeMap<String, Box<dyn AppAdapter>>,
    permissions: PermissionGate,
}

impl Default for IntegrationBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl IntegrationBridge {
    pub fn new() -> Self {
        Self {
            adapters: BTreeMap::new(),
            permissions: PermissionGate::default(),
        }
    }

    /// TRUSTED HOST ONLY; no app is implicitly discovered or enabled.
    pub fn register_adapter(&mut self, adapter: Box<dyn AppAdapter>) -> Result<(), BridgeError> {
        let app = adapter.app_id();
        if !safe_id(app) {
            return Err(BridgeError::InvalidIdentifier);
        }
        if self.adapters.contains_key(app) {
            return Err(BridgeError::DuplicateAdapter);
        }
        self.adapters.insert(app.to_owned(), adapter);
        Ok(())
    }

    pub fn permissions_mut(&mut self) -> &mut PermissionGate {
        // Only the trusted process host may call this method. The MCP request
        // dispatcher must hold a restricted reference to invoke/list_tools.
        &mut self.permissions
    }

    /// Advertise only actually available, registered and scoped app tools.
    pub fn list_tools(&self, client: &str) -> Vec<ToolDescription> {
        let mut visible: BTreeMap<Capability, Vec<String>> = BTreeMap::new();
        for (app_id, adapter) in &self.adapters {
            if !adapter.is_available() {
                continue;
            }
            let capabilities: BTreeSet<_> = adapter.capabilities().into_iter().collect();
            for capability in capabilities {
                if self.permissions.has_grant(client, app_id, capability) {
                    visible.entry(capability).or_default().push(app_id.clone());
                }
            }
        }
        visible
            .into_iter()
            .map(|(capability, allowed_app_ids)| ToolDescription {
                name: capability.name(),
                access: capability.access(),
                allowed_app_ids,
            })
            .collect()
    }

    pub fn invoke(
        &mut self,
        client: &str,
        request: &ToolRequest,
        approval: Option<ApprovalTicket>,
        cancellation: &CancellationToken,
        lifecycle: &mut LifecycleSupervisor,
        session_id: &str,
    ) -> Result<ToolResponse, BridgeError> {
        if lifecycle.state(session_id) != Some(SessionState::Running) {
            return Err(BridgeError::SessionStopped);
        }
        if cancellation.is_cancelled() {
            return Err(BridgeError::Cancelled);
        }
        let adapter = self
            .adapters
            .get(&request.app_id)
            .ok_or(BridgeError::AdapterNotRegistered)?;
        if !adapter.is_available() {
            return Err(BridgeError::AdapterUnavailable);
        }
        if !adapter.capabilities().contains(&request.capability) {
            return Err(BridgeError::CapabilityUnavailable);
        }
        self.permissions.authorize(client, request, approval, session_id)?;
        let response = adapter.invoke(client, request, cancellation, lifecycle, session_id)?;
        if cancellation.is_cancelled() {
            return Err(BridgeError::Cancelled);
        }
        Ok(response)
    }
}

/// Strict identifiers used as scoped keys, never as paths or commands.
pub fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes().first().is_some_and(u8::is_ascii_alphanumeric)
        && !value.contains("..")
        && value
            .bytes()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'.' | b':'))
}
