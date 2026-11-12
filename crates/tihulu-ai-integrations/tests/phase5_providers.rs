//! Phase 5 offline *in-memory mock-server* contracts for Jan and Ollama.
//! NO sockets, provider installation, inference, process signals or file writes.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tihulu_ai_integrations::lifecycle::{
    CancellationToken, LifecycleSupervisor, Ownership, ResourceHandle, ResourceKind, SessionState,
    ShutdownReason,
};
use tihulu_ai_integrations::providers::{
    ActionTicket, JobState, ModelEvidence, ProviderAction, ProviderCaller, ProviderController,
    ProviderDiscoveryPort, ProviderError, ProviderExecutionPort, ProviderGrant, ProviderKind,
    ProviderModel, ProviderModelId, ProviderStatus,
};
use tihulu_ai_integrations::typed::{
    AppId, ClientId, JobId, OwnerSessionId, RegistryModelId, TaskKind,
};

fn caller(client: &str, app: &str, session: &str) -> ProviderCaller {
    ProviderCaller {
        client: ClientId::new(client).unwrap(),
        app: AppId::new(app).unwrap(),
        session: OwnerSessionId::new(session).unwrap(),
    }
}

fn registry(id: &str) -> RegistryModelId {
    RegistryModelId::new(id).unwrap()
}

fn provider_model(id: &str) -> ProviderModelId {
    ProviderModelId::new(id).unwrap()
}

fn record(
    kind: ProviderKind,
    reference: &str,
    model: &str,
    evidence: ModelEvidence,
) -> ProviderModel {
    ProviderModel {
        provider: kind,
        registry_model: registry(reference),
        provider_model: provider_model(model),
        tasks: BTreeSet::from([TaskKind::Chat, TaskKind::Transcription]),
        evidence,
    }
}

fn action(provider: ProviderKind, reference: &str, label: &str, job: &str) -> ProviderAction {
    ProviderAction {
        provider,
        registry_model: registry(reference),
        provider_model: provider_model(label),
        task: TaskKind::Chat,
        job: JobId::new(job).unwrap(),
        input_bytes: 32,
    }
}

fn grant(reference: &str) -> ProviderGrant {
    ProviderGrant {
        read_only_discovery: true,
        allowed_models: BTreeSet::from([registry(reference)]),
        allowed_tasks: BTreeSet::from([TaskKind::Chat]),
        max_request_bytes: 1024,
    }
}

#[derive(Default)]
struct MockServerState {
    statuses: BTreeMap<ProviderKind, ProviderStatus>,
    models: BTreeMap<ProviderKind, Vec<ProviderModel>>,
    status_calls: usize,
    list_calls: usize,
    dispatch_calls: usize,
    active: BTreeMap<String, ProviderKind>,
    cancelled_requests: usize,
    forced_client_stream_closes: usize,
    // These remain zero: foreign-process/config mutation operations do not
    // exist in the ProviderDiscoveryPort or ProviderExecutionPort API.
    foreign_process_terminations: usize,
    foreign_config_writes: usize,
}

#[derive(Clone, Default)]
struct MockServer {
    state: Arc<Mutex<MockServerState>>,
}

impl MockServer {
    fn ready() -> Self {
        let server = Self::default();
        {
            let mut state = server.state.lock().unwrap();
            state
                .statuses
                .insert(ProviderKind::JanDesktop, ProviderStatus::Ready);
            state
                .statuses
                .insert(ProviderKind::Ollama, ProviderStatus::Ready);
            state.models.insert(
                ProviderKind::JanDesktop,
                vec![record(
                    ProviderKind::JanDesktop,
                    "ref-v1-jan",
                    "jan:local",
                    ModelEvidence::VerifiedRunnable,
                )],
            );
            state.models.insert(
                ProviderKind::Ollama,
                vec![record(
                    ProviderKind::Ollama,
                    "ref-v1-ollama",
                    "llama3:latest",
                    ModelEvidence::VerifiedRunnable,
                )],
            );
        }
        server
    }

    fn set_status(&self, provider: ProviderKind, status: ProviderStatus) {
        self.state.lock().unwrap().statuses.insert(provider, status);
    }

    fn set_models(&self, provider: ProviderKind, models: Vec<ProviderModel>) {
        self.state.lock().unwrap().models.insert(provider, models);
    }

    fn active_count(&self) -> usize {
        self.state.lock().unwrap().active.len()
    }

