use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tihulu_ai_integrations::bridge::{
    Access, AppAdapter, Arguments, BridgeError, Capability, IntegrationBridge, ToolRequest,
    ToolResponse,
};
use tihulu_ai_integrations::lifecycle::{
    CancellationToken, LifecycleError, LifecycleSupervisor, Ownership, ResourceHandle,
    ResourceKind, SessionState, ShutdownReason,
};
use tihulu_ai_integrations::references::{
    ModelReference, ReferenceError, StartupAvailability, StartupReferences, StartupRoute,
};
use tihulu_ai_integrations::wizard::{
    DetectedJan, JanProduct, SetupMethod, SetupWizard, WizardError, WizardState,
};

struct MockAdapter {
    id: &'static str,
    ready: bool,
    capabilities: Vec<Capability>,
    calls: Arc<AtomicUsize>,
}

impl AppAdapter for MockAdapter {
    fn app_id(&self) -> &str {
        self.id
    }

    fn is_available(&self) -> bool {
        self.ready
    }

    fn capabilities(&self) -> Vec<Capability> {
        self.capabilities.clone()
    }

    fn invoke(
        &self,
        _client: &str,
        _request: &ToolRequest,
        cancel: &CancellationToken,
        _lifecycle: &mut LifecycleSupervisor,
        _session_id: &str,
    ) -> Result<ToolResponse, BridgeError> {
        if cancel.is_cancelled() {
            return Err(BridgeError::Cancelled);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolResponse {
            summary: "mock: redacted".into(),
        })
    }
}

fn request(capability: Capability, arguments: Arguments) -> ToolRequest {
    ToolRequest {
        app_id: "satellite".into(),
        capability,
        arguments,
    }
}

#[test]
fn only_registered_available_scoped_adapters_advertise_tools() {
    let mut bridge = IntegrationBridge::new();
    assert!(bridge.list_tools("jan-client").is_empty());
    bridge
        .permissions_mut()
        .grant("jan-client", "satellite", Capability::GetAppStatus)
        .unwrap();
    assert!(bridge.list_tools("jan-client").is_empty()); // no adapter
    let calls = Arc::new(AtomicUsize::new(0));
    bridge
        .register_adapter(Box::new(MockAdapter {
            id: "satellite",
            ready: true,
            capabilities: vec![Capability::GetAppStatus],
            calls,
        }))
        .unwrap();
    assert_eq!(bridge.list_tools("jan-client").len(), 1);
    assert_eq!(
        bridge.list_tools("jan-client")[0].name,
        "tihulu.get_app_status"
    );
    assert_eq!(bridge.list_tools("jan-client")[0].access, Access::ReadOnly);
    assert!(bridge.list_tools("other-client").is_empty());
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("active").unwrap();
    let result = bridge.invoke(
        "other-client",
        &request(Capability::GetAppStatus, Arguments::None),
        None,
        &CancellationToken::default(),
        &mut lifecycle,
        "active",
    );
    assert_eq!(result, Err(BridgeError::NotGranted));
    let result = bridge.invoke(
        "jan-client",
        &request(
            Capability::RequestMediaTask,
            Arguments::TaskId("job".into()),
        ),
        None,
        &CancellationToken::default(),
        &mut lifecycle,
        "active",
    );
    assert_eq!(result, Err(BridgeError::CapabilityUnavailable));
}

#[test]
fn mcp_tool_names_are_unique_for_multiple_authorized_app_adapters() {
    let mut bridge = IntegrationBridge::new();
    for id in ["satellite", "media"] {
        bridge
            .register_adapter(Box::new(MockAdapter {
                id,
                ready: true,
                capabilities: vec![Capability::GetAppStatus],
                calls: Arc::new(AtomicUsize::new(0)),
            }))
            .unwrap();
        bridge
            .permissions_mut()
            .grant("jan", id, Capability::GetAppStatus)
            .unwrap();
    }
    let tools = bridge.list_tools("jan");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "tihulu.get_app_status");
    assert_eq!(
        tools[0].allowed_app_ids,
        vec!["media".to_owned(), "satellite".to_owned()]
    );
}

