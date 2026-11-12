//! Contract regression tests; these are NOT real mTLS/GPU/hardware tests.
use std::collections::HashSet;
use tihulu_ai_transport::*;

fn host_id() -> HostId {
    HostId::new("paired-pc-01").unwrap()
}
fn client(n: &str) -> ClientId {
    ClientId::new(n).unwrap()
}
fn app(n: &str) -> AppId {
    AppId::new(n).unwrap()
}
fn model(n: &str) -> ModelId {
    ModelId::new(n).unwrap()
}
fn lease(n: &str) -> LeaseId {
    LeaseId::new(n).unwrap()
}
fn request(n: &str, l: &str) -> RequestSpec {
    RequestSpec {
        id: RequestId::new(n).unwrap(),
        lease: lease(l),
        task: TaskKind::Chat,
        payload_bytes: 64,
    }
}
fn fingerprint(n: &str) -> String {
    n.repeat(64)
}

fn evidence(client_id: &str, app_id: &str) -> PeerEvidence {
    PeerEvidence {
        host: host_id(),
        client: client(client_id),
        app: app(app_id),
        app_session: AppSessionId::new("launch-1").unwrap(),
        task: TaskId::new("task-1").unwrap(),
        app_authentication: AppAuthentication::VerifiedScopedCredential,
        client_cert_sha256: fingerprint(if client_id == "pi" {
            "b"
        } else if client_id == "pi-2" {
            "d"
        } else {
            "c"
        }),
        server_cert_sha256: fingerprint("a"),
        security: ChannelSecurity::VerifiedMutualTls,
        route: NetworkRoute::DirectEthernet,
    }
}
fn grant(models: &[&str]) -> AppGrant {
    AppGrant {
        allowed_models: models.iter().map(|m| model(m)).collect::<HashSet<_>>(),
        allowed_tasks: [TaskKind::Chat, TaskKind::Transcription]
            .into_iter()
            .collect(),
        max_leases: 3,
        max_nonterminal_jobs: 2,
        max_payload_bytes: 1024,
        max_model_vram_mib: 8000,
    }
}
fn setup() -> MockComputeHost {
    let mut host = MockComputeHost::new(host_id(), fingerprint("a"), 30, 5, 10).unwrap();
    for name in ["llm", "asr"] {
        host.register_model(ModelRegistration {
            id: model(name),
            location: ModelLocation::HostStored,
            ownership: RuntimeOwnership::HostOwned,
            estimated_vram_mib: 3000,
        })
        .unwrap();
    }
    host.register_model(ModelRegistration {
        id: model("external"),
        location: ModelLocation::HostStored,
        ownership: RuntimeOwnership::BorrowedExternal,
        estimated_vram_mib: 7777,
    })
    .unwrap();
    for (device, hash) in [("pi", "b"), ("pi-2", "d"), ("laptop", "c")] {
        host.pair_client(
            PairingApproval {
                client: client(device),
                client_cert_sha256: fingerprint(hash),
                user_approved: true,
            },
            0,
        )
        .unwrap();
        for app_id in ["advisor", "agent", "text"] {
            host.set_grant(
                client(device),
                app(app_id),
                grant(&["llm", "asr", "external"]),
                0,
            )
            .unwrap();
        }
    }
    host
}

#[test]
fn l03_two_apps_one_model_close_one_keeps_the_other_and_host() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("a"), model("llm"), 0)
        .unwrap();
    host.open_lease(&evidence("laptop", "text"), lease("b"), model("llm"), 0)
        .unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 2);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 0)));
    assert_eq!(host.total_owned_vram_mib(), 3000);
    host.close_app(&evidence("pi", "advisor"), 1).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(host.status(), HostStatus::Accepting);
    host.submit(
        &evidence("laptop", "text"),
        request("still-running", "b"),
        2,
    )
    .unwrap();
    assert_eq!(
        host.start_next_job(2),
        Some(RequestId::new("still-running").unwrap())
    );
    assert_eq!(
        host.finish_job(&RequestId::new("still-running").unwrap(), true, 3),
        Ok(JobStatus::Succeeded)
    );
}

