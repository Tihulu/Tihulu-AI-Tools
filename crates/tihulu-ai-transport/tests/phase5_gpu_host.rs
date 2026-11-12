//! Phase 5 offline acceptance: the REAL core ResourceLedger is the sole
//! model cleanup-request authority. All GPU/RAM values are test fixtures.
use std::collections::HashSet;
use tihulu_ai_core::resource_lifecycle::ResourcePhase;
use tihulu_ai_transport::*;

fn id(s: &str) -> HostId {
    HostId::new(s).unwrap()
}
fn client(s: &str) -> ClientId {
    ClientId::new(s).unwrap()
}
fn app(s: &str) -> AppId {
    AppId::new(s).unwrap()
}
fn model(s: &str) -> ModelId {
    ModelId::new(s).unwrap()
}
fn lease(s: &str) -> LeaseId {
    LeaseId::new(s).unwrap()
}
fn pin(c: &str) -> String {
    c.repeat(64)
}
fn peer(device: &str, application: &str) -> PeerEvidence {
    PeerEvidence {
        host: id("pc-gpu-host"),
        client: client(device),
        app: app(application),
        app_session: AppSessionId::new("launch-1").unwrap(),
        task: TaskId::new("task-1").unwrap(),
        app_authentication: AppAuthentication::VerifiedScopedCredential,
        client_cert_sha256: if device == "pi-one" { pin("b") } else { pin("c") },
        server_cert_sha256: pin("a"),
        security: ChannelSecurity::VerifiedMutualTls,
        route: NetworkRoute::DirectEthernet,
    }
}
fn grant() -> AppGrant {
    AppGrant {
        allowed_models: [model("owned"), model("jan")].into_iter().collect(),
        allowed_tasks: [TaskKind::Chat, TaskKind::Transcription]
            .into_iter()
            .collect::<HashSet<_>>(),
        max_leases: 4,
        max_nonterminal_jobs: 3,
        max_payload_bytes: 8192,
        max_model_vram_mib: 8192,
    }
}
fn setup() -> (MockComputeHost, AuthoritativeMockLedgerBridge) {
    let mut host = MockComputeHost::new(id("pc-gpu-host"), pin("a"), 30, 5, 12).unwrap();
    host.register_model(ModelRegistration {
        id: model("owned"),
        location: ModelLocation::HostStored,
        ownership: RuntimeOwnership::HostOwned,
        estimated_vram_mib: 2048,
    }).unwrap();
    host.register_model(ModelRegistration {
        id: model("jan"),
        location: ModelLocation::HostStored,
        ownership: RuntimeOwnership::BorrowedExternal,
        estimated_vram_mib: 10000,
    }).unwrap();
    for (name, p) in [("pi-one", "b"), ("pi-two", "c")] {
        host.pair_client(PairingApproval {
            client: client(name),
            client_cert_sha256: pin(p),
            user_approved: true,
        }, 0).unwrap();
        for app_name in ["advisor", "text"] {
            host.set_grant(client(name), app(app_name), grant(), 0).unwrap();
        }
    }
    host.set_lifecycle_authority(LifecycleAuthority::CoordinatorLedger).unwrap();
    let mut bridge = AuthoritativeMockLedgerBridge::new();
    bridge.bind_runtime(&mut host, LedgerRuntimeBinding {
        model: model("owned"),
        runtime: RuntimeId::new("owned-runtime-generation-1").unwrap(),
        resource: LedgerResourceId(500),
    }, RuntimeOwnership::HostOwned, 768, 2048).unwrap();
    bridge.bind_runtime(&mut host, LedgerRuntimeBinding {
        model: model("jan"),
        runtime: RuntimeId::new("external-jan-provider").unwrap(),
        resource: LedgerResourceId(501),
    }, RuntimeOwnership::BorrowedExternal, 0, 0).unwrap();
    bridge.register_app_owner(
        &mut host, client("pi-one"), app("advisor"),
        AppSessionId::new("launch-1").unwrap(), LedgerOwnerId(11),
    ).unwrap();
    bridge.register_app_owner(
        &mut host, client("pi-two"), app("text"),
        AppSessionId::new("launch-1").unwrap(), LedgerOwnerId(12),
    ).unwrap();
    (host, bridge)
}
fn request(name: &str, lease_name: &str) -> RequestSpec {
    RequestSpec {
        id: RequestId::new(name).unwrap(),
        lease: lease(lease_name),
        task: TaskKind::Chat,
        payload_bytes: 96,
    }
}
fn successful_mock_release() -> MockCleanupObservation {
    MockCleanupObservation {
        joined_owned_workers: true,
        disposed_owned_context: true,
        remaining_attributable_ram_mib: 0,
        remaining_attributable_vram_mib: 0,
    }
}

