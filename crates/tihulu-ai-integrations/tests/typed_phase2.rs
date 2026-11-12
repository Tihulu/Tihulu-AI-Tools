//! Offline-only Phase 2 acceptance: NO Jan API, network, GPU or SDR.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use tihulu_ai_integrations::bridge::{
    Arguments, BridgeError, Capability, IntegrationBridge, ToolRequest,
};
use tihulu_ai_integrations::lifecycle::{
    CancellationToken, LifecycleSupervisor, Ownership, ResourceHandle, ResourceKind, SessionState,
    ShutdownReason,
};
use tihulu_ai_integrations::references::{ReferenceError, StartupReferences};
use tihulu_ai_integrations::typed::{
    AppId, BoundaryError, CallerIdentity, HostGateway, HostId, HostModelId, HostRoute,
    HostRuntimeOwner, HostTask, JobId, RegistryModelId, RegistryOwner, RegistryReader,
    RegistryRecord, RegistryVerification, TaskKind, TypedAppAdapter,
};

struct FakeRegistry {
    entries: Vec<(AppId, RegistryRecord)>,
}

impl RegistryReader for FakeRegistry {
    fn list_visible(&self, app: &AppId) -> Vec<RegistryRecord> {
        self.entries
            .iter()
            .filter(|(id, _)| id == app)
            .map(|(_, model)| model.clone())
            .collect()
    }

    fn resolve_visible(&self, app: &AppId, model: &RegistryModelId) -> Option<RegistryRecord> {
        self.entries
            .iter()
            .find(|(id, record)| id == app && &record.id == model)
            .map(|(_, record)| record.clone())
    }
}

#[derive(Default)]
struct FakeHostState {
    active: BTreeMap<String, (HostModelId, HostRuntimeOwner)>,
    synthetic_last_lease_unloads: usize,
    force_close_calls: usize,
    borrowed_process_stops: usize,
}

#[derive(Clone)]
struct FakeHost {
    state: Arc<Mutex<FakeHostState>>,
    permitted: bool,
    runtime_owner: HostRuntimeOwner,
    contradictory_route: bool,
    denied_apps: BTreeSet<AppId>,
}

impl FakeHost {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(FakeHostState::default())),
            permitted: true,
            runtime_owner: HostRuntimeOwner::HostOwned,
            contradictory_route: false,
            denied_apps: BTreeSet::new(),
        }
    }

    fn active(&self) -> usize {
        self.state.lock().unwrap().active.len()
    }

    fn unloads(&self) -> usize {
        self.state.lock().unwrap().synthetic_last_lease_unloads
    }
}

struct FakeJobHandle {
    state: Arc<Mutex<FakeHostState>>,
    key: String,
}

impl ResourceHandle for FakeJobHandle {
    fn request_cancel(&self) {
        let mut state = self.state.lock().unwrap();
        if let Some((model, ownership)) = state.active.remove(&self.key) {
            if ownership == HostRuntimeOwner::HostOwned
                && !state
                    .active
                    .values()
                    .any(|(active_model, _)| active_model == &model)
            {
                // This mock HOST makes the authoritative last-lease decision;
                // the MCP integration layer never unloads a GPU context.
                state.synthetic_last_lease_unloads += 1;
            }
        }
    }

    fn is_released(&self) -> bool {
        !self.state.lock().unwrap().active.contains_key(&self.key)
    }

    fn force_close_owned(&self) {
        self.state.lock().unwrap().force_close_calls += 1;
        self.request_cancel();
    }
}

impl HostGateway for FakeHost {
    fn authorize_route(
        &self,
        caller: &CallerIdentity,
        request: &HostTask,
    ) -> Result<HostRoute, BoundaryError> {
        if !self.permitted || self.denied_apps.contains(&caller.app) {
            return Err(BoundaryError::HostNotAuthorized);
        }
        let host = if self.contradictory_route {
            HostId::new("wrong-host").unwrap()
        } else {
            request.host.clone()
        };
        Ok(HostRoute {
            host,
            host_model: HostModelId::new("host-owned-model-42").unwrap(),
            registry_model: request.registry_model.clone(),
            task: request.task,
            runtime_owner: self.runtime_owner,
        })
    }

