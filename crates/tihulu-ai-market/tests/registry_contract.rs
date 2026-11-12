//! Phase 2 integration-contract fixtures: synthetic only; no Registry, GPU,
//! Jan, Pi or Hailo adapter runs here.
use tihulu_ai_market::registry_contract::{
    match_registry_model, ApplicationId, ApplicationTask, ArtifactEvidence, CatalogIdentityBinding,
    HostId, RegistryMatchRequest, RegistryModelId, RegistryModelSnapshot, RegistryOwner,
    RegistryPresence, RegistryRuntimeEvidence, RuntimeTarget, TrustedHardwareProbe,
};
use tihulu_ai_market::{
    validate_advisor_proposal, AdvisorPolicy, AdvisorProposal, Citation, HardwareTarget as H,
    License, MatchVerdict, ModelFormat as F, ModelManifest, ModelStatus, ProposalError,
    Runtime as R, TaskKind as T, TaskRequest,
};

fn fixture(name: &str) -> ModelManifest {
    let raw = match name {
        "jan" => include_str!("fixtures/jan-owned-gguf.json"),
        "asr" => include_str!("fixtures/asr-whisper.json"),
        "translation" => include_str!("fixtures/translation-tr-de.json"),
        "cuda" => include_str!("fixtures/cuda-chat.json"),
        "pi" => include_str!("fixtures/pi-cpu-chat.json"),
        "apple" => include_str!("fixtures/apple-silicon-mlx.json"),
        "hailo" => include_str!("fixtures/hailo-vision.json"),
        "paired" => include_str!("fixtures/paired-gpu-endpoint.json"),
        "license" => include_str!("fixtures/unknown-license.json"),
        "wrong-format" => include_str!("fixtures/wrong-format.json"),
        "online" => include_str!("fixtures/untrusted-online.json"),
        _ => panic!("unknown test fixture"),
    };
    ModelManifest::from_json(raw).unwrap()
}

fn task(model: &ModelManifest) -> TaskRequest {
    TaskRequest {
        task: model.tasks[0],
        input: model.input,
        output: model.output,
        languages: vec![model.languages[0].clone()],
        translation: model.translation_pairs.first().cloned(),
        quantization: None,
        requires_tool_calling: false,
        offline_only: true,
        license_allowlist: vec!["Apache-2.0".into()],
    }
}

fn target(runtime: R, hardware: H, host: &str) -> RuntimeTarget {
    RuntimeTarget {
        host_id: HostId::new(host).unwrap(),
        runtime,
        hardware,
    }
}

fn setup(
    model: &ModelManifest,
    owner: RegistryOwner,
    selected: RuntimeTarget,
) -> (
    RegistryModelSnapshot,
    RegistryMatchRequest,
    TrustedHardwareProbe,
) {
    let registry_id = RegistryModelId::new(format!("ref-v1-synthetic-{}", model.id)).unwrap();
    let presence = match model.format {
        F::Endpoint => RegistryPresence::EndpointListed,
        other => RegistryPresence::FileObserved { format_hint: other },
    };
    let registry = RegistryModelSnapshot {
        id: registry_id.clone(),
        catalog_binding: Some(CatalogIdentityBinding {
            catalog_model_id: model.id.clone(),
            evidence_id: "synthetic-provenance-binding".into(),
        }),
        owner,
        presence,
        artifact: ArtifactEvidence::Validated {
            evidence_id: "synthetic-artifact-test".into(),
        },
        runtime: RegistryRuntimeEvidence::Verified {
            runtime: selected.runtime,
            hardware: selected.hardware,
            host_id: selected.host_id.clone(),
            evidence_id: "synthetic-fixture-probe".into(),
        },
    };
    let request = RegistryMatchRequest {
        catalog_model_id: model.id.clone(),
        registry_model_id: registry_id,
        application_task: ApplicationTask {
            application_id: ApplicationId::new("tihulu-text-center").unwrap(),
            request: task(model),
            adapter_verified: true,
        },
        target: selected.clone(),
    };
    let probe = TrustedHardwareProbe {
        target: selected,
        hardware_verified: true,
        runtime_ready: true,
        available_ram_mib: Some(16_384),
        available_vram_mib: Some(16_384),
        available_npu_mib: Some(8_192),
        paired_host_authorized: true,
    };
    (registry, request, probe)
}