    fn assert_foreign_untouched(&self) {
        let state = self.state.lock().unwrap();
        assert_eq!(state.foreign_process_terminations, 0);
        assert_eq!(state.foreign_config_writes, 0);
    }
}

impl ProviderDiscoveryPort for MockServer {
    fn status(&self, provider: ProviderKind) -> ProviderStatus {
        let mut state = self.state.lock().unwrap();
        state.status_calls += 1;
        state
            .statuses
            .get(&provider)
            .copied()
            .unwrap_or(ProviderStatus::Unavailable)
    }

    fn list_models(&self, provider: ProviderKind) -> Result<Vec<ProviderModel>, ProviderError> {
        let mut state = self.state.lock().unwrap();
        state.list_calls += 1;
        Ok(state.models.get(&provider).cloned().unwrap_or_default())
    }
}

fn job_key(caller: &ProviderCaller, job: &JobId) -> String {
    format!(
        "{}|{}|{}|{}",
        caller.client.as_str(),
        caller.app.as_str(),
        caller.session.as_str(),
        job.as_str()
    )
}

struct MockClientStream {
    state: Arc<Mutex<MockServerState>>,
    key: String,
}

impl ResourceHandle for MockClientStream {
    fn request_cancel(&self) {
        let mut state = self.state.lock().unwrap();
        if state.active.remove(&self.key).is_some() {
            state.cancelled_requests += 1;
        }
    }

    fn is_released(&self) -> bool {
        !self.state.lock().unwrap().active.contains_key(&self.key)
    }

    fn force_close_owned(&self) {
        let mut state = self.state.lock().unwrap();
        if state.active.remove(&self.key).is_some() {
            state.forced_client_stream_closes += 1;
        }
    }
}

impl ProviderExecutionPort for MockServer {
    fn prepare_owned_request(
        &self,
        caller: &ProviderCaller,
        action: &ProviderAction,
    ) -> Result<Arc<dyn ResourceHandle>, ProviderError> {
        let key = job_key(caller, &action.job);
        // This only prepares an inert handle. No foreign provider is accessed.
        if self.state.lock().unwrap().active.contains_key(&key) {
            return Err(ProviderError::DuplicateJob);
        }
        Ok(Arc::new(MockClientStream {
            state: self.state.clone(),
            key,
        }))
    }

    fn dispatch_mock(
        &self,
        caller: &ProviderCaller,
        action: &ProviderAction,
        cancellation: &CancellationToken,
    ) -> Result<(), ProviderError> {
        if cancellation.is_cancelled() {
            return Err(ProviderError::SessionStopped);
        }
        let key = job_key(caller, &action.job);
        let mut state = self.state.lock().unwrap();
        if state.active.contains_key(&key) {
            return Err(ProviderError::DuplicateJob);
        }
        state.dispatch_calls += 1;
        state.active.insert(key, action.provider);
        Ok(())
    }
}

type Controller = ProviderController<MockServer, MockServer>;

fn setup() -> (Controller, MockServer, LifecycleSupervisor) {
    let server = MockServer::ready();
    let controller = ProviderController::new(server.clone(), server.clone());
    (controller, server, LifecycleSupervisor::default())
}

fn authorize(
    controller: &mut Controller,
    lifecycle: &mut LifecycleSupervisor,
    owner: &ProviderCaller,
    provider: ProviderKind,
    model_id: &str,
) {
    if lifecycle.state(owner.session.as_str()).is_none() {
        lifecycle.start_session(owner.session.as_str()).unwrap();
    }
    controller
        .approve_connection(owner.clone(), provider, true, grant(model_id), lifecycle)
        .unwrap();
}

fn approve(
    controller: &mut Controller,
    lifecycle: &LifecycleSupervisor,
    owner: &ProviderCaller,
    request: &ProviderAction,
) -> ActionTicket {
    controller
        .approve_action(owner, request, true, lifecycle)
        .unwrap()
}