#[test]
fn real_core_ledger_two_pi_clients_last_owned_lease_only_once() {
    let (mut host, mut bridge) = setup();
    let first = peer("pi-one", "advisor");
    let second = peer("pi-two", "text");
    host.open_lease(&first, lease("first"), model("owned"), 0).unwrap();
    host.open_lease(&second, lease("second"), model("owned"), 0).unwrap();
    assert_eq!(bridge.consume_host_events(&mut host, 1).unwrap(), 2);
    assert_eq!(bridge.active_core_leases(LedgerResourceId(500)), Some(2));
    assert_eq!(host.model_load_counts(&model("owned")), Some((1, 0)));
    host.close_app(&first, 2).unwrap();
    assert_eq!(bridge.consume_host_events(&mut host, 2).unwrap(), 1);
    assert_eq!(bridge.active_core_leases(LedgerResourceId(500)), Some(1));
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 0);
    assert_eq!(host.total_owned_vram_mib(), 2048);
    host.submit(&second, request("second-still-works", "second"), 3).unwrap();
    host.close_lease(&second, &lease("second"), 4).unwrap();
    assert_eq!(bridge.consume_host_events(&mut host, 4).unwrap(), 1);
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 1);
    assert_eq!(bridge.active_core_leases(LedgerResourceId(500)), Some(0));
    assert!(host.awaiting_ledger_unload(&model("owned")));
    assert_eq!(host.total_owned_vram_mib(), 2048);
    assert_eq!(bridge.cleanup_phase(LedgerResourceId(500)), Some(ResourcePhase::CleanupPending));
    assert_eq!(bridge.pending_cleanup(LedgerResourceId(500)).unwrap().expected_ram_mib, 768);
    assert_eq!(bridge.pending_cleanup(LedgerResourceId(500)).unwrap().expected_vram_mib, 2048);
    let report = bridge.finish_mock_cleanup(
        &mut host, LedgerResourceId(500), successful_mock_release(), 5,
    ).unwrap();
    assert!(report.succeeded_in_fixture);
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(host.model_load_counts(&model("owned")), Some((1, 1)));
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 1);
    assert_eq!(bridge.cleanup_phase(LedgerResourceId(500)), None);
    assert_eq!(host.status(), HostStatus::Accepting);
    assert_eq!(host.close_lease(&second, &lease("second"), 6), Ok(()));
    assert_eq!(bridge.consume_host_events(&mut host, 6).unwrap(), 0);
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 1);
}

#[test]
fn borrowed_external_provider_never_receives_a_cleanup_request() {
    let (mut host, mut bridge) = setup();
    host.open_lease(&peer("pi-one", "advisor"), lease("jan-a"), model("jan"), 0).unwrap();
    host.open_lease(&peer("pi-two", "text"), lease("jan-b"), model("jan"), 0).unwrap();
    bridge.consume_host_events(&mut host, 1).unwrap();
    host.close_app(&peer("pi-one", "advisor"), 2).unwrap();
    host.close_app(&peer("pi-two", "text"), 3).unwrap();
    assert_eq!(bridge.consume_host_events(&mut host, 4).unwrap(), 2);
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(501)), 0);
    assert_eq!(bridge.cleanup_phase(LedgerResourceId(501)), None);
    assert!(bridge.pending_cleanup(LedgerResourceId(501)).is_none());
    assert_eq!(host.model_load_counts(&model("jan")), Some((0, 0)));
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(
        bridge.finish_mock_cleanup(
            &mut host, LedgerResourceId(501), successful_mock_release(), 5,
        ),
        Err(BridgeError::NoCleanupRequest),
    );
}