    fn prepare_job(
        &self,
        caller: &CallerIdentity,
        _route: &HostRoute,
        job: &JobId,
    ) -> Result<Arc<dyn ResourceHandle>, BoundaryError> {
        let owner_session = caller.owner_session.as_str();
        // Pure handle allocation; no host job or remote lease has started.
        // An in-flight identical job ID belongs to its existing lease; do not
        // create a new handle that could cancel that first owner's work.
        let key = format!("{owner_session}:{}", job.as_str());
        if self.state.lock().unwrap().active.contains_key(&key) {
            return Err(BoundaryError::PrepareFailed);
        }
        Ok(Arc::new(FakeJobHandle {
            state: self.state.clone(),
            key,
        }))
    }

    fn dispatch_job(
        &self,
        caller: &CallerIdentity,
        route: &HostRoute,
        job: &JobId,
        cancellation: &CancellationToken,
    ) -> Result<(), BoundaryError> {
        let owner_session = caller.owner_session.as_str();
        if cancellation.is_cancelled() {
            return Err(BoundaryError::DispatchFailed);
        }
        let key = format!("{owner_session}:{}", job.as_str());
        let mut state = self.state.lock().unwrap();
        if state.active.contains_key(&key) {
            return Err(BoundaryError::DispatchFailed);
        }
        state
            .active
            .insert(key, (route.host_model.clone(), route.runtime_owner));
        Ok(())
    }
}

fn id(raw: &str) -> AppId {
    AppId::new(raw).unwrap()
}
fn model(raw: &str) -> RegistryModelId {
    RegistryModelId::new(raw).unwrap()
}
fn host() -> HostId {
    HostId::new("paired-pc-01").unwrap()
}
fn request(app: &str, capability: Capability, args: Arguments) -> ToolRequest {
    ToolRequest {
        app_id: app.into(),
        capability,
        arguments: args,
    }
}
fn task(job: &str, registry_model: &str) -> HostTask {
    HostTask {
        task: TaskKind::Transcription,
        registry_model: model(registry_model),
        host: host(),
        job: JobId::new(job).unwrap(),
    }
}

fn record(model_id: &str, status: RegistryVerification) -> RegistryRecord {
    RegistryRecord {
        id: model(model_id),
        owner: RegistryOwner::JanDesktop,
        verification: status,
        supported_tasks: BTreeSet::from([TaskKind::Transcription]),
    }
}

fn registry_for(app: &[&str], status: RegistryVerification) -> Arc<FakeRegistry> {
    Arc::new(FakeRegistry {
        entries: app
            .iter()
            .map(|app| (id(app), record("ref-v1-abc123", status)))
            .collect(),
    })
}

fn adapter(
    app: &str,
    registry: Arc<FakeRegistry>,
    host: FakeHost,
    capabilities: Vec<Capability>,
) -> TypedAppAdapter {
    TypedAppAdapter::new(id(app), true, capabilities, registry, Arc::new(host))
}

