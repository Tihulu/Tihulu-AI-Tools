//! Versioned, typed UI ⇄ Rust DTOs (schema v1), with no native side effects.
//!
//! No Tauri commands are registered. No model, GPU, SDR, Jan or host is touched.
//! The coordinator-owned resource ledger and owner-specific adapters will be the
//! authority for cleanup, not this module or the browser's mock reducer.

use serde::{Deserialize, Serialize};

pub const UI_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSource {
    DemoFixture,
    BackendVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceMode {
    Disabled,
    Independent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GuiPhase {
    Running,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    NotConnected,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityVerdict {
    Unverified,
    Incompatible,
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelAvailability {
    Unknown,
    Candidate,
    Installed,
    External,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownAction {
    #[serde(rename = "stop-ai")]
    StopAi,
    #[serde(rename = "app-quit")]
    AppQuit,
    #[serde(rename = "hard-stop")]
    HardStop,
    #[serde(rename = "quit-all")]
    QuitAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    Advisor,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EmptyPayload {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityQuery {
    /// A stable registry model ID. Never a raw filesystem path.
    pub model_id: String,
    pub task_id: String,
    pub host_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChatQuery {
    pub agent: Agent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StopQuery {
    pub action: ShutdownAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmedStop {
    pub action: ShutdownAction,
    pub confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConfirmedServiceMode {
    pub mode: ServiceMode,
    pub confirmed: bool,
}

/// Request payloads are intentionally closed: no shell, arbitrary path, URL,
/// process ID, credentials, device handle or unrestricted tool execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum UiCommand {
    ReadOverview(EmptyPayload),
    ReadModels(EmptyPayload),
    ReadCompatibility(CompatibilityQuery),
    ReadGpuHost(EmptyPayload),
    ReadChat(ChatQuery),
    ReadJan(EmptyPayload),
    PreviewStop(StopQuery),
    ExecuteStop(ConfirmedStop),
    SetServiceMode(ConfirmedServiceMode),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiRequest {
    pub schema_version: u8,
    pub request_id: String,
    #[serde(flatten)]
    pub command: UiCommand,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiReply<T> {
    pub schema_version: u8,
    pub source: EvidenceSource,
    pub request_id: String,
    pub kind: String,
    pub data: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GuiSnapshot {
    pub session_id: String,
    pub phase: GuiPhase,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub execution: String,
    pub background_enabled: bool,
    pub keep_warm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub resource_id: String,
    pub label: String,
    pub owner_id: String,
    pub provenance: String,
    pub resource_kind: String,
    pub mock_phase: String,
    pub estimated_ram_mib: Option<u64>,
    pub estimated_vram_mib: Option<u64>,
    pub backend_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyntheticTotals {
    #[serde(rename = "activeOwned")]
    pub active_owned: u64,
    #[serde(rename = "mockRamMiB")]
    pub mock_ram_mib: u64,
    #[serde(rename = "mockVramMiB")]
    pub mock_vram_mib: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverviewSnapshot {
    pub gui: GuiSnapshot,
    pub policy: ExecutionPolicy,
    pub resources: Vec<ResourceSnapshot>,
    pub synthetic_totals: SyntheticTotals,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSnapshot {
    pub model_id: String,
    pub name: String,
    pub format: String,
    pub modality: String,
    pub availability: ModelAvailability,
    pub owner_type: String,
    pub selected_runtime: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelsSnapshot {
    pub models: Vec<ModelSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompatibilitySnapshot {
    pub model_id: String,
    pub task_id: String,
    pub host_id: String,
    pub verdict: CompatibilityVerdict,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostSnapshot {
    pub host_id: String,
    pub service_mode: ServiceMode,
    pub gui_phase: GuiPhase,
    pub service_phase: String,
    pub active_other_clients: u32,
    pub connection: ConnectionState,
    pub scope: String,
    pub backend_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSnapshot {
    pub agent: Agent,
    pub messages: Vec<ChatMessage>,
    pub inference_connected: bool,
    pub permission_mode: String,
    pub available_tools: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JanSnapshot {
    pub connection: String,
    pub wizard_step: i32,
    pub registered: bool,
    pub verified_handshake: bool,
    pub executable_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownPreview {
    pub title: String,
    pub description: String,
    pub affected: Vec<String>,
    pub preserved: Vec<String>,
    pub warnings: Vec<String>,
    pub backend_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShutdownResult {
    pub outcome: String,
    pub gui_phase: GuiPhase,
    pub service_mode: ServiceMode,
    pub simulated_stages: Vec<String>,
    pub backend_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceModeResult {
    pub outcome: String,
    pub service_mode: ServiceMode,
    pub connected_clients: u32,
    pub backend_verified: bool,
}

/// Validates request envelope and confirmation gates, without authorizing work.
/// Real adapters additionally require authenticated owner/session and policy
/// approval. It never treats a browser request as authoritative.
pub fn validate_request(request: &UiRequest) -> Result<(), &'static str> {
    if request.schema_version != UI_SCHEMA_VERSION {
        return Err("Unsupported UI schema version");
    }
    if request.request_id.is_empty()
        || request.request_id.len() > 64
        || !request
            .request_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b':' )
    {
        return Err("Invalid request id");
    }
    match &request.command {
        UiCommand::ExecuteStop(command) if !command.confirmed => Err("Stop confirmation is required"),
        UiCommand::SetServiceMode(command) if !command.confirmed => Err("Service-mode confirmation is required"),
        UiCommand::ReadCompatibility(query)
            if query.model_id.is_empty() || query.task_id.is_empty() || query.host_id.is_empty() =>
        {
            Err("A typed model, task and host identity are required")
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_request_matches_browser_envelope() {
        let json = r#"{"schema_version":1,"request_id":"test-1","kind":"read_gpu_host","payload":{}}"#;
        let request: UiRequest = serde_json::from_str(json).expect("valid request JSON");
        assert_eq!(request.command, UiCommand::ReadGpuHost(EmptyPayload {}));
        assert_eq!(serde_json::to_value(&request).unwrap(), serde_json::from_str::<serde_json::Value>(json).unwrap());
        assert_eq!(validate_request(&request), Ok(()));
    }

    #[test]
    fn typed_shutdown_actions_and_explicit_approval() {
        for (name, action) in [
            ("stop-ai", ShutdownAction::StopAi),
            ("app-quit", ShutdownAction::AppQuit),
            ("hard-stop", ShutdownAction::HardStop),
            ("quit-all", ShutdownAction::QuitAll),
        ] {
            let json = format!(
                r#"{{"schema_version":1,"request_id":"stop-1","kind":"execute_stop","payload":{{"action":"{name}","confirmed":true}}}}"#
            );
            let request: UiRequest = serde_json::from_str(&json).unwrap();
            assert_eq!(request.command, UiCommand::ExecuteStop(ConfirmedStop { action, confirmed: true }));
            assert_eq!(validate_request(&request), Ok(()));
        }
        let unapproved: UiRequest = serde_json::from_str(
            r#"{"schema_version":1,"request_id":"stop-1","kind":"execute_stop","payload":{"action":"hard-stop","confirmed":false}}"#
        ).unwrap();
        assert!(validate_request(&unapproved).is_err());
    }

    #[test]
    fn no_raw_shell_or_filesystem_request_variant() {
        for kind in ["shell", "read_file", "stop_process_by_name", "run_untrusted_model", "open_usb"] {
            let json = format!(r#"{{"schema_version":1,"request_id":"bad","kind":"{kind}","payload":{{}}}}"#);
            assert!(serde_json::from_str::<UiRequest>(&json).is_err(), "{kind} should not parse");
        }
    }

    #[test]
    fn independent_service_is_not_the_gui_session() {
        let response = UiReply {
            schema_version: 1,
            source: EvidenceSource::DemoFixture,
            request_id: "host-1".to_owned(),
            kind: "read_gpu_host".to_owned(),
            data: HostSnapshot {
                host_id: "demo:independent-pc".to_owned(),
                service_mode: ServiceMode::Independent,
                gui_phase: GuiPhase::Closed,
                service_phase: "active".to_owned(),
                active_other_clients: 2,
                connection: ConnectionState::NotConnected,
                scope: "independent_tihulu_host".to_owned(),
                backend_verified: false,
            },
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"gui_phase\":\"closed\""));
        assert!(json.contains("\"service_mode\":\"independent\""));
        assert!(json.contains("\"backend_verified\":false"));
        let roundtrip: UiReply<HostSnapshot> = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.data.gui_phase, GuiPhase::Closed);
        assert_eq!(roundtrip.data.active_other_clients, 2);
    }

    #[test]
    fn compatibility_unverified_not_installed_runnable() {
        let result = CompatibilitySnapshot {
            model_id: "demo:external-gguf".to_owned(),
            task_id: "text".to_owned(),
            host_id: "local".to_owned(),
            verdict: CompatibilityVerdict::Unverified,
            reasons: vec!["Backend has not checked runtime".to_owned()],
        };
        assert_ne!(result.verdict, CompatibilityVerdict::Verified);
        assert_eq!(serde_json::to_value(&result).unwrap()["verdict"], "unverified");
    }
}