#[test]
fn last_lease_failure_stays_reported_until_core_authorized_retry() {
    let (mut host, mut bridge) = setup();
    let first = peer("pi-one", "advisor");
    host.open_lease(&first, lease("one"), model("owned"), 0).unwrap();
    bridge.consume_host_events(&mut host, 0).unwrap();
    host.close_lease(&first, &lease("one"), 1).unwrap();
    bridge.consume_host_events(&mut host, 1).unwrap();
    let report = bridge.finish_mock_cleanup(&mut host, LedgerResourceId(500),
        MockCleanupObservation {
            joined_owned_workers: false,
            disposed_owned_context: false,
            remaining_attributable_ram_mib: 64,
            remaining_attributable_vram_mib: 120,
        }, 2).unwrap();
    assert!(!report.succeeded_in_fixture);
    assert_eq!(report.failures.len(), 4);
    assert_eq!(bridge.cleanup_phase(LedgerResourceId(500)), Some(ResourcePhase::CleanupFailed));
    assert_eq!(host.total_owned_vram_mib(), 2048);
    assert!(host.awaiting_ledger_unload(&model("owned")));
    assert_eq!(host.open_lease(&peer("pi-two", "text"), lease("new"), model("owned"), 3),
        Err(Error::LedgerCleanupPending));
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 1);
    let retry = bridge.retry_failed_cleanup(LedgerResourceId(500)).unwrap();
    assert_eq!(retry.retry_number, 1);
    assert_eq!(bridge.cleanup_phase(LedgerResourceId(500)), Some(ResourcePhase::CleanupPending));
    let recovered = bridge.finish_mock_cleanup(
        &mut host, LedgerResourceId(500), successful_mock_release(), 4,
    ).unwrap();
    assert!(recovered.succeeded_in_fixture);
    assert_eq!(recovered.retry_number, 1);
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 1);
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(bridge.reports().len(), 2);
    assert_eq!(bridge.retry_failed_cleanup(LedgerResourceId(500)),
        Err(BridgeError::NoCleanupRequest));
}

#[test]
fn per_client_cancel_and_running_disconnect_do_not_affect_second_pi() {
    let (host, mut bridge) = setup();
    let mut transport = MockTransport::new(host);
    let first = peer("pi-one", "advisor");
    let second = peer("pi-two", "text");
    transport.exchange(&first, WireCommand::OpenLease {
        lease: lease("a"), model: model("owned"),
    }, 0).unwrap();
    transport.exchange(&second, WireCommand::OpenLease {
        lease: lease("b"), model: model("owned"),
    }, 0).unwrap();
    bridge.consume_host_events(&mut transport.host, 0).unwrap();
    transport.exchange(&first, WireCommand::Submit {
        request: request("a-job", "a"),
    }, 1).unwrap();
    transport.exchange(&second, WireCommand::Submit {
        request: request("b-job", "b"),
    }, 1).unwrap();
    assert_eq!(transport.exchange(&second, WireCommand::Cancel {
        request: RequestId::new("a-job").unwrap(),
    }, 1), Err(Error::NotOwner));
    assert_eq!(transport.host.start_next_job(2), Some(RequestId::new("a-job").unwrap()));
    transport.set_client_connected(client("pi-one"), false);
    assert_eq!(transport.exchange(&first, WireCommand::Heartbeat {
        lease: lease("a"),
    }, 20), Err(Error::TransportUnavailable));
    transport.exchange(&second, WireCommand::Heartbeat { lease: lease("b") }, 20).unwrap();
    transport.host.tick(30);
    assert_eq!(bridge.consume_host_events(&mut transport.host, 30).unwrap(), 0);
    assert_eq!(transport.host.job_status(&RequestId::new("a-job").unwrap()),
        Some(JobStatus::Cancelling));
    assert_eq!(transport.host.job_status(&RequestId::new("b-job").unwrap()),
        Some(JobStatus::Queued));
    transport.host.tick(35);
    assert_eq!(bridge.consume_host_events(&mut transport.host, 35).unwrap(), 1);
    assert_eq!(bridge.active_core_leases(LedgerResourceId(500)), Some(1));
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 0);
    assert_eq!(transport.host.total_owned_vram_mib(), 2048);
    transport.set_client_connected(client("pi-one"), true);
    assert_eq!(transport.exchange(&first, WireCommand::OpenLease {
        lease: lease("a"), model: model("owned"),
    }, 36), Err(Error::LeaseExpired));
    assert_eq!(transport.exchange(&first, WireCommand::OpenLease {
        lease: lease("reconnect-a"), model: model("owned"),
    }, 36), Ok(WireResponse::Accepted));
    assert_eq!(bridge.consume_host_events(&mut transport.host, 36).unwrap(), 1);
    assert_eq!(bridge.active_core_leases(LedgerResourceId(500)), Some(2));
    assert_eq!(transport.host.status(), HostStatus::Accepting);
}

