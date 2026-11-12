//! Contract regression tests; these are NOT real mTLS/GPU/hardware tests.
use std::collections::HashSet;
use tihulu_ai_transport::*;

fn host_id() -> HostId { HostId::new("paired-pc-01").unwrap() }
fn client(n: &str) -> ClientId { ClientId::new(n).unwrap() }
fn app(n: &str) -> AppId { AppId::new(n).unwrap() }
fn model(n: &str) -> ModelId { ModelId::new(n).unwrap() }
fn lease(n: &str) -> LeaseId { LeaseId::new(n).unwrap() }
fn request(n: &str, l: &str) -> RequestSpec {
    RequestSpec {
        id: RequestId::new(n).unwrap(), lease: lease(l),
        task: TaskKind::Chat, payload_bytes: 64,
    }
}
fn fingerprint(n: &str) -> String { n.repeat(64) }

fn evidence(client_id: &str, app_id: &str) -> PeerEvidence {
    PeerEvidence {
        host: host_id(), client: client(client_id), app: app(app_id),
        app_authentication: AppAuthentication::VerifiedScopedCredential,
        client_cert_sha256: fingerprint(if client_id == "pi" { "b" } else { "c" }),
        server_cert_sha256: fingerprint("a"),
        security: ChannelSecurity::VerifiedMutualTls,
        route: NetworkRoute::DirectEthernet,
    }
}
fn grant(models: &[&str]) -> AppGrant {
    AppGrant {
        allowed_models: models.iter().map(|m| model(m)).collect::<HashSet<_>>(),
        allowed_tasks: [TaskKind::Chat, TaskKind::Transcription].into_iter().collect(),
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
            id: model(name), location: ModelLocation::HostStored,
            ownership: RuntimeOwnership::HostOwned, estimated_vram_mib: 3000,
        }).unwrap();
    }
    host.register_model(ModelRegistration {
        id: model("external"),
        location: ModelLocation::HostStored,
        ownership: RuntimeOwnership::BorrowedExternal,
        estimated_vram_mib: 7777,
    }).unwrap();
    for (device, hash) in [("pi", "b"), ("laptop", "c")] {
        host.pair_client(PairingApproval {
            client: client(device), client_cert_sha256: fingerprint(hash),
            user_approved: true,
        }, 0).unwrap();
        for app_id in ["advisor", "agent", "text"] {
            host.set_grant(client(device), app(app_id),
                grant(&["llm", "asr", "external"]), 0).unwrap();
        }
    }
    host
}

#[test]
fn l03_two_apps_one_model_close_one_keeps_the_other_and_host() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&evidence("laptop", "text"), lease("b"), model("llm"), 0).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 2);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 0)));
    assert_eq!(host.total_owned_vram_mib(), 3000);
    host.close_app(&evidence("pi", "advisor"), 1).unwrap();
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(host.status(), HostStatus::Accepting);
    host.submit(&evidence("laptop", "text"), request("still-running", "b"), 2).unwrap();
    assert_eq!(host.start_next_job(2), Some(RequestId::new("still-running").unwrap()));
    assert_eq!(host.finish_job(&RequestId::new("still-running").unwrap(), true, 3),
        Ok(JobStatus::Succeeded));
}

#[test]
fn l04_last_lease_unloads_owned_vram_only_after_running_job_drained() {
    let mut host = setup();
    let peer = evidence("pi", "advisor");
    host.open_lease(&peer, lease("a"), model("llm"), 0).unwrap();
    host.submit(&peer, request("j", "a"), 1).unwrap();
    host.start_next_job(1);
    host.close_lease(&peer, &lease("a"), 2).unwrap();
    assert_eq!(host.job_status(&RequestId::new("j").unwrap()), Some(JobStatus::Cancelling));
    assert_eq!(host.total_owned_vram_mib(), 3000);
    assert_eq!(host.lease_status(&lease("a")), Some(LeaseStatus::Closing));
    host.tick(7);
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert_eq!(host.model_load_counts(&model("llm")), Some((1, 1)));
    assert_eq!(host.lease_count(), 0);
    assert_eq!(host.status(), HostStatus::Accepting);
    assert_eq!(host.finish_job(&RequestId::new("j").unwrap(), true, 8),
        Ok(JobStatus::Cancelled));
}