#[test]
fn rejects_empty_typed_identifiers() {
    assert!(RegistryModelId::new(" ").is_err());
    assert!(ApplicationId::new("").is_err());
    assert!(HostId::new("\t").is_err());
}

#[test]
fn jan_owned_gguf_is_external_inventory_not_implicitly_runtime_verified() {
    let model = fixture("jan");
    let (mut record, request, probe) = setup(
        &model,
        RegistryOwner::JanDesktop,
        target(R::LlamaCpp, H::Cpu, "local"),
    );
    record.runtime = RegistryRuntimeEvidence::Unchecked;
    let untested = match_registry_model(&model, &record, &request, &probe);
    assert_eq!(untested.inventory_status, ModelStatus::External);
    assert_eq!(untested.compatibility.verdict, MatchVerdict::Candidate);
    assert_eq!(untested.compatibility.status, ModelStatus::Unverified);
    assert!(!untested.can_propose_assignment);

    record.runtime = RegistryRuntimeEvidence::Verified {
        runtime: R::LlamaCpp,
        hardware: H::Cpu,
        host_id: HostId::new("local").unwrap(),
        evidence_id: "synthetic-fixture-probe".into(),
    };
    record.artifact = ArtifactEvidence::Unchecked;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Candidate,
    );

    record.artifact = ArtifactEvidence::Validated {
        evidence_id: "synthetic-artifact-test".into(),
    };
    let ready = match_registry_model(&model, &record, &request, &probe);
    assert_eq!(ready.inventory_status, ModelStatus::External);
    assert_eq!(ready.compatibility.verdict, MatchVerdict::Recommended);
    assert!(ready.can_propose_assignment);
    assert!(ready.read_only);
}

#[test]
fn catalog_reference_binding_and_ownership_must_be_consistent() {
    let model = fixture("jan");
    let (mut record, mut request, probe) = setup(
        &model,
        RegistryOwner::JanDesktop,
        target(R::LlamaCpp, H::Cpu, "local"),
    );
    record.catalog_binding = None;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
    record.catalog_binding = Some(CatalogIdentityBinding {
        catalog_model_id: "wrong-catalog".into(),
        evidence_id: "synthetic-provenance-binding".into(),
    });
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
    record.catalog_binding = Some(CatalogIdentityBinding {
        catalog_model_id: model.id.clone(),
        evidence_id: "".into(),
    });
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
    record.catalog_binding = Some(CatalogIdentityBinding {
        catalog_model_id: model.id.clone(),
        evidence_id: "synthetic-provenance-binding".into(),
    });
    request.registry_model_id = RegistryModelId::new("ref-v1-other").unwrap();
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
    request.registry_model_id = record.id.clone();
    record.owner = RegistryOwner::AiTools;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
}

#[test]
fn wrong_format_and_missing_or_unlisted_reference_are_never_recommended() {
    let model = fixture("wrong-format");
    let (mut record, request, probe) = setup(
        &model,
        RegistryOwner::JanDesktop,
        target(R::LlamaCpp, H::Cpu, "local"),
    );
    record.presence = RegistryPresence::FileObserved {
        format_hint: F::Onnx,
    };
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .status,
        ModelStatus::Incompatible,
    );
    record.presence = RegistryPresence::Missing;
    let missing = match_registry_model(&model, &record, &request, &probe);
    assert_eq!(missing.compatibility.status, ModelStatus::Unknown);
    assert_eq!(missing.inventory_status, ModelStatus::External);
    assert_eq!(missing.observed_presence, RegistryPresence::Missing);
    assert!(!missing.can_propose_assignment);
    record.presence = RegistryPresence::EndpointDeclared;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .status,
        ModelStatus::Unknown,
    );
}