#[test]
fn unauthorized_or_declined_consent_never_probes_either_provider() {
    let (mut controller, server, mut lifecycle) = setup();
    let jan = caller("client", "media", "media-session");
    lifecycle.start_session(jan.session.as_str()).unwrap();
    assert_eq!(
        controller.provider_status(&jan, ProviderKind::JanDesktop, &lifecycle),
        ProviderStatus::AuthorizationRequired
    );
    assert_eq!(
        controller.list_models(&jan, ProviderKind::Ollama, &lifecycle),
        Err(ProviderError::AuthorizationRequired)
    );
    assert_eq!(
        controller.approve_connection(
            jan.clone(),
            ProviderKind::JanDesktop,
            false,
            grant("ref-v1-jan"),
            &lifecycle
        ),
        Err(ProviderError::AuthorizationRequired)
    );
    let state = server.state.lock().unwrap();
    assert_eq!(state.status_calls, 0);
    assert_eq!(state.list_calls, 0);
    assert_eq!(state.dispatch_calls, 0);
}

#[test]
fn approved_readonly_discovery_reports_provider_status_and_model_provenance() {
    let (mut controller, server, mut lifecycle) = setup();
    let jan = caller("jan-ui", "media", "media-session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &jan,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    let before = server.state.lock().unwrap().status_calls;
    assert!(before == 0); // Consent alone never probes Jan.
    assert_eq!(
        controller.provider_status(&jan, ProviderKind::JanDesktop, &lifecycle),
        ProviderStatus::Ready
    );
    let items = controller
        .list_models(&jan, ProviderKind::JanDesktop, &lifecycle)
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].provider_model.as_str(), "jan:local");
    assert_eq!(items[0].registry_model, registry("ref-v1-jan"));
    assert_eq!(items[0].provider, ProviderKind::JanDesktop);
    assert_eq!(items[0].evidence, ModelEvidence::VerifiedRunnable);
    assert_eq!(controller.active_mock_jobs(&jan), 0);
    assert_eq!(server.state.lock().unwrap().dispatch_calls, 0);
    // Separate optional grants: Jan permission is not Ollama permission.
    assert_eq!(
        controller.provider_status(&jan, ProviderKind::Ollama, &lifecycle),
        ProviderStatus::AuthorizationRequired
    );
    server.assert_foreign_untouched();
}

#[test]
fn unavailable_and_degraded_providers_fail_closed_without_execution() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "text", "session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::Ollama,
        "ref-v1-ollama",
    );
    server.set_status(ProviderKind::Ollama, ProviderStatus::Unavailable);
    assert_eq!(
        controller.provider_status(&owner, ProviderKind::Ollama, &lifecycle),
        ProviderStatus::Unavailable
    );
    assert_eq!(
        controller.list_models(&owner, ProviderKind::Ollama, &lifecycle),
        Err(ProviderError::ProviderUnavailable)
    );
    server.set_status(ProviderKind::Ollama, ProviderStatus::Degraded);
    assert_eq!(
        controller
            .list_models(&owner, ProviderKind::Ollama, &lifecycle)
            .unwrap()
            .len(),
        1
    );
    let request = action(
        ProviderKind::Ollama,
        "ref-v1-ollama",
        "llama3:latest",
        "job",
    );
    let ticket = approve(&mut controller, &lifecycle, &owner, &request);
    assert_eq!(
        controller.submit_mock(
            &owner,
            &request,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::ProviderUnavailable)
    );
    assert_eq!(server.active_count(), 0);
    server.set_status(ProviderKind::Ollama, ProviderStatus::Ready);
    // Unavailable/degraded attempts consumed their one-shot approval.
    assert_eq!(
        controller.submit_mock(
            &owner,
            &request,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::InvalidApproval)
    );
    server.assert_foreign_untouched();
}

#[test]
fn exact_one_shot_approval_blocks_replay_cross_client_and_wrong_model() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client-a", "media", "session-a");
    let attacker = caller("client-b", "media", "session-b");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    authorize(
        &mut controller,
        &mut lifecycle,
        &attacker,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    let request = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job-a");
    let wrong_job = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job-b");
    assert_eq!(
        controller.approve_action(&owner, &request, false, &lifecycle),
        Err(ProviderError::AuthorizationRequired)
    );
    assert_eq!(
        controller.submit_mock(
            &owner,
            &request,
            None,
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::ApprovalRequired)
    );
    let ticket = approve(&mut controller, &lifecycle, &owner, &request);
    assert_eq!(
        controller.submit_mock(
            &owner,
            &wrong_job,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::InvalidApproval)
    );
    assert_eq!(
        controller.submit_mock(
            &attacker,
            &request,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::InvalidApproval)
    );
    let receipt = controller
        .submit_mock(
            &owner,
            &request,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default(),
        )
        .unwrap();
    assert_eq!(receipt.job, JobId::new("job-a").unwrap());
    assert_eq!(receipt.state, JobState::Accepted);
    assert_eq!(server.active_count(), 1);
    assert_eq!(
        controller.submit_mock(
            &owner,
            &request,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::InvalidApproval)
    );
    assert_eq!(
        controller.cancel_owned(&attacker, &request.job),
        Err(ProviderError::UnknownJob)
    );
    assert_eq!(server.active_count(), 1);
}