#[test]
fn host_admin_unpair_closes_its_leases_without_killing_other_client() {
    let (mut host, mut bridge) = setup();
    let one = peer("pi-one", "advisor");
    let two = peer("pi-two", "text");
    host.open_lease(&one, lease("one"), model("owned"), 0).unwrap();
    host.open_lease(&two, lease("two"), model("owned"), 0).unwrap();
    bridge.consume_host_events(&mut host, 0).unwrap();
    host.unpair_client(&client("pi-one"), 1);
    bridge.consume_host_events(&mut host, 2).unwrap();
    assert_eq!(bridge.active_core_leases(LedgerResourceId(500)), Some(1));
    assert_eq!(host.heartbeat(&one, &lease("one"), 3), Err(Error::PairingRequired));
    assert_eq!(host.heartbeat(&two, &lease("two"), 3), Ok(()));
    assert_eq!(host.total_owned_vram_mib(), 2048);
    assert_eq!(host.status(), HostStatus::Accepting);
}

#[test]
fn host_drain_waits_for_real_ledger_request_and_mock_disposal_ack() {
    let (mut host, mut bridge) = setup();
    host.open_lease(&peer("pi-one", "advisor"), lease("owned"), model("owned"), 0).unwrap();
    bridge.consume_host_events(&mut host, 0).unwrap();
    host.begin_host_drain(1);
    bridge.consume_host_events(&mut host, 1).unwrap();
    assert_eq!(host.status(), HostStatus::Draining);
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(500)), 1);
    bridge.finish_mock_cleanup(
        &mut host, LedgerResourceId(500), successful_mock_release(), 2,
    ).unwrap();
    assert_eq!(host.status(), HostStatus::Drained);
    assert_eq!(host.total_owned_vram_mib(), 0);
}

#[test]
fn mock_pairing_requires_explicit_match_expires_and_prevents_replay() {
    let mut broker = PairingBroker::new();
    let attempt = PairingAttemptId::new("pair-1").unwrap();
    let offer = PairingOffer {
        attempt: attempt.clone(),
        host: id("pc-gpu-host"),
        client: client("pi-one"),
        server_cert_sha256: pin("a"),
        client_cert_sha256: pin("b"),
        expires_at_ms: 50,
    };
    broker.propose(offer.clone(), 0).unwrap();
    assert_eq!(broker.propose(offer.clone(), 0), Err(PairingError::DuplicateAttempt));
    let mut proof = MockOutOfBandApproval {
        host: id("pc-gpu-host"),
        client: client("pi-one"),
        observed_server_cert_sha256: pin("a"),
        observed_client_cert_sha256: pin("b"),
        trusted_reviewer_authenticated: false,
        user_confirmed_both_fingerprints: true,
    };
    assert!(matches!(broker.approve_mock(&attempt, &proof, 3),
        Err(PairingError::UnauthenticatedReviewer)));
    proof.trusted_reviewer_authenticated = true;
    proof.observed_client_cert_sha256 = pin("d");
    assert!(matches!(broker.approve_mock(&attempt, &proof, 4),
        Err(PairingError::MismatchedIdentity)));
    proof.observed_client_cert_sha256 = pin("b");
    let approved = broker.approve_mock(&attempt, &proof, 5).unwrap();
    assert_eq!(approved.client, client("pi-one"));
    assert!(approved.user_approved);
    assert!(matches!(broker.approve_mock(&attempt, &proof, 6),
        Err(PairingError::UnknownOrConsumed)));
    assert_eq!(broker.propose(offer, 7), Err(PairingError::DuplicateAttempt));
    let expiring = PairingOffer {
        attempt: PairingAttemptId::new("pair-2").unwrap(),
        host: id("pc-gpu-host"),
        client: client("pi-one"),
        server_cert_sha256: pin("a"),
        client_cert_sha256: pin("b"),
        expires_at_ms: 11,
    };
    broker.propose(expiring, 8).unwrap();
    assert!(matches!(broker.approve_mock(&PairingAttemptId::new("pair-2").unwrap(),
        &proof, 11), Err(PairingError::Expired)));
}