#[test]
fn p2_readonly_registry_query_is_scoped_unverified_and_side_effect_free() {
    let registry = Arc::new(FakeRegistry {
        entries: vec![
            (
                id("media"),
                record("ref-v1-a", RegistryVerification::Candidate),
            ),
            (
                id("media"),
                record("ref-v1-b", RegistryVerification::RuntimeVerified),
            ),
            (
                id("text"),
                record("ref-v1-c", RegistryVerification::RuntimeVerified),
            ),
        ],
    });
    let host = FakeHost::new();
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::FindModelForTask, Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan-chat", "media", Capability::FindModelForTask)
        .unwrap();
    let visible = bridge.list_tools("jan-chat");
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].name, "tihulu.find_model_for_task");
    assert!(bridge.list_tools("other-chat").is_empty());
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("app-media-1").unwrap();
    let find = request(
        "media",
        Capability::FindModelForTask,
        Arguments::FindModelsForTask(TaskKind::Transcription),
    );
    let result = bridge
        .invoke(
            "jan-chat",
            &find,
            None,
            &CancellationToken::default(),
            &mut lifecycle,
            "app-media-1",
        )
        .unwrap();
    assert!(result
        .summary
        .contains("2 task candidates; 1 registry runtime-verified"));
    assert!(result.summary.contains("ref-v1-a (JanDesktop, Candidate)"));
    assert!(result
        .summary
        .contains("ref-v1-b (JanDesktop, RuntimeVerified)"));
    assert!(!result.summary.contains("Jan/path"));
    assert_eq!(host.active(), 0);
    assert_eq!(host.unloads(), 0);
    assert_eq!(
        bridge.invoke(
            "other-chat",
            &find,
            None,
            &CancellationToken::default(),
            &mut lifecycle,
            "app-media-1",
        ),
        Err(BridgeError::NotGranted)
    );
}

#[test]
fn p2_action_requires_one_shot_approval_and_stop_releases_only_owned_lease() {
    let registry = registry_for(&["media"], RegistryVerification::Candidate);
    let host = FakeHost::new();
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::FindModelForTask, Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan-chat", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("media-session-1").unwrap();
    let cancel = CancellationToken::default();

    // Externally running Jan/Ollama/readsb are borrowed, even under Hard Stop.
    let sentinels: Vec<_> = [
        ResourceKind::BorrowedJan,
        ResourceKind::BorrowedOllama,
        ResourceKind::BorrowedReadsb,
    ]
    .iter()
    .map(|kind| {
        let fake = Arc::new(FakeBorrowedResource::default());
        lifecycle
            .track(
                "media-session-1",
                Ownership::BorrowedExternal,
                *kind,
                CancellationToken::default(),
                fake.clone(),
            )
            .unwrap();
        fake
    })
    .collect();

    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job-1", "ref-v1-abc123")),
    );
    assert_eq!(
        bridge.invoke(
            "jan-chat",
            &action,
            None,
            &cancel,
            &mut lifecycle,
            "media-session-1"
        ),
        Err(BridgeError::ApprovalRequired)
    );
    assert_eq!(host.active(), 0);
    let approval = bridge
        .permissions_mut()
        .owner_approved_action("jan-chat", &action, "media-session-1")
        .unwrap();
    let success = bridge.invoke(
        "jan-chat",
        &action,
        Some(approval),
        &cancel,
        &mut lifecycle,
        "media-session-1",
    );
    assert!(success.is_ok(), "{success:?}");
    assert_eq!(host.active(), 1);
    assert_eq!(
        bridge.invoke(
            "jan-chat",
            &action,
            Some(approval),
            &cancel,
            &mut lifecycle,
            "media-session-1"
        ),
        Err(BridgeError::InvalidApproval)
    );
    let report = lifecycle
        .shutdown(
            "media-session-1",
            ShutdownReason::HardStop,
            std::time::Duration::ZERO,
        )
        .unwrap();
    assert_eq!(report.state, SessionState::Stopped);
    assert_eq!(report.borrowed_preserved, 3);
    assert!(cancel.is_cancelled());
    assert_eq!(host.active(), 0);
    assert_eq!(host.unloads(), 1);
    assert_eq!(host.state.lock().unwrap().borrowed_process_stops, 0);
    for sentinel in sentinels {
        assert_eq!(sentinel.stops(), 0);
    }
    let again = lifecycle
        .shutdown(
            "media-session-1",
            ShutdownReason::Exit,
            std::time::Duration::ZERO,
        )
        .unwrap();
    assert_eq!(again.cancellation_requests, 0);
    assert_eq!(host.unloads(), 1);
    assert_eq!(
        bridge.invoke(
            "jan-chat",
            &action,
            None,
            &cancel,
            &mut lifecycle,
            "media-session-1"
        ),
        Err(BridgeError::SessionStopped)
    );
}