#[test]
fn l04_last_lease_unloads_owned_vram_only_after_running_job_drained() {
    let mut host = setup();
    let peer = evidence("pi", "advisor");
    host.open_lease(&peer, lease("a"), model("llm"), 0).unwrap();
    host.submit(&peer, request("j", "a"), 1).unwrap();
    host.start_next_job(1);
    host.close_lease(&peer, &lease("a"), 2).unwrap();
    assert_eq!(
        host.job_status(&RequestId::new("j").unwrap()),
        Some(JobStatus::Cancelling)
    );
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(host.lease_status(&lease("a")), Some(LeaseStatus::Closing));
    host.tick(7);
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 1)));
    assert_eq!(host.lease_count(), 0);
    assert_eq!(host.status(), HostStatus::Accepting);
    assert_eq!(
        host.finish_job(&RequestId::new("j").unwrap(), true, 8),
        Ok(JobStatus::Cancelled)
    );
}

#[test]
fn l02_advisor_quit_does_not_cancel_agent_same_pi() {
    let mut host = setup();
    host.open_lease(
        &evidence("pi", "advisor"),
        lease("advisor-lease"),
        model("llm"),
        0,
    )
    .unwrap();
    host.open_lease(
        &evidence("pi", "agent"),
        lease("agent-lease"),
        model("llm"),
        0,
    )
    .unwrap();
    host.submit(
        &evidence("pi", "agent"),
        request("agent-job", "agent-lease"),
        1,
    )
    .unwrap();
    host.close_app(&evidence("pi", "advisor"), 2).unwrap();
    assert_eq!(host.lease_count(), 1);
    assert_eq!(
        host.job_status(&RequestId::new("agent-job").unwrap()),
        Some(JobStatus::Queued)
    );
    assert_eq!(host.total_owned_vram_mib(), 3000);
    host.close_app(&evidence("pi", "agent"), 3).unwrap();
    assert_eq!(host.total_owned_vram_mib(), 0);
}

#[test]
fn l09_disconnect_or_crash_orphan_expires_and_other_client_survives() {
    let mut transport = MockTransport::new(setup());
    let peer = evidence("pi", "advisor");
    transport
        .exchange(
            &peer,
            WireCommand::OpenLease {
                lease: lease("a"),
                model: model("llm"),
            },
            0,
        )
        .unwrap();
    transport
        .host
        .open_lease(&evidence("laptop", "agent"), lease("b"), model("llm"), 0)
        .unwrap();
    transport.set_connected(false);
    assert_eq!(
        transport.exchange(&peer, WireCommand::Heartbeat { lease: lease("a") }, 10),
        Err(Error::TransportUnavailable)
    );
    // Other client heartbeat is an independent connection in production;
    // this mock advances that lease directly to simulate its separate channel.
    transport
        .host
        .heartbeat(&evidence("laptop", "agent"), &lease("b"), 20)
        .unwrap();
    transport.host.tick(30);
    assert_eq!(transport.host.model_lease_count(&model("llm")), 1);
    assert_eq!(transport.host.total_owned_vram_mib(), 3000);
    transport.host.tick(51);
    assert_eq!(transport.host.total_owned_vram_mib(), 0);
}

#[test]
fn l12_ui_quit_does_not_terminate_separately_enabled_shared_host() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("ui"), model("llm"), 0)
        .unwrap();
    host.open_lease(
        &evidence("laptop", "text"),
        lease("headless"),
        model("llm"),
        0,
    )
    .unwrap();
    host.close_app(&evidence("pi", "advisor"), 1).unwrap();
    assert_eq!(host.status(), HostStatus::Accepting);
    assert_eq!(host.model_lease_count(&model("llm")), 1);
}

#[test]
fn l13_explicit_host_owner_drain_waits_for_grace_and_rejects_new_work() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.submit(&pi, request("j", "a"), 0).unwrap();
    host.start_next_job(1);
    host.begin_host_drain(2);
    assert_eq!(host.status(), HostStatus::Draining);
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(
        host.open_lease(&evidence("laptop", "text"), lease("next"), model("asr"), 3),
        Err(Error::ShuttingDown)
    );
    host.tick(7);
    assert_eq!(host.status(), HostStatus::Drained);
    assert_eq!(host.total_owned_vram_mib(), 0);
}

#[test]
fn l15_separate_models_only_unload_unreferenced_model() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("a"), model("llm"), 0)
        .unwrap();
    host.open_lease(&evidence("laptop", "text"), lease("b"), model("asr"), 0)
        .unwrap();
    assert_eq!(host.total_owned_vram_mib(), 6000);
    host.close_app(&evidence("pi", "advisor"), 1).unwrap();
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 1)));
    assert_eq!(host.model_load_counts(&model("asr")), Some((1, 0)));
}

