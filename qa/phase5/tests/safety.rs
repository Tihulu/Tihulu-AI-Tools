//! QA-owned black-box regressions against public APIs; synthetic temp data only.
//! These are MOCK acceptance checks, not hardware, OS installer or real provider tests.
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tihulu_ai_core::{
    OwnerId, Provenance, ResourceId, ResourceKind as LedgerKind, ResourceLedger, ResourcePhase,
};
use tihulu_ai_integrations::lifecycle::{
    CancellationToken, LifecycleSupervisor, Ownership, ResourceHandle,
    ResourceKind as ProviderKind, SessionState, ShutdownReason,
};
use tihulu_ai_registry::{
    app_default_model_root, plan_app_model_path_change, scan_approved_source, AppModelPathRecord,
    ChangeMode, DirectorySource, ModelLocationOwnership, PathPlanError, ReadApproval, ScanLimits,
};
use tihulu_ai_transport::{
    AppAuthentication, AppGrant, AppId, AppSessionId, ChannelSecurity, ClientId,
    Error as HostError, HostId, LeaseId, MockComputeHost, ModelId, ModelLocation,
    ModelRegistration, NetworkRoute, PairingApproval, PeerEvidence, RuntimeOwnership, TaskId,
    TaskKind,
};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);
struct SyntheticRoot(PathBuf);
impl SyntheticRoot {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "tihulu-qa08-{}-{nonce}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self) -> &Path {
        &self.0
    }
}
impl Drop for SyntheticRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove only QA-owned synthetic root");
    }
}

#[test]
fn qa_permission_denial_and_cross_scope_guard() {
    let selected = SyntheticRoot::new();
    let other = SyntheticRoot::new();
    fs::write(selected.path().join("model.gguf"), b"fixture").unwrap();
    let source = DirectorySource::CustomFolder {
        root: selected.path().to_owned(),
        owner_label: "qa-user".into(),
    };
    let denied = scan_approved_source(
        &source,
        &ReadApproval {
            approved: false,
            scope: selected.path().to_owned(),
        },
        ScanLimits::default(),
    );
    assert_eq!(denied.unwrap_err().kind(), ErrorKind::PermissionDenied);
    let outside = scan_approved_source(
        &source,
        &ReadApproval {
            approved: true,
            scope: other.path().to_owned(),
        },
        ScanLimits::default(),
    );
    assert_eq!(outside.unwrap_err().kind(), ErrorKind::PermissionDenied);
    assert_eq!(
        fs::read(selected.path().join("model.gguf")).unwrap(),
        b"fixture"
    );
}

#[test]
fn qa_jan_inventory_stays_in_approved_model_subtrees_without_modification() {
    let root = SyntheticRoot::new();
    let safe = root.path().join("llamacpp/models");
    let private = root.path().join("chats");
    fs::create_dir_all(&safe).unwrap();
    fs::create_dir_all(&private).unwrap();
    fs::write(safe.join("speech.gguf"), b"valid-fixture").unwrap();
    fs::write(private.join("secret.gguf"), b"never-scan").unwrap();
    fs::write(root.path().join("root.gguf"), b"never-scan").unwrap();
    let report = scan_approved_source(
        &DirectorySource::JanDesktop {
            data_root: root.path().to_owned(),
        },
        &ReadApproval {
            approved: true,
            scope: root.path().to_owned(),
        },
        ScanLimits::default(),
    )
    .unwrap();
    assert_eq!(report.artifacts.len(), 1);
    assert!(report.artifacts[0]
        .canonical_path
        .starts_with(safe.canonicalize().unwrap()));
    assert_eq!(
        fs::read(private.join("secret.gguf")).unwrap(),
        b"never-scan"
    );
    assert_eq!(
        fs::read(safe.join("speech.gguf")).unwrap(),
        b"valid-fixture"
    );
}

#[cfg(unix)]
#[test]
fn qa_jan_symlink_escape_is_not_discovered() {
    use std::os::unix::fs::symlink;
    let root = SyntheticRoot::new();
    let outside = SyntheticRoot::new();
    let safe = root.path().join("llamacpp/models");
    fs::create_dir_all(&safe).unwrap();
    fs::write(outside.path().join("stolen.gguf"), b"private").unwrap();
    symlink(outside.path().join("stolen.gguf"), safe.join("stolen.gguf")).unwrap();
    let report = scan_approved_source(
        &DirectorySource::JanDesktop {
            data_root: root.path().to_owned(),
        },
        &ReadApproval {
            approved: true,
            scope: root.path().to_owned(),
        },
        ScanLimits::default(),
    )
    .unwrap();
    assert!(report.artifacts.is_empty());
    assert!(report.skipped_symlinks >= 1);
}