#[derive(Default)]
struct FakeBorrowedResource {
    stop_count: std::sync::atomic::AtomicUsize,
}
impl FakeBorrowedResource {
    fn stops(&self) -> usize {
        self.stop_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}
impl ResourceHandle for FakeBorrowedResource {
    fn request_cancel(&self) {
        self.stop_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn is_released(&self) -> bool {
        false
    }
    fn force_close_owned(&self) {
        self.stop_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn p2_two_clients_shared_host_last_client_unloads_once_without_shutting_host() {
    let registry = registry_for(&["media", "text"], RegistryVerification::RuntimeVerified);
    let host = FakeHost::new();
    let mut bridge = IntegrationBridge::new();
    for app in ["media", "text"] {
        bridge
            .register_adapter(Box::new(adapter(
                app,
                registry.clone(),
                host.clone(),
                vec![Capability::RequestMediaTask],
            )))
            .unwrap();
        bridge
            .permissions_mut()
            .grant(app, app, Capability::RequestMediaTask)
            .unwrap();
    }
    let mut lifecycle = LifecycleSupervisor::default();
    for app in ["media", "text"] {
        lifecycle.start_session(app).unwrap();
        let action = request(
            app,
            Capability::RequestMediaTask,
            Arguments::HostTask(task(&format!("{app}-job"), "ref-v1-abc123")),
        );
        let ticket = bridge
            .permissions_mut()
            .owner_approved_action(app, &action, app)
            .unwrap();
        bridge
            .invoke(
                app,
                &action,
                Some(ticket),
                &CancellationToken::default(),
                &mut lifecycle,
                app,
            )
            .unwrap();
    }
    assert_eq!(host.active(), 2);
    lifecycle
        .shutdown("media", ShutdownReason::Exit, std::time::Duration::ZERO)
        .unwrap();
    assert_eq!(host.active(), 1);
    assert_eq!(host.unloads(), 0);
    lifecycle
        .shutdown("text", ShutdownReason::Exit, std::time::Duration::ZERO)
        .unwrap();
    assert_eq!(host.active(), 0);
    assert_eq!(host.unloads(), 1);
}

#[test]
fn p2_cannot_override_registry_status_or_cross_app_host_authorization() {
    for status in [
        RegistryVerification::Missing,
        RegistryVerification::Incompatible,
    ] {
        let registry = registry_for(&["media"], status);
        let host = FakeHost::new();
        let mut bridge = IntegrationBridge::new();
        bridge
            .register_adapter(Box::new(adapter(
                "media",
                registry,
                host.clone(),
                vec![Capability::RequestMediaTask],
            )))
            .unwrap();
        bridge
            .permissions_mut()
            .grant("jan", "media", Capability::RequestMediaTask)
            .unwrap();
        let mut lifecycle = LifecycleSupervisor::default();
        lifecycle.start_session("app").unwrap();
        let action = request(
            "media",
            Capability::RequestMediaTask,
            Arguments::HostTask(task("job", "ref-v1-abc123")),
        );
        let ticket = bridge
            .permissions_mut()
            .owner_approved_action("jan", &action, "app")
            .unwrap();
        assert_eq!(
            bridge.invoke(
                "jan",
                &action,
                Some(ticket),
                &CancellationToken::default(),
                &mut lifecycle,
                "app"
            ),
            Err(BridgeError::AdapterFailure)
        );
        assert_eq!(host.active(), 0);
    }

    let registry = registry_for(&["media"], RegistryVerification::RuntimeVerified);
    let mut host = FakeHost::new();
    host.permitted = false;
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("app").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job", "ref-v1-abc123")),
    );
    let ticket = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "app")
        .unwrap();
    assert_eq!(
        bridge.invoke(
            "jan",
            &action,
            Some(ticket),
            &CancellationToken::default(),
            &mut lifecycle,
            "app"
        ),
        Err(BridgeError::AdapterFailure)
    );
    assert_eq!(host.active(), 0);
}

#[test]
fn p2_mock_borrowed_host_model_no_unload_or_external_process_stop() {
    let registry = registry_for(&["media"], RegistryVerification::RuntimeVerified);
    let mut host = FakeHost::new();
    host.runtime_owner = HostRuntimeOwner::BorrowedExternal;
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("media-session").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job", "ref-v1-abc123")),
    );
    let ticket = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "media-session")
        .unwrap();
    bridge
        .invoke(
            "jan",
            &action,
            Some(ticket),
            &CancellationToken::default(),
            &mut lifecycle,
            "media-session",
        )
        .unwrap();
    lifecycle
        .shutdown(
            "media-session",
            ShutdownReason::Stop,
            std::time::Duration::ZERO,
        )
        .unwrap();
    assert_eq!(host.active(), 0);
    assert_eq!(host.unloads(), 0);
    assert_eq!(host.state.lock().unwrap().borrowed_process_stops, 0);
}