#[test]
fn foreign_runtime_never_unloaded_or_counted_as_ours() {
    let mut host = setup();
    let peer = evidence("pi", "advisor");
    host.open_lease(&peer, lease("a"), model("external"), 0)
        .unwrap();
    assert_eq!(host.total_owned_vram_mib(), 0);
    host.close_lease(&peer, &lease("a"), 1).unwrap();
    assert_eq!(host.model_load_counts(&model("external")), Some((0, 0)));
    assert!(!host.audit().iter().any(|event| {
        event.action == AuditAction::ModelUnloaded && event.model == Some(model("external"))
    }));
}

#[test]
fn denial_checks_host_pin_client_pin_tls_and_network_route() {
    let mut host = setup();
    let mut peer = evidence("pi", "advisor");
    peer.security = ChannelSecurity::Unverified;
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::UnverifiedChannel)
    );
    peer.security = ChannelSecurity::VerifiedMutualTls;
    peer.app_authentication = AppAuthentication::Unverified;
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::UnverifiedApplication)
    );
    peer.app_authentication = AppAuthentication::VerifiedScopedCredential;
    peer.route = NetworkRoute::PublicInternet;
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::UnverifiedChannel)
    );
    peer.route = NetworkRoute::ApprovedVpn;
    peer.client_cert_sha256 = fingerprint("d");
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::IdentityMismatch)
    );
    peer.client_cert_sha256 = fingerprint("b");
    peer.server_cert_sha256 = fingerprint("d");
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::IdentityMismatch)
    );
    peer.server_cert_sha256 = fingerprint("a");
    peer.host = HostId::new("other-pc").unwrap();
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::IdentityMismatch)
    );
}

#[test]
fn request_replay_is_idempotent_cross_client_cancel_is_denied() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    let other = evidence("laptop", "text");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&other, lease("b"), model("llm"), 0)
        .unwrap();
    let original = request("job-1", "a");
    assert_eq!(host.submit(&pi, original.clone(), 1), Ok(JobStatus::Queued));
    assert_eq!(host.submit(&pi, original, 2), Ok(JobStatus::Queued));
    assert_eq!(
        host.submit(&other, request("job-1", "b"), 2),
        Err(Error::IdentifierConflict)
    );
    assert_eq!(
        host.cancel_request(&other, &RequestId::new("job-1").unwrap(), 2),
        Err(Error::NotOwner)
    );
    assert_eq!(
        host.close_lease(&other, &lease("a"), 2),
        Err(Error::NotOwner)
    );
    assert_eq!(
        host.job_status(&RequestId::new("job-1").unwrap()),
        Some(JobStatus::Queued)
    );
    assert_eq!(
        host.cancel_request(&pi, &RequestId::new("job-1").unwrap(), 3),
        Ok(JobStatus::Cancelled)
    );
    assert_eq!(
        host.cancel_request(&pi, &RequestId::new("job-1").unwrap(), 4),
        Ok(JobStatus::Cancelled)
    );
}

#[test]
fn quota_and_backpressure_and_task_grants() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    let oversized = RequestSpec {
        payload_bytes: 1025,
        ..request("big", "a")
    };
    assert_eq!(host.submit(&pi, oversized, 1), Err(Error::RequestTooLarge));
    let wrong_task = RequestSpec {
        task: TaskKind::Vision,
        ..request("vision", "a")
    };
    assert_eq!(
        host.submit(&pi, wrong_task, 1),
        Err(Error::PermissionDenied)
    );
    host.submit(&pi, request("j1", "a"), 1).unwrap();
    host.submit(&pi, request("j2", "a"), 1).unwrap();
    assert_eq!(
        host.submit(&pi, request("j3", "a"), 1),
        Err(Error::QuotaExceeded)
    );
    host.cancel_request(&pi, &RequestId::new("j1").unwrap(), 2)
        .unwrap();
    assert_eq!(
        host.submit(&pi, request("j3", "a"), 3),
        Ok(JobStatus::Queued)
    );
}

#[test]
fn heartbeat_replay_and_expiry_do_not_resurrect_stale_session() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.heartbeat(&pi, &lease("a"), 20).unwrap();
    host.tick(40);
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(
        host.heartbeat(&pi, &lease("a"), 50),
        Err(Error::UnknownLease)
    );
    assert_eq!(host.total_owned_vram_mib(), 0);
    host.close_lease(&pi, &lease("a"), 51).unwrap();
    assert_eq!(host.lease_count(), 0);
}