#[test]
fn qa_independent_app_paths_are_plans_not_migrations() {
    let root = SyntheticRoot::new();
    let current = root.path().join("media-legacy");
    let destination = root.path().join("external-ssd-models");
    fs::create_dir(&current).unwrap();
    fs::write(current.join("existing.gguf"), b"leave-as-is").unwrap();
    for app in [
        "tihulu-media-center",
        "tihulu-text-center",
        "tihulu-satellite-center",
        "tihulu-flight-radar",
        "tihulu-rf-geolocation",
        "tihulu-survival-kit",
    ] {
        let suggested = app_default_model_root(root.path(), app).unwrap();
        assert!(suggested.ends_with(Path::new("apps").join(app)));
        assert!(!suggested.exists(), "proposal must not create files");
    }
    let mut record = AppModelPathRecord {
        app_id: "tihulu-media-center".into(),
        installation_path: Some(root.path().join("bin/media")),
        current_model_root: current.clone(),
        current_owner: ModelLocationOwnership::AppManaged,
        legacy_model_roots: vec![current.clone()],
        authorized_consumers: 1,
    };
    assert_eq!(
        plan_app_model_path_change(
            &record,
            &destination,
            root.path(),
            ChangeMode::CopyToManagedRoot,
            false
        ),
        Err(PathPlanError::UserApprovalRequired)
    );
    let plan = plan_app_model_path_change(
        &record,
        &destination,
        root.path(),
        ChangeMode::CopyToManagedRoot,
        true,
    )
    .unwrap();
    assert!(!plan.effectful_operation_executed);
    assert!(plan.native_verification_required && plan.backup_and_rollback_required);
    assert!(plan.preserve_original && plan.requires_app_adapter);
    assert!(!destination.exists(), "approved plan must not copy");
    assert_eq!(
        fs::read(current.join("existing.gguf")).unwrap(),
        b"leave-as-is"
    );
    assert!(record
        .installation_path
        .as_ref()
        .unwrap()
        .ends_with("bin/media"));
    record.current_owner = ModelLocationOwnership::BorrowedExternal;
    assert_eq!(
        plan_app_model_path_change(
            &record,
            &destination,
            root.path(),
            ChangeMode::PlanMoveWithRollback,
            true
        ),
        Err(PathPlanError::BorrowedModelCannotMove)
    );
    record.current_owner = ModelLocationOwnership::AppManaged;
    record.authorized_consumers = 2;
    assert_eq!(
        plan_app_model_path_change(
            &record,
            &destination,
            root.path(),
            ChangeMode::PlanMoveWithRollback,
            true
        ),
        Err(PathPlanError::SharedModelCannotMove)
    );
    assert_eq!(
        plan_app_model_path_change(
            &record,
            &root.path().join("..").join("secrets"),
            root.path(),
            ChangeMode::RepointInPlace,
            true
        ),
        Err(PathPlanError::InvalidPath)
    );
}

#[test]
fn qa_authoritative_shared_ledger_only_requests_last_owned_cleanup_once() {
    let mut ledger = ResourceLedger::new();
    let a = OwnerId(8001);
    let b = OwnerId(8002);
    let owned = ResourceId(9001);
    let borrowed = ResourceId(9002);
    ledger.open_owner(a).unwrap();
    ledger.open_owner(b).unwrap();
    ledger
        .register(
            a,
            owned,
            LedgerKind::ModelContext,
            Provenance::SharedManaged,
        )
        .unwrap();
    ledger.attach(b, owned).unwrap();
    ledger
        .register(
            a,
            borrowed,
            LedgerKind::NetworkSession,
            Provenance::BorrowedExternal,
        )
        .unwrap();
    assert!(
        ledger.stop_owner(a).unwrap().is_empty(),
        "borrowed provider is not unloaded"
    );
    assert_eq!(ledger.active_lease_count(owned), Some(1));
    let requests = ledger.stop_owner(b).unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].resource, owned);
    assert_eq!(
        ledger.resource_phase(owned),
        Some(ResourcePhase::CleanupPending)
    );
    assert!(
        ledger.stop_owner(b).unwrap().is_empty(),
        "second stop must be inert"
    );
    assert_eq!(ledger.resource_phase(borrowed), None);
    ledger.complete_cleanup(owned, true).unwrap();
    assert_eq!(ledger.resource_phase(owned), None);
}