#[test]
fn asr_and_directional_translation_are_distinct_app_tasks() {
    let asr = fixture("asr");
    let (record, mut request, probe) = setup(
        &asr,
        RegistryOwner::AiTools,
        target(R::WhisperCpp, H::Cpu, "local"),
    );
    request.application_task.application_id = ApplicationId::new("media-center").unwrap();
    assert_eq!(
        match_registry_model(&asr, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended,
    );
    request.application_task.request.task = T::Translation;
    assert_eq!(
        match_registry_model(&asr, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
    let translator = fixture("translation");
    let (record, mut request, probe) = setup(
        &translator,
        RegistryOwner::AiTools,
        target(R::Ctranslate2, H::Cpu, "local"),
    );
    request.application_task.request.languages = vec!["tr".into(), "de".into()];
    assert_eq!(
        match_registry_model(&translator, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended,
    );
    request.application_task.request.translation = Some(tihulu_ai_market::LanguagePair {
        from: "de".into(),
        to: "tr".into(),
    });
    assert_eq!(
        match_registry_model(&translator, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
}

#[test]
fn cuda_and_pi_cpu_differ_in_memory_and_runtime_probe_identity() {
    let cuda = fixture("cuda");
    let (mut record, request, mut probe) = setup(
        &cuda,
        RegistryOwner::AiTools,
        target(R::LlamaCpp, H::Cuda, "pc-gpu"),
    );
    record.runtime = RegistryRuntimeEvidence::Unchecked;
    let installed_but_unverified = match_registry_model(&cuda, &record, &request, &probe);
    assert_eq!(
        installed_but_unverified.inventory_status,
        ModelStatus::Installed
    );
    assert_eq!(
        installed_but_unverified.compatibility.verdict,
        MatchVerdict::Candidate
    );
    assert!(!installed_but_unverified.can_propose_assignment);
    record.runtime = RegistryRuntimeEvidence::Verified {
        runtime: R::LlamaCpp,
        hardware: H::Cuda,
        host_id: HostId::new("pc-gpu").unwrap(),
        evidence_id: "synthetic-fixture-probe".into(),
    };
    assert_eq!(
        match_registry_model(&cuda, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended,
    );
    probe.available_vram_mib = Some(2048);
    assert_eq!(
        match_registry_model(&cuda, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
    probe.available_vram_mib = Some(16384);
    probe.target.host_id = HostId::new("different-host").unwrap();
    assert_eq!(
        match_registry_model(&cuda, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
    probe.target.host_id = HostId::new("pc-gpu").unwrap();
    record.runtime = RegistryRuntimeEvidence::Incompatible {
        reason: "runtime self-test failed".into(),
    };
    assert_eq!(
        match_registry_model(&cuda, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );

    let pi = fixture("pi");
    let (record, request, probe) = setup(
        &pi,
        RegistryOwner::AiTools,
        target(R::LlamaCpp, H::RaspberryPiCpu, "pi-5"),
    );
    assert_eq!(
        match_registry_model(&pi, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended,
    );
    let mut not_pi = request.clone();
    not_pi.target.hardware = H::Cuda;
    assert_eq!(
        match_registry_model(&pi, &record, &not_pi, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
}

#[test]
fn apple_silicon_uses_unified_ram_not_cuda_vram_and_requires_mlx() {
    let model = fixture("apple");
    let (registry, request, mut probe) = setup(
        &model,
        RegistryOwner::AiTools,
        target(R::Mlx, H::AppleSilicon, "mac-m"),
    );
    probe.available_vram_mib = None;
    assert_eq!(
        match_registry_model(&model, &registry, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended
    );
    probe.available_ram_mib = Some(1024);
    assert_eq!(
        match_registry_model(&model, &registry, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible
    );
    probe.available_ram_mib = Some(8192);
    let mut cuda_target = request.clone();
    cuda_target.target.hardware = H::Cuda;
    let mut cuda_probe = probe.clone();
    cuda_probe.target = cuda_target.target.clone();
    assert_eq!(
        match_registry_model(&model, &registry, &cuda_target, &cuda_probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn hailo_hef_never_equals_cuda_and_npu_budget_is_independent() {
    let model = fixture("hailo");
    let (record, request, mut probe) = setup(
        &model,
        RegistryOwner::AiTools,
        target(R::HailoRt, H::Hailo, "pi-hailo"),
    );
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended,
    );
    probe.available_npu_mib = Some(2048);
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
    probe.available_npu_mib = Some(8192);
    let mut cuda_request = request.clone();
    cuda_request.target.hardware = H::Cuda;
    let mut cuda_probe = probe.clone();
    cuda_probe.target = cuda_request.target.clone();
    assert_eq!(
        match_registry_model(&model, &record, &cuda_request, &cuda_probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
}

#[test]
fn paired_gpu_host_is_not_a_local_cuda_lease_and_requires_authorization() {
    let model = fixture("paired");
    let (mut record, request, mut probe) = setup(
        &model,
        RegistryOwner::ExternalService("Paired Host".into()),
        target(R::OpenAiApi, H::PairedGpuHost, "gpu-host-1"),
    );
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Recommended,
    );
    probe.paired_host_authorized = false;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Incompatible,
    );
    probe.paired_host_authorized = true;
    record.presence = RegistryPresence::EndpointDeclared;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
}

#[test]
fn unknown_license_never_becomes_verified_runnable() {
    let model = fixture("license");
    let (record, request, probe) = setup(
        &model,
        RegistryOwner::AiTools,
        target(R::LlamaCpp, H::Cpu, "local"),
    );
    assert_eq!(model.license, License::Unknown);
    let decision = match_registry_model(&model, &record, &request, &probe);
    assert_eq!(decision.inventory_status, ModelStatus::Installed);
    assert_eq!(decision.compatibility.verdict, MatchVerdict::Unknown);
    assert!(!decision.can_propose_assignment);
}

#[test]
fn untrusted_online_recommendations_cannot_create_registry_probes_or_download() {
    let model = fixture("online");
    let (record, request, probe) = setup(
        &model,
        RegistryOwner::AiTools,
        target(R::LlamaCpp, H::Cpu, "local"),
    );
    let result = match_registry_model(&model, &record, &request, &probe);
    assert_eq!(result.compatibility.verdict, MatchVerdict::Incompatible);
    assert!(!result.can_propose_assignment);
    assert!(result.read_only);

    let suggestion = AdvisorProposal {
        model_id: model.id.clone(),
        task: T::Chat,
        explanation: "Unverified model-card performance claim".into(),
        confidence_pct: 100,
        citations: vec![Citation {
            title: "Unverified vendor page".into(),
            url: "https://example.test/model".into(),
            supported_claim: "Claims compatibility with all accelerators".into(),
        }],
        internet_used: true,
    };
    let req = task(&model);
    let ctx = tihulu_ai_market::CompatibilityContext {
        runtime: R::LlamaCpp,
        hardware: H::Cpu,
        available_ram_mib: Some(16_384),
        available_vram_mib: None,
        available_npu_mib: None,
        hardware_verified: false,
        runtime_ready: false,
        consumer_adapter_verified: false,
        paired_host_authorized: false,
        trusted_probes: Vec::new(),
    };
    assert_eq!(
        validate_advisor_proposal(
            &suggestion,
            &AdvisorPolicy::default(),
            &[model.clone()],
            &req,
            &ctx
        ),
        Err(ProposalError::InternetNotAuthorized),
    );
    let validated = validate_advisor_proposal(
        &suggestion,
        &AdvisorPolicy {
            internet_opt_in: true,
        },
        &[model],
        &req,
        &ctx,
    )
    .unwrap();
    assert!(!validated.executable);
    assert_ne!(
        validated.deterministic_match.verdict,
        MatchVerdict::Recommended
    );
}

#[test]
fn application_adapter_and_host_readiness_are_independent_of_catalog_inventory() {
    let model = fixture("jan");
    let (record, mut request, mut probe) = setup(
        &model,
        RegistryOwner::JanDesktop,
        target(R::LlamaCpp, H::Cpu, "local"),
    );
    request.application_task.adapter_verified = false;
    let decision = match_registry_model(&model, &record, &request, &probe);
    assert_eq!(decision.inventory_status, ModelStatus::External);
    assert_eq!(decision.compatibility.verdict, MatchVerdict::Unknown);
    request.application_task.adapter_verified = true;
    probe.runtime_ready = false;
    assert_eq!(
        match_registry_model(&model, &record, &request, &probe)
            .compatibility
            .verdict,
        MatchVerdict::Unknown,
    );
}