#[test]
fn consented_staging_is_distinct_from_host_stored_weights() {
    let mut host = setup();
    host.register_model(ModelRegistration {
        id: model("pi-digest"),
        location: ModelLocation::ClientStaged {
            sha256: fingerprint("e"),
            transfer_approved: false,
            integrity_verified: true,
            runtime_compatible: true,
        },
        ownership: RuntimeOwnership::HostOwned,
        estimated_vram_mib: 100,
    })
    .unwrap();
    host.set_grant(
        client("pi"),
        app("advisor"),
        grant(&["llm", "pi-digest"]),
        0,
    )
    .unwrap();
    assert_eq!(
        host.open_lease(
            &evidence("pi", "advisor"),
            lease("device-model"),
            model("pi-digest"),
            1
        ),
        Err(Error::StagingNotVerified)
    );
    assert_eq!(
        ModelId::new("../../home/pi/model.gguf"),
        Err(Error::InvalidIdentifier)
    );
    assert_eq!(
        ModelId::new("C:\\weights\\model"),
        Err(Error::InvalidIdentifier)
    );
}

#[test]
fn revoking_one_app_does_not_cancel_other_app_lease() {
    let mut host = setup();
    let advisor = evidence("pi", "advisor");
    host.open_lease(&advisor, lease("a"), model("llm"), 0)
        .unwrap();
    host.open_lease(&evidence("pi", "agent"), lease("b"), model("llm"), 0)
        .unwrap();
    host.revoke_app(&client("pi"), &app("advisor"), 1);
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(
        host.open_lease(&advisor, lease("new"), model("llm"), 2),
        Err(Error::PermissionDenied)
    );
}

#[test]
fn force_cancel_cannot_be_reversed_by_late_backend_callback() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.submit(&pi, request("j", "a"), 1).unwrap();
    host.start_next_job(1);
    host.close_client(&client("pi"), 2);
    host.tick(7);
    assert_eq!(
        host.finish_job(&RequestId::new("j").unwrap(), true, 8),
        Ok(JobStatus::Cancelled)
    );
    assert_eq!(host.lease_count(), 0);
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert!(host
        .audit()
        .iter()
        .any(|e| e.action == AuditAction::ForceCancelled));
}

fn ledger_host() -> MockComputeHost {
    let mut host = setup();
    host.set_lifecycle_authority(LifecycleAuthority::CoordinatorLedger)
        .unwrap();
    host.bind_ledger_runtime(LedgerRuntimeBinding {
        model: model("llm"),
        runtime: RuntimeId::new("pc-cuda-ctx-1").unwrap(),
        resource: LedgerResourceId(500),
    })
    .unwrap();
    host.bind_ledger_runtime(LedgerRuntimeBinding {
        model: model("asr"),
        runtime: RuntimeId::new("pc-asr-ctx-2").unwrap(),
        resource: LedgerResourceId(501),
    })
    .unwrap();
    host.bind_ledger_runtime(LedgerRuntimeBinding {
        model: model("external"),
        runtime: RuntimeId::new("borrowed-jan-1").unwrap(),
        resource: LedgerResourceId(502),
    })
    .unwrap();
    for (device, app_name, owner) in [
        ("pi", "advisor", 101),
        ("pi-2", "text", 102),
        ("pi", "agent", 103),
        ("laptop", "text", 104),
    ] {
        host.bind_ledger_owner(
            client(device),
            app(app_name),
            AppSessionId::new("launch-1").unwrap(),
            LedgerOwnerId(owner),
        )
        .unwrap();
    }
    host
}