#[test]
fn registry_scope_task_and_max_bytes_enforced_before_any_dispatch() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "media", "session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    let invalid_model = action(
        ProviderKind::JanDesktop,
        "ref-v1-other",
        "jan:local",
        "job-a",
    );
    assert_eq!(
        controller.approve_action(&owner, &invalid_model, true, &lifecycle),
        Err(ProviderError::ModelNotGranted)
    );
    let mut forbidden_task = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job-b");
    forbidden_task.task = TaskKind::Translation;
    assert_eq!(
        controller.approve_action(&owner, &forbidden_task, true, &lifecycle),
        Err(ProviderError::TaskNotGranted)
    );
    let mut huge = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job-c");
    huge.input_bytes = 5000;
    assert_eq!(
        controller.approve_action(&owner, &huge, true, &lifecycle),
        Err(ProviderError::RequestTooLarge)
    );
    let mut zero = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job-d");
    zero.input_bytes = 0;
    assert_eq!(
        controller.approve_action(&owner, &zero, true, &lifecycle),
        Err(ProviderError::RequestTooLarge)
    );
    assert_eq!(server.state.lock().unwrap().dispatch_calls, 0);
}

#[test]
fn listed_only_contradictory_and_wrong_provider_models_never_submit() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "media", "session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    let request = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job");
    server.set_models(
        ProviderKind::JanDesktop,
        vec![record(
            ProviderKind::JanDesktop,
            "ref-v1-jan",
            "jan:local",
            ModelEvidence::ListedOnly,
        )],
    );
    let ticket = approve(&mut controller, &lifecycle, &owner, &request);
    assert_eq!(
        controller.submit_mock(
            &owner,
            &request,
            Some(ticket),
            &mut lifecycle,
            CancellationToken::default()
        ),
        Err(ProviderError::ModelNotVerified)
    );
    server.set_models(
        ProviderKind::JanDesktop,
        vec![record(
            ProviderKind::Ollama,
            "ref-v1-jan",
            "jan:local",
            ModelEvidence::VerifiedRunnable,
        )],
    );
    assert_eq!(
        controller.list_models(&owner, ProviderKind::JanDesktop, &lifecycle),
        Err(ProviderError::ProviderMismatch)
    );
    server.set_models(
        ProviderKind::JanDesktop,
        vec![
            record(
                ProviderKind::JanDesktop,
                "ref-v1-jan",
                "jan:a",
                ModelEvidence::VerifiedRunnable,
            ),
            record(
                ProviderKind::JanDesktop,
                "ref-v1-jan",
                "jan:b",
                ModelEvidence::VerifiedRunnable,
            ),
        ],
    );
    assert_eq!(
        controller.list_models(&owner, ProviderKind::JanDesktop, &lifecycle),
        Err(ProviderError::ProviderMismatch)
    );
    assert_eq!(server.active_count(), 0);
}