#[test]
fn p2_duplicate_job_id_does_not_cancel_preexisting_owned_host_lease() {
    let registry = registry_for(&["media"], RegistryVerification::RuntimeVerified);
    let host = FakeHost::new();
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("app").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job-1", "ref-v1-abc123")),
    );
    let first = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "app")
        .unwrap();
    bridge
        .invoke(
            "jan",
            &action,
            Some(first),
            &CancellationToken::default(),
            &mut lifecycle,
            "app",
        )
        .unwrap();
    assert_eq!(host.active(), 1);
    let duplicate = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "app")
        .unwrap();
    assert_eq!(
        bridge.invoke(
            "jan",
            &action,
            Some(duplicate),
            &CancellationToken::default(),
            &mut lifecycle,
            "app"
        ),
        Err(BridgeError::AdapterFailure)
    );
    assert_eq!(host.active(), 1); // no second handle to stop the first job
    assert_eq!(host.unloads(), 0);
    lifecycle
        .shutdown("app", ShutdownReason::Stop, std::time::Duration::ZERO)
        .unwrap();
    assert_eq!(host.active(), 0);
    assert_eq!(host.unloads(), 1);
}

#[test]
fn p2_approval_is_bound_to_host_session_and_host_checks_app_scopes() {
    let registry = registry_for(&["media", "text"], RegistryVerification::RuntimeVerified);
    let mut host = FakeHost::new();
    host.denied_apps.insert(id("text"));
    let mut bridge = IntegrationBridge::new();
    for app in ["media", "text"] {
        bridge
            .register_adapter(Box::new(adapter(
                app,
                registry.clone(),
                host.clone(),
                vec![Capability::RequestMediaTask],
            )))
            .unwrap();
        bridge
            .permissions_mut()
            .grant("jan", app, Capability::RequestMediaTask)
            .unwrap();
    }
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("media-owner").unwrap();
    lifecycle.start_session("other-owner").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job", "ref-v1-abc123")),
    );
    let ticket = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "media-owner")
        .unwrap();
    assert_eq!(
        bridge.invoke(
            "jan",
            &action,
            Some(ticket),
            &CancellationToken::default(),
            &mut lifecycle,
            "other-owner"
        ),
        Err(BridgeError::InvalidApproval)
    );
    assert_eq!(host.active(), 0);
    assert!(bridge
        .invoke(
            "jan",
            &action,
            Some(ticket),
            &CancellationToken::default(),
            &mut lifecycle,
            "media-owner"
        )
        .is_ok());
    assert_eq!(host.active(), 1);

    let forbidden = request(
        "text",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job-2", "ref-v1-abc123")),
    );
    let approval = bridge
        .permissions_mut()
        .owner_approved_action("jan", &forbidden, "other-owner")
        .unwrap();
    assert_eq!(
        bridge.invoke(
            "jan",
            &forbidden,
            Some(approval),
            &CancellationToken::default(),
            &mut lifecycle,
            "other-owner"
        ),
        Err(BridgeError::AdapterFailure)
    );
    assert_eq!(host.active(), 1); // other app's live lease is preserved
    lifecycle
        .shutdown(
            "media-owner",
            ShutdownReason::Stop,
            std::time::Duration::ZERO,
        )
        .unwrap();
}