#[test]
fn phase2_two_pi_apps_share_pc_gpu_ledger_is_only_unload_authority() {
    let mut host = ledger_host();
    let first = evidence("pi", "advisor");
    let second = evidence("pi-2", "text");
    host.open_lease(&first, lease("a"), model("llm"), 0)
        .unwrap();
    host.open_lease(&second, lease("b"), model("llm"), 1)
        .unwrap();
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 0)));
    assert_eq!(host.model_lease_count(&model("llm")), 2);
    assert_eq!(host.total_owned_vram_mib(), 3000);
    let acquired: Vec<_> = host.pending_ledger_events().iter().cloned().collect();
    assert_eq!(acquired.len(), 2);
    assert!(acquired
        .iter()
        .all(|event| event.action == LedgerLeaseAction::Acquired
            && event.model == model("llm")
            && event.resource == LedgerResourceId(500)));
    assert_ne!(acquired[0].owner, acquired[1].owner);
    // At-least-once delivery must be deduplicated by host+sequence at the ledger adapter.
    assert!(host.acknowledge_ledger_event(acquired[0].sequence, 10));
    assert!(!host.acknowledge_ledger_event(acquired[0].sequence, 10));
    assert!(host.acknowledge_ledger_event(acquired[1].sequence, 10));
    host.close_app(&first, 2).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert!(!host.awaiting_ledger_unload(&model("llm")));
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 0)));
    host.submit(&second, request("safe-client-b", "b"), 3)
        .unwrap();
    host.close_app(&second, 4).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 0);
    assert!(host.awaiting_ledger_unload(&model("llm")));
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 0)));
    let mut fresh_launch = second.clone();
    fresh_launch.app_session = AppSessionId::new("launch-new").unwrap();
    host.bind_ledger_owner(
        client("pi-2"),
        app("text"),
        fresh_launch.app_session.clone(),
        LedgerOwnerId(105),
    )
    .unwrap();
    assert_eq!(
        host.open_lease(&fresh_launch, lease("new"), model("llm"), 5),
        Err(Error::LedgerCleanupPending)
    );
    let gen = host.model_generation(&model("llm")).unwrap();
    let releases: Vec<_> = host
        .pending_ledger_events()
        .iter()
        .filter(|e| e.action == LedgerLeaseAction::Released && e.model == model("llm"))
        .cloned()
        .collect();
    assert_eq!(releases.len(), 2);
    assert_eq!(
        host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), gen, true, 5),
        Err(Error::LedgerAckRejected)
    ); // Ledger has not processed release yet.
    assert!(!host.acknowledge_ledger_event(releases[1].sequence, 10)); // Out of order.
    assert!(host.acknowledge_ledger_event(releases[0].sequence, 10));
    assert!(host.acknowledge_ledger_event(releases[1].sequence, 10));
    assert_eq!(
        host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(999), gen, true, 6),
        Err(Error::LedgerAckRejected)
    );
    assert_eq!(
        host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), gen + 1, true, 6),
        Err(Error::LedgerAckRejected)
    );
    host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), gen, false, 6)
        .unwrap();
    assert_eq!(host.total_owned_vram_mib(), 3000);
    host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), gen, true, 7)
        .unwrap();
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 1)));
    assert_eq!(
        host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), gen, true, 8),
        Err(Error::LedgerAckRejected)
    );
    assert_eq!(host.status(), HostStatus::Accepting);
    assert_eq!(host.pending_ledger_events().len(), 0);
}

#[test]
fn phase2_ledger_ttl_disconnect_and_borrowed_runtime_no_unload() {
    let mut host = ledger_host();
    host.open_lease(&evidence("pi", "advisor"), lease("a"), model("llm"), 0)
        .unwrap();
    host.open_lease(&evidence("pi-2", "text"), lease("b"), model("external"), 0)
        .unwrap();
    host.heartbeat(&evidence("pi-2", "text"), &lease("b"), 20)
        .unwrap();
    // Pi-1 is disconnected and no longer sends heartbeat.
    host.tick(30);
    assert_eq!(host.model_lease_count(&model("llm")), 0);
    assert!(host.awaiting_ledger_unload(&model("llm")));
    assert_eq!(host.model_lease_count(&model("external")), 1);
    assert_eq!(host.model_load_counts(&model("external")), Some((0, 0)));
    let event_ids: Vec<_> = host
        .pending_ledger_events()
        .iter()
        .map(|event| event.sequence)
        .collect();
    assert_eq!(event_ids.len(), 3);
    for event in event_ids {
        assert!(host.acknowledge_ledger_event(event, 10));
    }
    host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), 1, true, 31)
        .unwrap();
    assert_eq!(host.total_owned_vram_mib(), 0);
    host.close_app(&evidence("pi-2", "text"), 40).unwrap();
    assert!(!host.awaiting_ledger_unload(&model("external")));
    assert_eq!(host.model_load_counts(&model("external")), Some((0, 0)));
}

#[test]
fn phase2_app_launch_and_task_scope_prevent_cross_session_cancel() {
    let mut host = setup();
    let first = evidence("pi", "advisor");
    let mut second = evidence("pi", "advisor");
    second.app_session = AppSessionId::new("launch-2").unwrap();
    second.task = TaskId::new("task-2").unwrap();
    host.open_lease(&first, lease("a"), model("llm"), 0)
        .unwrap();
    host.open_lease(&second, lease("b"), model("llm"), 0)
        .unwrap();
    host.submit(&first, request("request-a", "a"), 1).unwrap();
    host.submit(&second, request("request-b", "b"), 1).unwrap();
    assert_eq!(
        host.cancel_request(&second, &RequestId::new("request-a").unwrap(), 2),
        Err(Error::NotOwner)
    );
    assert_eq!(
        host.heartbeat(&second, &lease("a"), 2),
        Err(Error::NotOwner)
    );
    assert_eq!(
        host.close_lease(&second, &lease("a"), 2),
        Err(Error::NotOwner)
    );
    host.close_task(&first, 3).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(
        host.job_status(&RequestId::new("request-b").unwrap()),
        Some(JobStatus::Queued)
    );
    host.close_app(&first, 4).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    host.close_app(&second, 5).unwrap();
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 1)));
}