#[test]
fn gpu_capability_probe_only_reports_hints_never_infers_gpu_ready() {
    let mut probe = MockGpuCapabilityProbe::new();
    let gpu = GpuDeviceHint {
        device: AcceleratorId::new("gpu-zero").unwrap(),
        backend: AcceleratorBackend::Cuda,
        claimed_vram_total_mib: Some(16_000),
        claimed_vram_available_mib: Some(12_000),
        supports_inference_claim: true,
    };
    probe.put(GpuCapabilitySnapshot {
        host: id("pc-gpu-host"),
        at_ms: 123,
        evidence: CapabilityEvidence::MockFixture,
        devices: vec![gpu.clone()],
    }).unwrap();
    let snap = probe.probe(&id("pc-gpu-host")).unwrap();
    assert_eq!(snap.devices, vec![gpu.clone()]);
    assert!(!snap.verified_runnable());
    assert_eq!(probe.probe(&id("other-host")), Err(CapabilityError::UnknownHost));
    assert_eq!(probe.put(GpuCapabilitySnapshot {
        host: id("pc-other"),
        at_ms: 123,
        evidence: CapabilityEvidence::MockFixture,
        devices: vec![gpu.clone(), gpu.clone()],
    }), Err(CapabilityError::DuplicateDevice));
    assert_eq!(probe.put(GpuCapabilitySnapshot {
        host: id("pc-other"),
        at_ms: 123,
        evidence: CapabilityEvidence::UnverifiedReport,
        devices: vec![GpuDeviceHint {
            claimed_vram_available_mib: Some(20_000),
            ..gpu
        }],
    }), Err(CapabilityError::ImpossibleMemoryClaim));
}

#[test]
fn provenance_mismatch_and_nonledger_mock_authority_fail_closed() {
    let (mut host, _bridge) = setup();
    let mut other = AuthoritativeMockLedgerBridge::new();
    assert_eq!(other.bind_runtime(&mut host, LedgerRuntimeBinding {
        model: model("jan"),
        runtime: RuntimeId::new("bad").unwrap(),
        resource: LedgerResourceId(700),
    }, RuntimeOwnership::HostOwned, 10, 10), Err(BridgeError::InconsistentIdentity));
    let mut isolated = MockComputeHost::new(id("pc-gpu-host"), pin("a"), 30, 5, 12).unwrap();
    assert_eq!(other.consume_host_events(&mut isolated, 0),
        Err(BridgeError::WrongAuthority));
}

#[test]
fn new_ledger_resource_and_runtime_epoch_after_last_client_unload() {
    let (mut host, mut bridge) = setup();
    let old = peer("pi-one", "advisor");
    host.open_lease(&old, lease("old-lease"), model("owned"), 0).unwrap();
    bridge.consume_host_events(&mut host, 0).unwrap();
    host.close_app(&old, 1).unwrap();
    bridge.consume_host_events(&mut host, 1).unwrap();
    bridge.finish_mock_cleanup(
        &mut host, LedgerResourceId(500), successful_mock_release(), 2,
    ).unwrap();
    let mut fresh = old.clone();
    fresh.app_session = AppSessionId::new("launch-2").unwrap();
    fresh.task = TaskId::new("task-2").unwrap();
    bridge.register_app_owner(&mut host,
        client("pi-one"), app("advisor"), fresh.app_session.clone(), LedgerOwnerId(22)
    ).unwrap();
    assert_eq!(host.open_lease(&fresh, lease("fresh"), model("owned"), 3),
        Err(Error::LedgerBindingRequired));
    bridge.rotate_runtime(&mut host, LedgerRuntimeBinding {
        model: model("owned"),
        runtime: RuntimeId::new("owned-runtime-generation-2").unwrap(),
        resource: LedgerResourceId(600),
    }, RuntimeOwnership::HostOwned, 768, 2048).unwrap();
    host.open_lease(&fresh, lease("fresh"), model("owned"), 4).unwrap();
    bridge.consume_host_events(&mut host, 4).unwrap();
    assert_eq!(bridge.active_core_leases(LedgerResourceId(600)), Some(1));
    assert_eq!(host.model_load_counts(&model("owned")), Some((2, 1)));
    assert_eq!(bridge.cleanup_request_count(LedgerResourceId(600)), 0);
}