#[test]
fn unavailable_adapter_and_stopped_session_are_not_callable() {
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(MockAdapter {
            id: "satellite",
            ready: false,
            capabilities: vec![Capability::GetAppStatus],
            calls: Arc::new(AtomicUsize::new(0)),
        }))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "satellite", Capability::GetAppStatus)
        .unwrap();
    assert!(bridge.list_tools("jan").is_empty());
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("active").unwrap();
    lifecycle.start_session("stopped").unwrap();
    lifecycle
        .shutdown("stopped", ShutdownReason::Stop, Duration::ZERO)
        .unwrap();
    let req = request(Capability::GetAppStatus, Arguments::None);
    assert_eq!(
        bridge.invoke(
            "jan",
            &req,
            None,
            &CancellationToken::default(),
            &mut lifecycle,
            "active"
        ),
        Err(BridgeError::AdapterUnavailable)
    );
    assert_eq!(
        bridge.invoke(
            "jan",
            &req,
            None,
            &CancellationToken::default(),
            &mut lifecycle,
            "stopped"
        ),
        Err(BridgeError::SessionStopped)
    );
}

#[test]
fn mutating_tool_requires_exact_scoped_single_use_approval() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(MockAdapter {
            id: "satellite",
            ready: true,
            capabilities: vec![Capability::RequestMediaTask],
            calls: calls.clone(),
        }))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "satellite", Capability::RequestMediaTask)
        .unwrap();
    let req = request(
        Capability::RequestMediaTask,
        Arguments::TaskId("approved-task".into()),
    );
    let wrong = request(
        Capability::RequestMediaTask,
        Arguments::TaskId("unapproved-task".into()),
    );
    let cancel = CancellationToken::default();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("active").unwrap();
    assert_eq!(
        bridge.invoke("jan", &req, None, &cancel, &mut lifecycle, "active"),
        Err(BridgeError::ApprovalRequired)
    );
    let ticket = bridge
        .permissions_mut()
        .owner_approved_action("jan", &req, "active")
        .unwrap();
    assert_eq!(
        bridge.invoke(
            "jan",
            &wrong,
            Some(ticket),
            &cancel,
            &mut lifecycle,
            "active"
        ),
        Err(BridgeError::InvalidApproval)
    );
    assert_eq!(
        bridge.invoke(
            "other",
            &req,
            Some(ticket),
            &cancel,
            &mut lifecycle,
            "active"
        ),
        Err(BridgeError::NotGranted)
    );
    assert!(bridge
        .invoke("jan", &req, Some(ticket), &cancel, &mut lifecycle, "active")
        .is_ok());
    assert_eq!(
        bridge.invoke("jan", &req, Some(ticket), &cancel, &mut lifecycle, "active"),
        Err(BridgeError::InvalidApproval)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    bridge.permissions_mut().revoke_client("jan");
    assert!(bridge.list_tools("jan").is_empty());
}

struct MockResource {
    cancelled: AtomicUsize,
    forced: AtomicUsize,
    released: AtomicBool,
    release_on_cancel: bool,
    release_on_force: bool,
}

impl MockResource {
    fn new(release_on_cancel: bool, release_on_force: bool) -> Self {
        Self {
            cancelled: AtomicUsize::new(0),
            forced: AtomicUsize::new(0),
            released: AtomicBool::new(false),
            release_on_cancel,
            release_on_force,
        }
    }
}

impl ResourceHandle for MockResource {
    fn request_cancel(&self) {
        self.cancelled.fetch_add(1, Ordering::SeqCst);
        if self.release_on_cancel {
            self.released.store(true, Ordering::SeqCst);
        }
    }

    fn is_released(&self) -> bool {
        self.released.load(Ordering::SeqCst)
    }

    fn force_close_owned(&self) {
        self.forced.fetch_add(1, Ordering::SeqCst);
        if self.release_on_force {
            self.released.store(true, Ordering::SeqCst);
        }
    }
}