#[test]
fn l02_advisor_quit_does_not_cancel_agent_same_pi() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("advisor-lease"), model("llm"), 0).unwrap();
    host.open_lease(&evidence("pi", "agent"), lease("agent-lease"), model("llm"), 0).unwrap();
    host.submit(&evidence("pi", "agent"), request("agent-job", "agent-lease"), 1).unwrap();
    host.close_app(&evidence("pi", "advisor"), 2).unwrap();
    assert_eq!(host.lease_count(), 1);
    assert_eq!(host.job_status(&RequestId::new("agent-job").unwrap()), Some(JobStatus::Queued));
    assert_eq!(host.total_owned_vram_mib(), 3000);
    host.close_app(&evidence("pi", "agent"), 3).unwrap();
    assert_eq!(host.total_owned_vram_mib(), 0);
}

#[test]
fn l09_disconnect_or_crash_orphan_expires_and_other_client_survives() {
    let mut transport = MockTransport::new(setup());
    let peer = evidence("pi", "advisor");
    transport.exchange(&peer, WireCommand::OpenLease {
        lease: lease("a"), model: model("llm"),
    }, 0).unwrap();
    transport.host.open_lease(&evidence("laptop", "agent"), lease("b"), model("llm"), 0).unwrap();
    transport.set_connected(false);
    assert_eq!(transport.exchange(&peer, WireCommand::Heartbeat { lease: lease("a") }, 10),
        Err(Error::TransportUnavailable));
    // Other client heartbeat is an independent connection in production;
    // this mock advances that lease directly to simulate its separate channel.
    transport.host.heartbeat(&evidence("laptop", "agent"), &lease("b"), 20).unwrap();
    transport.host.tick(30);
    assert_eq!(transport.host.model_lease_count(&model("llm")), 1);
    assert_eq!(transport.host.total_owned_vram_mib(), 3000);
    transport.host.tick(51);
    assert_eq!(transport.host.total_owned_vram_mib(), 0);
}

#[test]
fn l12_ui_quit_does_not_terminate_separately_enabled_shared_host() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("ui"), model("llm"), 0).unwrap();
    host.open_lease(&evidence("laptop", "text"), lease("headless"), model("llm"), 0).unwrap();
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
    assert_eq!(host.open_lease(&evidence("laptop", "text"), lease("next"), model("asr"), 3),
        Err(Error::ShuttingDown));
    host.tick(7);
    assert_eq!(host.status(), HostStatus::Drained);
    assert_eq!(host.total_owned_vram_mib(), 0);
}

#[test]
fn l15_separate_models_only_unload_unreferenced_model() {
    let mut host = setup();
    host.open_lease(&evidence("pi", "advisor"), lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&evidence("laptop", "text"), lease("b"), model("asr"), 0).unwrap();
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
    host.open_lease(&peer, lease("a"), model("external"), 0).unwrap();
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
    assert_eq!(host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::UnverifiedChannel));
    peer.security = ChannelSecurity::VerifiedMutualTls;
    peer.app_authentication = AppAuthentication::Unverified;
    assert_eq!(host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::UnverifiedApplication));
    peer.app_authentication = AppAuthentication::VerifiedScopedCredential;
    peer.route = NetworkRoute::PublicInternet;
    assert_eq!(host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::UnverifiedChannel));
    peer.route = NetworkRoute::ApprovedVpn;
    peer.client_cert_sha256 = fingerprint("d");
    assert_eq!(host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::IdentityMismatch));
    peer.client_cert_sha256 = fingerprint("b");
    peer.server_cert_sha256 = fingerprint("d");
    assert_eq!(host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::IdentityMismatch));
    peer.server_cert_sha256 = fingerprint("a");
    peer.host = HostId::new("other-pc").unwrap();
    assert_eq!(host.open_lease(&peer, lease("a"), model("llm"), 0),
        Err(Error::IdentityMismatch));
}