#[test]
fn p2_contradictory_trusted_host_route_rejected_before_lease() {
    let registry = registry_for(&["media"], RegistryVerification::RuntimeVerified);
    let mut host = FakeHost::new();
    host.contradictory_route = true;
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("app").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job", "ref-v1-abc123")),
    );
    let ticket = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "app")
        .unwrap();
    assert_eq!(
        bridge.invoke(
            "jan",
            &action,
            Some(ticket),
            &CancellationToken::default(),
            &mut lifecycle,
            "app"
        ),
        Err(BridgeError::AdapterFailure)
    );
    assert_eq!(host.active(), 0);
}

#[test]
fn p2_rejects_token_path_injection_and_unscoped_aipath_fields() {
    assert_eq!(
        HostId::new("http://unapproved"),
        Err(BoundaryError::InvalidIdentifier)
    );
    assert_eq!(
        JobId::new("../escape"),
        Err(BoundaryError::InvalidIdentifier)
    );
    assert_eq!(
        RegistryModelId::new("ref-v1-../../secret"),
        Err(BoundaryError::InvalidIdentifier)
    );
    assert_eq!(
        StartupReferences::parse_proposal(
            "schema=1\napp_id=media\nmanager_mode=optional\nremote_host_id=paired-pc-01\nfallback=legacy\ntoken=abc"
        ),
        Err(ReferenceError::UnknownField)
    );
    assert_eq!(
        StartupReferences::parse_proposal(
            "schema=2-proposal\napp_id=media\nmanager_mode=optional\nfallback=legacy\njwt=secret"
        ),
        Err(ReferenceError::UnknownField)
    );
}

#[test]
fn p2_explicit_cancellation_before_action_dispatch_keeps_host_idle() {
    let registry = registry_for(&["media"], RegistryVerification::RuntimeVerified);
    let host = FakeHost::new();
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("app").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job", "ref-v1-abc123")),
    );
    let ticket = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "app")
        .unwrap();
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    assert_eq!(
        bridge.invoke(
            "jan",
            &action,
            Some(ticket),
            &cancelled,
            &mut lifecycle,
            "app"
        ),
        Err(BridgeError::Cancelled)
    );
    assert_eq!(host.active(), 0);
    assert_eq!(host.unloads(), 0);
}

#[test]
fn p2_stop_before_authorized_action_rejects_new_tasks_and_keeps_host_idle() {
    let registry = registry_for(&["media"], RegistryVerification::RuntimeVerified);
    let host = FakeHost::new();
    let mut bridge = IntegrationBridge::new();
    bridge
        .register_adapter(Box::new(adapter(
            "media",
            registry,
            host.clone(),
            vec![Capability::RequestMediaTask],
        )))
        .unwrap();
    bridge
        .permissions_mut()
        .grant("jan", "media", Capability::RequestMediaTask)
        .unwrap();
    let mut lifecycle = LifecycleSupervisor::default();
    lifecycle.start_session("old").unwrap();
    let action = request(
        "media",
        Capability::RequestMediaTask,
        Arguments::HostTask(task("job-1", "ref-v1-abc123")),
    );
    let approval = bridge
        .permissions_mut()
        .owner_approved_action("jan", &action, "old")
        .unwrap();
    lifecycle
        .shutdown("old", ShutdownReason::Stop, std::time::Duration::ZERO)
        .unwrap();
    let result = bridge.invoke(
        "jan",
        &action,
        Some(approval),
        &CancellationToken::default(),
        &mut lifecycle,
        "old",
    );
    assert_eq!(result, Err(BridgeError::SessionStopped));
    assert_eq!(host.active(), 0);
}