#[test]
fn l05_l06_l07_app_sdr_released_external_services_preserved() {
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("satellite-session").unwrap();
    lifecycle.start_session("other-app-session").unwrap();
    let sdr = Arc::new(MockResource::new(true, true));
    let other_sdr = Arc::new(MockResource::new(true, true));
    let token = CancellationToken::default();
    lifecycle
        .track(
            "satellite-session",
            Ownership::AppOwned,
            ResourceKind::SdrConnection,
            token.clone(),
            sdr.clone(),
        )
        .unwrap();
    lifecycle
        .track(
            "other-app-session",
            Ownership::AppOwned,
            ResourceKind::SdrConnection,
            CancellationToken::default(),
            other_sdr.clone(),
        )
        .unwrap();
    let borrowed: Vec<_> = [
        ResourceKind::BorrowedJan,
        ResourceKind::BorrowedOllama,
        ResourceKind::BorrowedReadsb,
        ResourceKind::BorrowedTar1090,
    ]
    .into_iter()
    .map(|kind| {
        let handle = Arc::new(MockResource::new(true, true));
        let external_token = CancellationToken::default();
        lifecycle
            .track(
                "satellite-session",
                Ownership::BorrowedExternal,
                kind,
                external_token.clone(),
                handle.clone(),
            )
            .unwrap();
        (handle, external_token)
    })
    .collect();

    let report = lifecycle
        .shutdown(
            "satellite-session",
            ShutdownReason::HardStop,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(report.borrowed_preserved, 4);
    assert_eq!(report.unresolved_owned, 0);
    assert_eq!(report.state, SessionState::Stopped);
    assert!(token.is_cancelled());
    assert_eq!(sdr.cancelled.load(Ordering::SeqCst), 1);
    assert_eq!(other_sdr.cancelled.load(Ordering::SeqCst), 0);
    for (handle, external_token) in borrowed {
        assert_eq!(handle.cancelled.load(Ordering::SeqCst), 0);
        assert_eq!(handle.forced.load(Ordering::SeqCst), 0);
        assert!(!external_token.is_cancelled());
    }
    let again = lifecycle
        .shutdown("satellite-session", ShutdownReason::Exit, Duration::ZERO)
        .unwrap();
    assert_eq!(again.cancellation_requests, 0);
}

#[test]
fn l08_unresponsive_owned_worker_escalates_only_own_handle() {
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("own-session").unwrap();
    let owned = Arc::new(MockResource::new(false, true));
    let external = Arc::new(MockResource::new(false, true));
    lifecycle
        .track(
            "own-session",
            Ownership::AppOwned,
            ResourceKind::OwnedChild,
            CancellationToken::default(),
            owned.clone(),
        )
        .unwrap();
    lifecycle
        .track(
            "own-session",
            Ownership::BorrowedExternal,
            ResourceKind::BorrowedJan,
            CancellationToken::default(),
            external.clone(),
        )
        .unwrap();
    let report = lifecycle
        .shutdown(
            "own-session",
            ShutdownReason::Exit,
            Duration::from_millis(1),
        )
        .unwrap();
    assert_eq!(report.force_close_requests, 1);
    assert_eq!(report.state, SessionState::Stopped);
    assert_eq!(owned.forced.load(Ordering::SeqCst), 1);
    assert_eq!(external.forced.load(Ordering::SeqCst), 0);
}

#[test]
fn l10_late_registration_blocked_and_leaked_resource_reported() {
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("old-session").unwrap();
    let stuck = Arc::new(MockResource::new(false, false));
    lifecycle
        .track(
            "old-session",
            Ownership::AppOwned,
            ResourceKind::RemoteModelLease,
            CancellationToken::default(),
            stuck.clone(),
        )
        .unwrap();
    let report = lifecycle
        .shutdown("old-session", ShutdownReason::HardStop, Duration::ZERO)
        .unwrap();
    assert_eq!(report.state, SessionState::Stopping);
    assert_eq!(report.unresolved_owned, 1);
    assert_eq!(
        lifecycle.track(
            "old-session",
            Ownership::AppOwned,
            ResourceKind::McpTask,
            CancellationToken::default(),
            Arc::new(MockResource::new(true, true))
        ),
        Err(LifecycleError::SessionStopping)
    );
    stuck.released.store(true, Ordering::SeqCst); // simulated host TTL
    let report = lifecycle
        .shutdown("old-session", ShutdownReason::Exit, Duration::ZERO)
        .unwrap();
    assert_eq!(report.state, SessionState::Stopped);
    lifecycle.start_session("fresh-session").unwrap();
    assert_eq!(
        lifecycle.state("fresh-session"),
        Some(SessionState::Running)
    );
}

#[test]
fn jan_wizard_manual_fallback_requires_external_verification() {
    let mut wizard = SetupWizard::new();
    wizard
        .detect(DetectedJan {
            product: JanProduct::Desktop,
            version: "test-version".into(),
            verified_cli_supported: false,
            existing_entry: false,
            existing_entry_owned_by_us: false,
        })
        .unwrap();
    assert_eq!(wizard.review().unwrap(), SetupMethod::GuidedManual);
    assert_eq!(
        wizard.owner_approved(false),
        Err(WizardError::PermissionRequired)
    );
    assert_eq!(wizard.review().unwrap(), SetupMethod::GuidedManual);
    assert_eq!(
        wizard.owner_approved(true).unwrap(),
        SetupMethod::GuidedManual
    );
    assert_eq!(wizard.state(), WizardState::AwaitingManualConfiguration);
    wizard.record_configuration(true, true).unwrap();
    assert_eq!(
        wizard.record_verification(true, false),
        Err(WizardError::NotActuallyVerified)
    );
    assert_ne!(wizard.state(), WizardState::ConnectedVerified);
    assert!(wizard.plan_disconnect().unwrap());
    wizard.record_disconnect(true).unwrap();
    assert_eq!(wizard.state(), WizardState::Disconnected);
}

#[test]
fn jan_wizard_protects_existing_third_party_entry() {
    let mut wizard = SetupWizard::new();
    assert_eq!(
        wizard.detect(DetectedJan {
            product: JanProduct::Agent,
            version: "mock".into(),
            verified_cli_supported: true,
            existing_entry: true,
            existing_entry_owned_by_us: false,
        }),
        Err(WizardError::ExistingForeignEntry)
    );
    assert_eq!(wizard.state(), WizardState::NotConfigured);
}

struct MockAvailability {
    local: bool,
    remote: bool,
}
impl StartupAvailability for MockAvailability {
    fn model_usable(&self, _reference: &ModelReference) -> bool {
        self.local
    }
    fn paired_host_usable(&self, _host_id: &str) -> bool {
        self.remote
    }
}

#[test]
fn aipath_references_are_non_secret_and_gui_independent() {
    let spec = "schema=2-proposal\napp_id=survival-kit\nmanager_mode=optional\n\
                task.llm.model_ref=external:jan:llm-v1\n\
                task.llm.remote_host_id=paired-pc-01\n\
                task.llm.compute=remote-first\nfallback=legacy";
    let refs = StartupReferences::parse_proposal(spec).unwrap();
    assert_eq!(
        refs.choose_route(&MockAvailability {
            local: true,
            remote: true
        }),
        StartupRoute::PairedHost
    );
    assert_eq!(
        refs.choose_route(&MockAvailability {
            local: true,
            remote: false
        }),
        StartupRoute::DirectVerifiedModel
    );
    assert_eq!(
        refs.choose_route(&MockAvailability {
            local: false,
            remote: false
        }),
        StartupRoute::LegacyFallback
    );
    assert_eq!(
        StartupReferences::parse_proposal(&format!("{spec}\ntoken=secret")),
        Err(ReferenceError::UnknownField)
    );
    assert_eq!(
        StartupReferences::parse_proposal(
            &spec.replace("external:jan:llm-v1", "external:jan:../../private")
        ),
        Err(ReferenceError::InvalidValue)
    );
}