#[test]
fn phase2_retry_same_lease_does_not_double_count_reuse_denied() {
    let mut host = ledger_host();
    let peer = evidence("pi", "advisor");
    host.open_lease(&peer, lease("stable"), model("llm"), 0)
        .unwrap();
    host.open_lease(&peer, lease("stable"), model("llm"), 1)
        .unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(host.pending_ledger_events().len(), 1);
    assert_eq!(
        host.open_lease(&evidence("pi-2", "text"), lease("stable"), model("llm"), 2),
        Err(Error::NotOwner)
    );
    host.close_lease(&peer, &lease("stable"), 3).unwrap();
    host.close_lease(&peer, &lease("stable"), 4).unwrap();
    assert_eq!(
        host.open_lease(&peer, lease("stable"), model("llm"), 4),
        Err(Error::LeaseExpired)
    );
    assert_eq!(
        host.close_lease(&evidence("pi-2", "text"), &lease("stable"), 5),
        Err(Error::NotOwner)
    );
    assert_eq!(host.pending_ledger_events().len(), 2);
}

#[test]
fn phase2_missing_or_conflicting_coordinator_mapping_fails_closed() {
    let mut host = setup();
    host.set_lifecycle_authority(LifecycleAuthority::CoordinatorLedger)
        .unwrap();
    let peer = evidence("pi", "advisor");
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::LedgerBindingRequired)
    );
    host.bind_ledger_runtime(LedgerRuntimeBinding {
        model: model("llm"),
        runtime: RuntimeId::new("runtime-1").unwrap(),
        resource: LedgerResourceId(500),
    })
    .unwrap();
    assert_eq!(
        host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::LedgerBindingRequired)
    );
    assert_eq!(
        host.bind_ledger_runtime(LedgerRuntimeBinding {
            model: model("asr"),
            runtime: RuntimeId::new("runtime-2").unwrap(),
            resource: LedgerResourceId(500),
        }),
        Err(Error::IdentifierConflict)
    );
    host.bind_ledger_owner(
        client("pi"),
        app("advisor"),
        AppSessionId::new("launch-1").unwrap(),
        LedgerOwnerId(2),
    )
    .unwrap();
    assert_eq!(
        host.bind_ledger_owner(
            client("pi-2"),
            app("text"),
            AppSessionId::new("launch-1").unwrap(),
            LedgerOwnerId(2)
        ),
        Err(Error::IdentifierConflict)
    );
    host.open_lease(&peer, lease("a"), model("llm"), 0).unwrap();
    assert_eq!(
        host.set_lifecycle_authority(LifecycleAuthority::MockLocal),
        Err(Error::InvalidConfiguration)
    );
}

#[test]
fn phase2_restricted_grant_revokes_existing_app_work_only() {
    let mut host = setup();
    let a = evidence("pi", "advisor");
    let b = evidence("pi-2", "text");
    host.open_lease(&a, lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&b, lease("b"), model("llm"), 0).unwrap();
    host.submit(&a, request("j-a", "a"), 1).unwrap();
    host.submit(&b, request("j-b", "b"), 1).unwrap();
    host.set_grant(client("pi"), app("advisor"), grant(&["asr"]), 2)
        .unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(
        host.job_status(&RequestId::new("j-a").unwrap()),
        Some(JobStatus::Cancelled)
    );
    assert_eq!(
        host.job_status(&RequestId::new("j-b").unwrap()),
        Some(JobStatus::Queued)
    );
    assert_eq!(host.total_owned_vram_mib(), 3000);
}