#[derive(Default)]
struct ProbeHandle {
    cancelled: AtomicUsize,
    forced: AtomicUsize,
    released: AtomicBool,
}
impl ResourceHandle for ProbeHandle {
    fn request_cancel(&self) {
        self.cancelled.fetch_add(1, Ordering::SeqCst);
        self.released.store(true, Ordering::SeqCst);
    }
    fn is_released(&self) -> bool {
        self.released.load(Ordering::SeqCst)
    }
    fn force_close_owned(&self) {
        self.forced.fetch_add(1, Ordering::SeqCst);
    }
}
#[test]
fn qa_jan_ollama_and_foreign_services_are_never_killed_on_app_stop() {
    let mut supervisor = LifecycleSupervisor::default();
    supervisor.start_session("qa-app").unwrap();
    supervisor.start_session("other-app").unwrap();
    let owned = Arc::new(ProbeHandle::default());
    let borrowed: Vec<_> = (0..4).map(|_| Arc::new(ProbeHandle::default())).collect();
    supervisor
        .track(
            "qa-app",
            Ownership::AppOwned,
            ProviderKind::OwnedAiWorker,
            CancellationToken::default(),
            owned.clone(),
        )
        .unwrap();
    for (handle, kind) in borrowed.iter().zip([
        ProviderKind::BorrowedJan,
        ProviderKind::BorrowedOllama,
        ProviderKind::BorrowedReadsb,
        ProviderKind::BorrowedTar1090,
    ]) {
        supervisor
            .track(
                "qa-app",
                Ownership::BorrowedExternal,
                kind,
                CancellationToken::default(),
                handle.clone(),
            )
            .unwrap();
    }
    let report = supervisor
        .shutdown("qa-app", ShutdownReason::HardStop, Duration::ZERO)
        .unwrap();
    assert_eq!(report.state, SessionState::Stopped);
    assert_eq!(report.borrowed_preserved, 4);
    assert_eq!(report.cancellation_requests, 1);
    assert_eq!(owned.cancelled.load(Ordering::SeqCst), 1);
    assert_eq!(supervisor.state("other-app"), Some(SessionState::Running));
    for handle in borrowed {
        assert_eq!(handle.cancelled.load(Ordering::SeqCst), 0);
        assert_eq!(handle.forced.load(Ordering::SeqCst), 0);
    }
}
fn host_id() -> HostId {
    HostId::new("qa-pc").unwrap()
}
fn client_id(name: &str) -> ClientId {
    ClientId::new(name).unwrap()
}
fn app_id() -> AppId {
    AppId::new("media").unwrap()
}
fn model_id() -> ModelId {
    ModelId::new("qa-llm").unwrap()
}
fn peer(client: &str, fingerprint_char: &str) -> PeerEvidence {
    PeerEvidence {
        host: host_id(),
        client: client_id(client),
        app: app_id(),
        app_session: AppSessionId::new("launch-a").unwrap(),
        task: TaskId::new("task-a").unwrap(),
        app_authentication: AppAuthentication::VerifiedScopedCredential,
        client_cert_sha256: fingerprint_char.repeat(64),
        server_cert_sha256: "a".repeat(64),
        security: ChannelSecurity::VerifiedMutualTls,
        route: NetworkRoute::DirectEthernet,
    }
}
#[test]
fn qa_gpu_mock_isolates_clients_and_rejects_unverified_pairing() {
    let mut host = MockComputeHost::new(host_id(), "a".repeat(64), 30, 5, 10).unwrap();
    host.register_model(ModelRegistration {
        id: model_id(),
        location: ModelLocation::HostStored,
        ownership: RuntimeOwnership::HostOwned,
        estimated_vram_mib: 64,
    })
    .unwrap();
    let denial = host.pair_client(
        PairingApproval {
            client: client_id("unapproved"),
            client_cert_sha256: "b".repeat(64),
            user_approved: false,
        },
        0,
    );
    assert_eq!(denial, Err(HostError::PairingRequired));
    for (client, fingerprint_char) in [("client-a", "b"), ("client-b", "c")] {
        host.pair_client(
            PairingApproval {
                client: client_id(client),
                client_cert_sha256: fingerprint_char.repeat(64),
                user_approved: true,
            },
            0,
        )
        .unwrap();
        host.set_grant(
            client_id(client),
            app_id(),
            AppGrant {
                allowed_models: HashSet::from([model_id()]),
                allowed_tasks: HashSet::from([TaskKind::Chat]),
                max_leases: 2,
                max_nonterminal_jobs: 2,
                max_payload_bytes: 512,
                max_model_vram_mib: 1024,
            },
            0,
        )
        .unwrap();
    }
    let a = peer("client-a", "b");
    let b = peer("client-b", "c");
    let lease_a = LeaseId::new("lease-a").unwrap();
    let lease_b = LeaseId::new("lease-b").unwrap();
    host.open_lease(&a, lease_a.clone(), model_id(), 0).unwrap();
    host.open_lease(&b, lease_b.clone(), model_id(), 0).unwrap();
    assert_eq!(host.close_lease(&b, &lease_a, 1), Err(HostError::NotOwner));
    host.close_app(&a, 1).unwrap();
    assert_eq!(host.model_lease_count(&model_id()), 1);
    assert_eq!(host.total_owned_vram_mib(), 64);
    host.close_app(&b, 2).unwrap();
    assert_eq!(host.model_lease_count(&model_id()), 0);
    assert_eq!(host.model_load_counts(&model_id()), Some((1, 1)));
    assert_eq!(
        host.total_owned_vram_mib(),
        0,
        "mock only, not physical VRAM"
    );
}