#[test]
fn request_replay_is_idempotent_cross_client_cancel_is_denied() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    let other = evidence("laptop", "text");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&other, lease("b"), model("llm"), 0).unwrap();
    let original = request("job-1", "a");
    assert_eq!(host.submit(&pi, original.clone(), 1), Ok(JobStatus::Queued));
    assert_eq!(host.submit(&pi, original, 2), Ok(JobStatus::Queued));
    assert_eq!(host.submit(&other, request("job-1", "b"), 2),
        Err(Error::IdentifierConflict));
    assert_eq!(host.cancel_request(&other, &RequestId::new("job-1").unwrap(), 2),
        Err(Error::NotOwner));
    assert_eq!(host.close_lease(&other, &lease("a"), 2), Err(Error::NotOwner));
    assert_eq!(host.job_status(&RequestId::new("job-1").unwrap()), Some(JobStatus::Queued));
    assert_eq!(host.cancel_request(&pi, &RequestId::new("job-1").unwrap(), 3),
        Ok(JobStatus::Cancelled));
    assert_eq!(host.cancel_request(&pi, &RequestId::new("job-1").unwrap(), 4),
        Ok(JobStatus::Cancelled));
}

#[test]
fn quota_and_backpressure_and_task_grants() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    let oversized = RequestSpec { payload_bytes: 1025, ..request("big", "a") };
    assert_eq!(host.submit(&pi, oversized, 1), Err(Error::RequestTooLarge));
    let wrong_task = RequestSpec { task: TaskKind::Vision, ..request("vision", "a") };
    assert_eq!(host.submit(&pi, wrong_task, 1), Err(Error::PermissionDenied));
    host.submit(&pi, request("j1", "a"), 1).unwrap();
    host.submit(&pi, request("j2", "a"), 1).unwrap();
    assert_eq!(host.submit(&pi, request("j3", "a"), 1), Err(Error::QuotaExceeded));
    host.cancel_request(&pi, &RequestId::new("j1").unwrap(), 2).unwrap();
    assert_eq!(host.submit(&pi, request("j3", "a"), 3), Ok(JobStatus::Queued));
}

#[test]
fn heartbeat_replay_and_expiry_do_not_resurrect_stale_session() {
    let mut host = setup();
    let pi = evidence("pi", "advisor");
    host.open_lease(&pi, lease("a"), model("llm"), 0).unwrap();
    host.heartbeat(&pi, &lease("a"), 20).unwrap();
    host.tick(40);
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(host.heartbeat(&pi, &lease("a"), 50), Err(Error::UnknownLease));
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
            sha256: fingerprint("e"), transfer_approved: false,
            integrity_verified: true, runtime_compatible: true,
        },
        ownership: RuntimeOwnership::HostOwned, estimated_vram_mib: 100,
    }).unwrap();
    host.set_grant(client("pi"), app("advisor"),
        grant(&["llm", "pi-digest"]), 0).unwrap();
    assert_eq!(host.open_lease(&evidence("pi", "advisor"),
        lease("device-model"), model("pi-digest"), 1), Err(Error::StagingNotVerified));
    assert_eq!(ModelId::new("../../home/pi/model.gguf"), Err(Error::InvalidIdentifier));
    assert_eq!(ModelId::new("C:\\weights\\model"), Err(Error::InvalidIdentifier));
}

#[test]
fn revoking_one_app_does_not_cancel_other_app_lease() {
    let mut host = setup();
    let advisor = evidence("pi", "advisor");
    host.open_lease(&advisor, lease("a"), model("llm"), 0).unwrap();
    host.open_lease(&evidence("pi", "agent"), lease("b"), model("llm"), 0).unwrap();
    host.revoke_app(&client("pi"), &app("advisor"), 1);
    assert_eq!(host.model_lease_count(&model("llm")), 1);
    assert_eq!(host.open_lease(&advisor, lease("new"), model("llm"), 2),
        Err(Error::PermissionDenied));
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
    assert_eq!(host.finish_job(&RequestId::new("j").unwrap(), true, 8),
        Ok(JobStatus::Cancelled));
    assert_eq!(host.lease_count(), 0);
    assert_eq!(host.total_owned_vram_mib(), 0);
    assert!(host.audit().iter().any(|e| e.action == AuditAction::ForceCancelled));
}