#[test]
fn phase2_host_drain_not_reported_complete_before_ledger_ack() {
    let mut host = ledger_host();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.begin_host_drain(1);
    assert_eq!(host.status(), HostStatus::Draining);
    assert!(host.awaiting_ledger_unload(&model("llm")));
    assert_eq!(host.total_owned_vram_mib(), 3000);
    let pending: Vec<_> = host
        .pending_ledger_events()
        .iter()
        .map(|event| event.sequence)
        .collect();
    assert_eq!(pending.len(), 2);
    for event in pending {
        assert!(host.acknowledge_ledger_event(event, 2));
    }
    assert_eq!(host.status(), HostStatus::Draining);
    host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), 1, true, 3)
        .unwrap();
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(host.status(), HostStatus::Drained);
}

#[test]
fn phase2_one_pi_disconnect_does_not_block_another_pi_transport() {
    let mut transport = MockTransport::new(setup());
    let pi = evidence("pi", "advisor");
    let second = evidence("pi-2", "text");
    transport
        .exchange(
            &pi,
            WireCommand::OpenLease {
                lease: lease("pi-a"),
                model: model("llm"),
            },
            0,
        )
        .unwrap();
    transport
        .exchange(
            &second,
            WireCommand::OpenLease {
                lease: lease("pi-b"),
                model: model("llm"),
            },
            0,
        )
        .unwrap();
    transport.set_client_connected(client("pi"), false);
    assert_eq!(
        transport.exchange(
            &pi,
            WireCommand::Heartbeat {
                lease: lease("pi-a"),
            },
            10
        ),
        Err(Error::TransportUnavailable)
    );
    assert_eq!(
        transport.exchange(
            &second,
            WireCommand::Heartbeat {
                lease: lease("pi-b"),
            },
            20
        ),
        Ok(WireResponse::Accepted)
    );
    transport.host.tick(30);
    assert_eq!(transport.host.model_lease_count(&model("llm")), 1);
    assert_eq!(transport.host.status(), HostStatus::Accepting);
    assert_eq!(
        transport.exchange(
            &second,
            WireCommand::Submit {
                request: request("pi-b-job", "pi-b"),
            },
            31
        ),
        Ok(WireResponse::Job(JobStatus::Queued))
    );
    assert_eq!(transport.host.total_owned_vram_mib(), 3000);
}

// Test-only fixture models ResourceLedger's reference semantics. This is NOT
// imported core PR #12, not a second production ledger, and not GPU evidence.
#[derive(Default)]
struct LedgerFixture {
    references: std::collections::HashMap<
        LedgerResourceId,
        std::collections::HashSet<(LedgerOwnerId, LeaseId)>,
    >,
    seen: std::collections::HashSet<(HostId, u64)>,
    cleanup_requests: std::collections::HashMap<LedgerResourceId, usize>,
}
impl LedgerFixture {
    fn apply(&mut self, event: &LedgerLeaseEvent) {
        if !self.seen.insert((event.host.clone(), event.sequence)) {
            return;
        }
        let holders = self.references.entry(event.resource).or_default();
        let holder = (event.owner, event.lease.clone());
        match event.action {
            LedgerLeaseAction::Acquired => {
                assert!(holders.insert(holder), "duplicate lease acquired");
            }
            LedgerLeaseAction::Released => {
                assert!(holders.remove(&holder), "release must follow acquire");
                if holders.is_empty() {
                    *self.cleanup_requests.entry(event.resource).or_default() += 1;
                }
            }
        }
    }
}

#[test]
fn phase2_ledger_fixture_replayed_events_result_in_one_last_client_cleanup_request() {
    let mut host = ledger_host();
    let pi_a = evidence("pi", "advisor");
    let pi_b = evidence("pi-2", "text");
    host.open_lease(&pi_a, lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&pi_b, lease("b"), model("llm"), 0).unwrap();
    let mut ledger = LedgerFixture::default();
    let original: Vec<_> = host.pending_ledger_events().iter().cloned().collect();
    // At-least-once transport may deliver the same event again before ACK.
    for event in &original {
        ledger.apply(event);
        ledger.apply(event);
    }
    for event in original {
        assert!(host.acknowledge_ledger_event(event.sequence, 1));
    }
    assert_eq!(ledger.references[&LedgerResourceId(500)].len(), 2);
    assert_eq!(ledger.cleanup_requests.get(&LedgerResourceId(500)), None);
    host.close_app(&pi_a, 2).unwrap();
    let first_release = host.pending_ledger_events().front().unwrap().clone();
    ledger.apply(&first_release);
    assert!(host.acknowledge_ledger_event(first_release.sequence, 2));
    assert_eq!(ledger.references[&LedgerResourceId(500)].len(), 1);
    assert_eq!(ledger.cleanup_requests.get(&LedgerResourceId(500)), None);
    host.close_app(&pi_b, 3).unwrap();
    let last_release = host.pending_ledger_events().front().unwrap().clone();
    ledger.apply(&last_release);
    ledger.apply(&last_release);
    assert!(host.acknowledge_ledger_event(last_release.sequence, 3));
    assert_eq!(
        ledger.cleanup_requests.get(&LedgerResourceId(500)),
        Some(&1)
    );
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 0)));
    host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), 1, true, 4)
        .unwrap();
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 1)));
}