#[test]
fn stop_and_quit_cancel_only_owned_requests_and_keep_borrowed_services_alive() {
    let (mut controller, server, mut lifecycle) = setup();
    let first = caller("client-a", "media", "session-a");
    let other = caller("client-b", "text", "session-b");
    authorize(
        &mut controller,
        &mut lifecycle,
        &first,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    authorize(
        &mut controller,
        &mut lifecycle,
        &first,
        ProviderKind::Ollama,
        "ref-v1-ollama",
    );
    authorize(
        &mut controller,
        &mut lifecycle,
        &other,
        ProviderKind::Ollama,
        "ref-v1-ollama",
    );

    let foreign = vec![
        Arc::new(ForeignSentinel::default()),
        Arc::new(ForeignSentinel::default()),
        Arc::new(ForeignSentinel::default()),
    ];
    for (kind, resource) in [
        ResourceKind::BorrowedJan,
        ResourceKind::BorrowedOllama,
        ResourceKind::BorrowedReadsb,
    ]
    .into_iter()
    .zip(foreign.iter())
    {
        lifecycle
            .track(
                first.session.as_str(),
                Ownership::BorrowedExternal,
                kind,
                CancellationToken::default(),
                resource.clone(),
            )
            .unwrap();
    }

    let a = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "a-jan");
    let b = action(
        ProviderKind::Ollama,
        "ref-v1-ollama",
        "llama3:latest",
        "a-ollama",
    );
    let c = action(
        ProviderKind::Ollama,
        "ref-v1-ollama",
        "llama3:latest",
        "b-ollama",
    );
    for (owner, request) in [(&first, &a), (&first, &b), (&other, &c)] {
        let ticket = approve(&mut controller, &lifecycle, owner, request);
        controller
            .submit_mock(
                owner,
                request,
                Some(ticket),
                &mut lifecycle,
                CancellationToken::default(),
            )
            .unwrap();
    }
    assert_eq!(server.active_count(), 3);
    assert_eq!(
        controller.cancel_owned(&other, &a.job),
        Err(ProviderError::UnknownJob)
    );
    assert_eq!(controller.cancel_owned(&first, &a.job), Ok(true));
    assert_eq!(controller.cancel_owned(&first, &a.job), Ok(false));
    assert_eq!(server.active_count(), 2); // Jan request closed; Ollama streams remain.

    let report = controller
        .stop_session(
            &first,
            ShutdownReason::HardStop,
            &mut lifecycle,
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(report.state, SessionState::Stopped);
    assert_eq!(report.borrowed_preserved, 3);
    assert_eq!(server.active_count(), 1); // Other app still using Ollama.
    assert_eq!(controller.active_mock_jobs(&first), 0);
    for resource in foreign {
        assert_eq!(resource.stops.load(Ordering::SeqCst), 0);
    }
    assert_eq!(
        controller.provider_status(&first, ProviderKind::Ollama, &lifecycle),
        ProviderStatus::Stopped
    );
    let another = controller
        .stop_session(&first, ShutdownReason::Exit, &mut lifecycle, Duration::ZERO)
        .unwrap();
    assert_eq!(another.state, SessionState::Stopped);
    assert_eq!(another.cancellation_requests, 0);
    let report = controller
        .stop_session(&other, ShutdownReason::Exit, &mut lifecycle, Duration::ZERO)
        .unwrap();
    assert_eq!(report.state, SessionState::Stopped);
    assert_eq!(server.active_count(), 0);
    server.assert_foreign_untouched();
}

#[derive(Default)]
struct ForeignSentinel {
    stops: AtomicUsize,
}

impl ResourceHandle for ForeignSentinel {
    fn request_cancel(&self) {
        self.stops.fetch_add(1, Ordering::SeqCst);
    }

    fn is_released(&self) -> bool {
        false
    }

    fn force_close_owned(&self) {
        self.stops.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn provider_revoke_only_cancels_that_provider_and_does_not_close_other_app() {
    let (mut controller, server, mut lifecycle) = setup();
    let first = caller("client-a", "media", "session-a");
    let other = caller("client-b", "text", "session-b");
    authorize(
        &mut controller,
        &mut lifecycle,
        &first,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    authorize(
        &mut controller,
        &mut lifecycle,
        &first,
        ProviderKind::Ollama,
        "ref-v1-ollama",
    );
    authorize(
        &mut controller,
        &mut lifecycle,
        &other,
        ProviderKind::Ollama,
        "ref-v1-ollama",
    );
    for (owner, request) in [
        (
            &first,
            action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "a"),
        ),
        (
            &first,
            action(ProviderKind::Ollama, "ref-v1-ollama", "llama3:latest", "b"),
        ),
        (
            &other,
            action(ProviderKind::Ollama, "ref-v1-ollama", "llama3:latest", "c"),
        ),
    ] {
        let ticket = approve(&mut controller, &lifecycle, owner, &request);
        controller
            .submit_mock(
                owner,
                &request,
                Some(ticket),
                &mut lifecycle,
                CancellationToken::default(),
            )
            .unwrap();
    }
    controller.revoke_provider(&first, ProviderKind::JanDesktop);
    assert_eq!(
        controller.provider_status(&first, ProviderKind::JanDesktop, &lifecycle),
        ProviderStatus::AuthorizationRequired
    );
    assert_eq!(server.active_count(), 2);
    assert_eq!(
        controller.provider_status(&first, ProviderKind::Ollama, &lifecycle),
        ProviderStatus::Ready
    );
    server.assert_foreign_untouched();
}

#[test]
fn session_binding_prevents_another_client_from_stealing_provider_grant() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("original", "media", "shared-session");
    let imposter = caller("imposter", "media", "shared-session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    assert_eq!(
        controller.approve_connection(
            imposter.clone(),
            ProviderKind::JanDesktop,
            true,
            grant("ref-v1-jan"),
            &lifecycle,
        ),
        Err(ProviderError::SessionCollision)
    );
    assert_eq!(
        controller.provider_status(&imposter, ProviderKind::JanDesktop, &lifecycle),
        ProviderStatus::AuthorizationRequired
    );
    assert_eq!(
        controller.stop_session(
            &imposter,
            ShutdownReason::HardStop,
            &mut lifecycle,
            Duration::ZERO
        ),
        Err(ProviderError::AuthorizationRequired)
    );
    assert_eq!(server.state.lock().unwrap().dispatch_calls, 0);
}

#[test]
fn model_ids_are_not_urls_paths_or_credentials_and_no_socket_is_opened() {
    assert_eq!(
        ProviderModelId::new("../private"),
        Err(ProviderError::InvalidRequest)
    );
    assert_eq!(
        ProviderModelId::new("https://localhost:11434"),
        Err(ProviderError::InvalidRequest)
    );
    assert_eq!(
        ProviderModelId::new("token=secret"),
        Err(ProviderError::InvalidRequest)
    );
    assert!(ProviderModelId::new("llama3.2:latest").is_ok());
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "media", "session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    assert_eq!(server.state.lock().unwrap().dispatch_calls, 0);
    server.assert_foreign_untouched();
}

#[test]
fn cancelled_token_prevents_dispatch_and_prepares_no_foreign_work() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "media", "session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::Ollama,
        "ref-v1-ollama",
    );
    let request = action(
        ProviderKind::Ollama,
        "ref-v1-ollama",
        "llama3:latest",
        "job",
    );
    let ticket = approve(&mut controller, &lifecycle, &owner, &request);
    let cancelled = CancellationToken::default();
    cancelled.cancel();
    assert_eq!(
        controller.submit_mock(&owner, &request, Some(ticket), &mut lifecycle, cancelled),
        Err(ProviderError::SessionStopped)
    );
    assert_eq!(server.state.lock().unwrap().dispatch_calls, 0);
    assert_eq!(server.active_count(), 0);
}