#[test]
fn phase2_resource_id_must_rotate_after_owned_unload_before_relaunch() {
    let mut host = ledger_host();
    let old = evidence("pi", "advisor");
    host.open_lease(&old, lease("first"), model("llm"), 0)
        .unwrap();
    host.close_app(&old, 1).unwrap();
    let pending: Vec<_> = host
        .pending_ledger_events()
        .iter()
        .map(|e| e.sequence)
        .collect();
    for seq in pending {
        assert!(host.acknowledge_ledger_event(seq, 2));
    }
    host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), 1, true, 3)
        .unwrap();
    let mut fresh = old.clone();
    fresh.app_session = AppSessionId::new("launch-2").unwrap();
    fresh.task = TaskId::new("task-2").unwrap();
    host.bind_ledger_owner(
        client("pi"),
        app("advisor"),
        fresh.app_session.clone(),
        LedgerOwnerId(200),
    )
    .unwrap();
    assert_eq!(
        host.open_lease(&old, lease("stale"), model("llm"), 4),
        Err(Error::LeaseExpired)
    );
    assert_eq!(
        host.open_lease(&fresh, lease("second"), model("llm"), 4),
        Err(Error::LedgerBindingRequired)
    );
    assert_eq!(
        host.rotate_ledger_runtime(LedgerRuntimeBinding {
            model: model("llm"),
            runtime: RuntimeId::new("pc-cuda-ctx-2").unwrap(),
            resource: LedgerResourceId(500),
        }),
        Err(Error::IdentifierConflict)
    ); // Recycled core ResourceId.
    host.rotate_ledger_runtime(LedgerRuntimeBinding {
        model: model("llm"),
        runtime: RuntimeId::new("pc-cuda-ctx-2").unwrap(),
        resource: LedgerResourceId(600),
    })
    .unwrap();
    host.open_lease(&fresh, lease("second"), model("llm"), 5)
        .unwrap();
    assert_eq!(host.model_load_counts(&model("llm")), Some((2, 1)));
    assert_eq!(host.model_generation(&model("llm")), Some(2));
    assert_eq!(
        host.acknowledge_ledger_unload(&model("llm"), LedgerResourceId(500), 1, true, 6),
        Err(Error::LedgerAckRejected)
    );
    let latest = host.pending_ledger_events().back().unwrap();
    assert_eq!(latest.resource, LedgerResourceId(600));
    assert_eq!(latest.owner, LedgerOwnerId(200));
}

#[test]
fn phase2_borrowed_provider_reference_uses_new_ledger_id_not_new_process() {
    let mut host = ledger_host();
    let first = evidence("pi-2", "text");
    host.open_lease(&first, lease("borrow-1"), model("external"), 0)
        .unwrap();
    host.close_app(&first, 1).unwrap();
    let pending: Vec<_> = host
        .pending_ledger_events()
        .iter()
        .map(|e| e.sequence)
        .collect();
    for seq in pending {
        assert!(host.acknowledge_ledger_event(seq, 2));
    }
    assert_eq!(host.model_load_counts(&model("external")), Some((0, 0)));
    let mut second = first.clone();
    second.app_session = AppSessionId::new("fresh-external").unwrap();
    host.bind_ledger_owner(
        client("pi-2"),
        app("text"),
        second.app_session.clone(),
        LedgerOwnerId(201),
    )
    .unwrap();
    assert_eq!(
        host.open_lease(&second, lease("borrow-2"), model("external"), 3),
        Err(Error::LedgerBindingRequired)
    );
    host.rotate_ledger_runtime(LedgerRuntimeBinding {
        model: model("external"),
        runtime: RuntimeId::new("borrowed-jan-1").unwrap(),
        resource: LedgerResourceId(601),
    })
    .unwrap();
    host.open_lease(&second, lease("borrow-2"), model("external"), 4)
        .unwrap();
    assert_eq!(host.model_load_counts(&model("external")), Some((0, 0)));
    assert_eq!(
        host.pending_ledger_events().back().unwrap().resource,
        LedgerResourceId(601)
    );
}