#[test]
fn renewing_user_consent_invalidates_old_one_shot_ticket() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "media", "session");
    authorize(
        &mut controller,
        &mut lifecycle,
        &owner,
        ProviderKind::JanDesktop,
        "ref-v1-jan",
    );
    let request = action(ProviderKind::JanDesktop, "ref-v1-jan", "jan:local", "job");
    let old_ticket = approve(&mut controller, &lifecycle, &owner, &request);
    controller
        .approve_connection(
            owner.clone(),
            ProviderKind::JanDesktop,
            true,
            grant("ref-v1-jan"),
            &lifecycle,
        )
        .unwrap();
    assert_eq!(
        controller.submit_mock(
            &owner,
            &request,
            Some(old_ticket),
            &mut lifecycle,
            CancellationToken::default(),
        ),
        Err(ProviderError::InvalidApproval)
    );
    assert_eq!(server.active_count(), 0);
    assert_eq!(server.state.lock().unwrap().dispatch_calls, 0);
}

#[test]
fn no_discovery_permission_means_no_model_list_call_after_connection_grant() {
    let (mut controller, server, mut lifecycle) = setup();
    let owner = caller("client", "media", "session");
    lifecycle.start_session(owner.session.as_str()).unwrap();
    let mut restricted = grant("ref-v1-jan");
    restricted.read_only_discovery = false;
    controller
        .approve_connection(
            owner.clone(),
            ProviderKind::JanDesktop,
            true,
            restricted,
            &lifecycle,
        )
        .unwrap();
    assert_eq!(
        controller.list_models(&owner, ProviderKind::JanDesktop, &lifecycle),
        Err(ProviderError::AuthorizationRequired)
    );
    assert_eq!(server.state.lock().unwrap().list_calls, 0);
}
