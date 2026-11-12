//! Phase 5 synthetic-only Registry -> Market inventory/compatibility tests.
//! All paths, runtime/host evidence, models and budgets are fabricated.
//! These tests do not open files, execute models or test actual GPU/NPU drivers.

use std::path::PathBuf;

use tihulu_ai_market::{
    inspect_registered_model, registry_format_hint, ApplicationId, ApplicationTask, HardwareTarget,
    HostId, InventoryInspection, License, MatchVerdict, Modality, ModelArchitecture, ModelFormat,
    ModelManifest, ModelStatus, RegistryModelId, Runtime, RuntimeTarget, TaskKind, TaskRequest,
    TrustedHardwareProbe,
};
use tihulu_ai_registry::registry::{
    ArtifactObservation, ArtifactValidation, EndpointObservation, EndpointState, FileFormat,
    ModelId, ModelOwner, ModelRecord, ModelReference, RuntimeVerification, SourceAdapter,
};

fn catalog(name: &str) -> ModelManifest {
    let raw = match name {
        "owned" => include_str!("fixtures/installed-chat.json"),
        "jan" => include_str!("fixtures/jan-owned-gguf.json"),
        "cuda" => include_str!("fixtures/cuda-chat.json"),
        "pi" => include_str!("fixtures/pi-cpu-chat.json"),
        "hailo" => include_str!("fixtures/hailo-vision.json"),
        "asr" => include_str!("fixtures/asr-whisper.json"),
        "translation" => include_str!("fixtures/translation-tr-de.json"),
        "unknown-license" => include_str!("fixtures/unknown-license.json"),
        "paired" => include_str!("fixtures/paired-gpu-endpoint.json"),
        _ => panic!("unknown offline fixture"),
    };
    ModelManifest::from_json(raw).unwrap()
}

fn model_architecture(model: &mut ModelManifest) {
    model.architecture = match model.format {
        ModelFormat::WhisperGgml => ModelArchitecture::Whisper,
        ModelFormat::HailoHef => ModelArchitecture::HailoCompiled,
        ModelFormat::Endpoint => ModelArchitecture::ServedEndpoint,
        ModelFormat::Ctranslate2 => ModelArchitecture::EncoderDecoderTransformer,
        _ => ModelArchitecture::DecoderOnlyTransformer,
    };
}

fn file_record(
    source: &str,
    owner: ModelOwner,
    format: FileFormat,
    validated: bool,
    runtime: &str,
    host: &str,
) -> ModelRecord {
    let observation = ArtifactObservation {
        source_path: PathBuf::from(source),
        canonical_path: PathBuf::from(source),
        owner: owner.clone(),
        adapter: SourceAdapter::CustomFolder,
        format_hint: format,
        size_bytes: 1_024,
    };
    let id = ModelId::from_artifact(&observation);
    ModelRecord {
        id,
        owner,
        reference: ModelReference::File(observation),
        artifact_validation: if validated {
            ArtifactValidation::Validated {
                validator: "synthetic-binary-validator".into(),
                evidence: "synthetic-artifact-validation".into(),
            }
        } else {
            ArtifactValidation::Unchecked
        },
        runtime_verification: if runtime.is_empty() {
            RuntimeVerification::Unchecked
        } else {
            RuntimeVerification::Verified {
                runtime: runtime.into(),
                host: host.into(),
                evidence: "synthetic-fixture-probe".into(),
            }
        },
        missing: false,
    }
}

fn endpoint_record(owner: ModelOwner, runtime: &str, host: &str) -> ModelRecord {
    let observation = EndpointObservation {
        base_url: "https://synthetic.invalid".into(),
        served_model_id: "test:chat".into(),
        owner: owner.clone(),
        adapter: SourceAdapter::LocalInferenceApi,
        state: EndpointState::ListedByAuthorizedClient,
    };
    ModelRecord {
        id: ModelId::from_endpoint(&observation),
        owner,
        reference: ModelReference::Endpoint(observation),
        artifact_validation: ArtifactValidation::Validated {
            validator: "synthetic-endpoint-validator".into(),
            evidence: "synthetic-artifact-validation".into(),
        },
        runtime_verification: RuntimeVerification::Verified {
            runtime: runtime.into(),
            host: host.into(),
            evidence: "synthetic-fixture-probe".into(),
        },
        missing: false,
    }
}

fn selected_target(hardware: HardwareTarget, runtime: Runtime, host: &str) -> RuntimeTarget {
    RuntimeTarget {
        host_id: HostId::new(host).unwrap(),
        runtime,
        hardware,
    }
}

fn probe(target: &RuntimeTarget) -> TrustedHardwareProbe {
    TrustedHardwareProbe {
        target: target.clone(),
        hardware_verified: true, // mock flag, NOT evidence from physical devices
        runtime_ready: true,
        available_ram_mib: Some(16_384),
        available_vram_mib: Some(16_384),
        available_npu_mib: Some(8_192),
        paired_host_authorized: true,
    }
}

fn application(model: &ModelManifest) -> ApplicationTask {
    ApplicationTask {
        application_id: ApplicationId::new("text-center").unwrap(),
        request: TaskRequest {
            task: model.tasks[0],
            input: model.input,
            output: model.output,
            languages: vec![model.languages[0].clone()],
            translation: model.translation_pairs.first().cloned(),
            quantization: model.quantization.clone(),
            requires_tool_calling: false,
            offline_only: true,
            license_allowlist: vec!["Apache-2.0".into()],
        },
        adapter_verified: true,
    }
}

fn inspect(
    model: &ModelManifest,
    record: &ModelRecord,
    id: &ModelId,
    target: &RuntimeTarget,
    environment: &TrustedHardwareProbe,
    app: &ApplicationTask,
) -> tihulu_ai_market::InventoryCompatibilityReport {
    inspect_with_binding(
        model,
        record,
        id,
        target,
        environment,
        app,
        Some("mock-binding"),
    )
}

fn inspect_with_binding(
    model: &ModelManifest,
    record: &ModelRecord,
    id: &ModelId,
    target: &RuntimeTarget,
    environment: &TrustedHardwareProbe,
    app: &ApplicationTask,
    binding: Option<&str>,
) -> tihulu_ai_market::InventoryCompatibilityReport {
    inspect_registered_model(&InventoryInspection {
        catalog: model,
        registry: record,
        selected_model_id: id,
        opaque_handle: RegistryModelId::new("mock-handle-1").unwrap(),
        catalog_binding_evidence: binding,
        application: app,
        target,
        probe: environment,
    })
}

#[test]
fn offline_catalog_and_registry_format_hint_matrix() {
    let all = [
        (FileFormat::Gguf, ModelFormat::Gguf),
        (FileFormat::WhisperGgml, ModelFormat::WhisperGgml),
        (FileFormat::Onnx, ModelFormat::Onnx),
        (FileFormat::Tflite, ModelFormat::Tflite),
        (FileFormat::Safetensors, ModelFormat::Safetensors),
        (FileFormat::HailoHef, ModelFormat::HailoHef),
    ];
    for (registry, expected) in all {
        assert_eq!(registry_format_hint(registry), expected);
    }
    let mut offline = catalog("owned");
    model_architecture(&mut offline);
    let encoded = offline.to_json_pretty().unwrap();
    assert_eq!(ModelManifest::from_json(&encoded).unwrap(), offline);
    assert!(!encoded.contains("/synthetic/private"));
}

#[test]
fn installed_discovered_artifact_verified_runtime_verified_and_preflight_are_distinct() {
    let mut model = catalog("owned");
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::Cpu, Runtime::LlamaCpp, "local");
    let env = probe(&target);
    let app = application(&model);
    let mut record = file_record(
        "/synthetic/private/chat.gguf",
        ModelOwner::AiTools,
        FileFormat::Gguf,
        false,
        "",
        "local",
    );
    let id = record.id.clone();

    let discovered = inspect(&model, &record, &id, &target, &env, &app);
    assert_eq!(discovered.readiness.catalog_status, ModelStatus::Installed);
    assert!(discovered.readiness.discovered);
    assert!(!discovered.readiness.artifact_verified);
    assert!(!discovered.readiness.target_runtime_verified);
    assert!(!discovered.readiness.runnable_preflight);
    assert!(!discovered.decision.can_propose_assignment);

    record.artifact_validation = ArtifactValidation::Validated {
        validator: "mock-binary-validator".into(),
        evidence: "mock-evidence".into(),
    };
    let validated = inspect(&model, &record, &id, &target, &env, &app);
    assert!(validated.readiness.artifact_verified);
    assert!(!validated.readiness.target_runtime_verified);

    record.runtime_verification = RuntimeVerification::Verified {
        runtime: "llama_cpp".into(),
        host: "local".into(),
        evidence: "synthetic-fixture-probe".into(),
    };
    let verified = inspect(&model, &record, &id, &target, &env, &app);
    assert!(verified.readiness.artifact_verified);
    assert!(verified.readiness.target_runtime_verified);
    assert!(verified.readiness.runnable_preflight);
    assert_eq!(
        verified.decision.compatibility.verdict,
        MatchVerdict::Recommended
    );
    assert!(verified.decision.can_propose_assignment);
    assert!(verified.decision.read_only);

    record.missing = true;
    let missing = inspect(&model, &record, &id, &target, &env, &app);
    assert_eq!(missing.readiness.catalog_status, ModelStatus::Installed);
    assert!(!missing.readiness.discovered);
    assert!(!missing.readiness.runnable_preflight);
}

#[test]
fn actual_registry_model_id_prevents_same_basename_false_identity() {
    let mut model = catalog("jan");
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::Cpu, Runtime::LlamaCpp, "local");
    let record = file_record(
        "/synthetic/jan/one/model.gguf",
        ModelOwner::JanDesktop,
        FileFormat::Gguf,
        true,
        "llama_cpp",
        "local",
    );
    let other = file_record(
        "/synthetic/jan/two/model.gguf",
        ModelOwner::JanDesktop,
        FileFormat::Gguf,
        true,
        "llama_cpp",
        "local",
    );
    let report = inspect(
        &model,
        &record,
        &other.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert_eq!(report.readiness.catalog_status, ModelStatus::External);
    assert_eq!(report.decision.compatibility.verdict, MatchVerdict::Unknown);
    assert!(!report.decision.can_propose_assignment);
    let text = format!("{:?}", report);
    assert!(!text.contains("/synthetic/jan/one"));
    assert!(!text.contains("/synthetic/jan/two"));
}

#[test]
fn borrowed_jan_reference_is_reusable_but_not_owned_or_auto_started() {
    let mut model = catalog("jan");
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::Cpu, Runtime::LlamaCpp, "local");
    let record = file_record(
        "/synthetic/jan/model.gguf",
        ModelOwner::JanDesktop,
        FileFormat::Gguf,
        true,
        "llama_cpp",
        "local",
    );
    let ready = inspect(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert_eq!(ready.readiness.catalog_status, ModelStatus::External);
    assert!(ready.readiness.runnable_preflight);
    assert!(ready.decision.read_only);
}

#[test]
fn known_cpu_cuda_apple_pi_hailo_and_paired_host_mappings_are_target_specific() {
    let cases = [
        (
            "owned",
            HardwareTarget::Cpu,
            Runtime::LlamaCpp,
            "local",
            FileFormat::Gguf,
        ),
        (
            "cuda",
            HardwareTarget::Cuda,
            Runtime::LlamaCpp,
            "gpu-pc",
            FileFormat::Gguf,
        ),
        (
            "pi",
            HardwareTarget::RaspberryPiCpu,
            Runtime::LlamaCpp,
            "pi",
            FileFormat::Gguf,
        ),
        (
            "hailo",
            HardwareTarget::Hailo,
            Runtime::HailoRt,
            "pi-hailo",
            FileFormat::HailoHef,
        ),
    ];
    for (name, hardware, runtime, host, format) in cases {
        let mut model = catalog(name);
        model_architecture(&mut model);
        let target = selected_target(hardware, runtime, host);
        let record = file_record(
            "/synthetic/model",
            ModelOwner::AiTools,
            format,
            true,
            match runtime {
                Runtime::LlamaCpp => "llama_cpp",
                Runtime::HailoRt => "hailo_rt",
                _ => unreachable!(),
            },
            host,
        );
        let result = inspect(
            &model,
            &record,
            &record.id,
            &target,
            &probe(&target),
            &application(&model),
        );
        assert_eq!(
            result.decision.compatibility.verdict,
            MatchVerdict::Recommended,
            "{name}"
        );
        assert!(result.readiness.runnable_preflight, "{name}");
    }

    // Registry does not currently index .mlx. ONNX on Apple Silicon is the
    // supported mock path; this is NOT a Metal or real Apple GPU acceptance.
    let mut apple = catalog("owned");
    apple.format = ModelFormat::Onnx;
    apple.runtimes = vec![Runtime::OnnxRuntime];
    apple.hardware_targets = vec![HardwareTarget::AppleSilicon];
    apple.verified_runtimes = vec![tihulu_ai_market::RuntimeVerification {
        runtime: Runtime::OnnxRuntime,
        hardware: HardwareTarget::AppleSilicon,
        evidence_id: "synthetic-fixture-probe".into(),
    }];
    model_architecture(&mut apple);
    let apple_target = selected_target(HardwareTarget::AppleSilicon, Runtime::OnnxRuntime, "mac");
    let apple_record = file_record(
        "/synthetic/apple.onnx",
        ModelOwner::AiTools,
        FileFormat::Onnx,
        true,
        "onnx_runtime",
        "mac",
    );
    let apple_result = inspect(
        &apple,
        &apple_record,
        &apple_record.id,
        &apple_target,
        &probe(&apple_target),
        &application(&apple),
    );
    assert!(apple_result.readiness.runnable_preflight);
    assert_eq!(apple_result.observed_format, Some(ModelFormat::Onnx));

    let mut served = catalog("paired");
    model_architecture(&mut served);
    let remote_target = selected_target(
        HardwareTarget::PairedGpuHost,
        Runtime::OpenAiApi,
        "paired-pc",
    );
    let remote_record = endpoint_record(
        ModelOwner::UserNamed("Paired Host".into()),
        "open_ai_api",
        "paired-pc",
    );
    let remote_result = inspect(
        &served,
        &remote_record,
        &remote_record.id,
        &remote_target,
        &probe(&remote_target),
        &application(&served),
    );
    assert!(remote_result.readiness.runnable_preflight);
    let mut unauthorized = probe(&remote_target);
    unauthorized.paired_host_authorized = false;
    let rejected = inspect(
        &served,
        &remote_record,
        &remote_record.id,
        &remote_target,
        &unauthorized,
        &application(&served),
    );
    assert_eq!(
        rejected.decision.compatibility.verdict,
        MatchVerdict::Incompatible
    );
    assert!(!rejected.readiness.runnable_preflight);
}

#[test]
fn unknown_architecture_and_malformed_or_corrupt_metadata_fail_closed() {
    let model = catalog("owned"); // existing fixture defaults to unknown architecture
    let target = selected_target(HardwareTarget::Cpu, Runtime::LlamaCpp, "local");
    let record = file_record(
        "/synthetic/owned.gguf",
        ModelOwner::AiTools,
        FileFormat::Gguf,
        true,
        "llama_cpp",
        "local",
    );
    let result = inspect(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert_eq!(result.declared_architecture, ModelArchitecture::Unknown);
    assert!(result.readiness.target_runtime_verified);
    assert!(!result.readiness.runnable_preflight);
    assert_eq!(result.decision.compatibility.verdict, MatchVerdict::Unknown);
    assert!(ModelManifest::from_json("{bad json").is_err());
    assert!(ModelManifest::from_json(r#"{"schema_version":999}"#).is_err());

    let mut corrupt = model.clone();
    corrupt.architecture = ModelArchitecture::HailoCompiled;
    let invalid = inspect(
        &corrupt,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&corrupt),
    );
    assert_eq!(
        invalid.decision.compatibility.verdict,
        MatchVerdict::Unknown
    );
    assert!(!invalid.readiness.runnable_preflight);
}

#[test]
fn unsupported_tflite_or_wrong_format_cannot_pass_preflight() {
    let mut model = catalog("owned");
    model.format = ModelFormat::Tflite;
    model.runtimes = vec![Runtime::OnnxRuntime];
    model.verified_runtimes = vec![tihulu_ai_market::RuntimeVerification {
        runtime: Runtime::OnnxRuntime,
        hardware: HardwareTarget::Cpu,
        evidence_id: "synthetic-fixture-probe".into(),
    }];
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::Cpu, Runtime::OnnxRuntime, "local");
    let record = file_record(
        "/synthetic/model.tflite",
        ModelOwner::AiTools,
        FileFormat::Tflite,
        true,
        "onnx_runtime",
        "local",
    );
    let unknown = inspect(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert_eq!(unknown.observed_format, Some(ModelFormat::Tflite));
    assert_eq!(
        unknown.decision.compatibility.verdict,
        MatchVerdict::Unknown
    );
    assert!(!unknown.readiness.runnable_preflight);
    let mut wrong = model;
    wrong.format = ModelFormat::Gguf;
    let rejected = inspect(
        &wrong,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&wrong),
    );
    assert_eq!(
        rejected.decision.compatibility.verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn bad_runtime_evidence_host_or_missing_trust_cannot_become_runnable() {
    let mut model = catalog("owned");
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::Cpu, Runtime::LlamaCpp, "local");
    let mut record = file_record(
        "/synthetic/model.gguf",
        ModelOwner::AiTools,
        FileFormat::Gguf,
        true,
        "llama.cpp", // wrong token, must never be guessed
        "local",
    );
    let unknown = inspect(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert!(!unknown.readiness.target_runtime_verified);
    assert!(!unknown.readiness.runnable_preflight);
    record.runtime_verification = RuntimeVerification::Verified {
        runtime: "llama_cpp".into(),
        host: "unapproved-host".into(),
        evidence: "synthetic-fixture-probe".into(),
    };
    let wrong_host = inspect(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert!(!wrong_host.readiness.runnable_preflight);
    record.runtime_verification = RuntimeVerification::Verified {
        runtime: "llama_cpp".into(),
        host: "local".into(),
        evidence: "synthetic-fixture-probe".into(),
    };
    let mut unverified_probe = probe(&target);
    unverified_probe.hardware_verified = false;
    assert!(
        !inspect(
            &model,
            &record,
            &record.id,
            &target,
            &unverified_probe,
            &application(&model)
        )
        .readiness
        .runnable_preflight
    );
    let no_binding = inspect_with_binding(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
        None,
    );
    assert!(!no_binding.readiness.runnable_preflight);
}

#[test]
fn unsupported_task_license_and_forged_record_ownership_fail_safely() {
    let mut model = catalog("jan");
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::Cpu, Runtime::LlamaCpp, "local");
    let mut record = file_record(
        "/synthetic/jan.gguf",
        ModelOwner::JanDesktop,
        FileFormat::Gguf,
        true,
        "llama_cpp",
        "local",
    );
    let mut app = application(&model);
    app.request.task = TaskKind::Asr;
    app.request.input = Modality::Audio;
    assert_eq!(
        inspect(&model, &record, &record.id, &target, &probe(&target), &app)
            .decision
            .compatibility
            .verdict,
        MatchVerdict::Incompatible
    );
    app = application(&model);
    model.license = License::Unknown;
    assert_eq!(
        inspect(&model, &record, &record.id, &target, &probe(&target), &app)
            .decision
            .compatibility
            .verdict,
        MatchVerdict::Unknown
    );
    model.license = License::Known {
        spdx: "GPL-3.0-only".into(),
    };
    assert_eq!(
        inspect(&model, &record, &record.id, &target, &probe(&target), &app)
            .decision
            .compatibility
            .verdict,
        MatchVerdict::Incompatible
    );
    model.license = License::Known {
        spdx: "Apache-2.0".into(),
    };
    record.owner = ModelOwner::AiTools; // record ID still refers to Jan
    assert_eq!(
        inspect(&model, &record, &record.id, &target, &probe(&target), &app)
            .decision
            .compatibility
            .verdict,
        MatchVerdict::Unknown
    );
}

#[test]
fn missing_and_declared_endpoints_are_observations_not_runtime_execution() {
    let mut model = catalog("paired");
    model_architecture(&mut model);
    let target = selected_target(HardwareTarget::PairedGpuHost, Runtime::OpenAiApi, "remote");
    let mut record = endpoint_record(
        ModelOwner::UserNamed("Paired Host".into()),
        "open_ai_api",
        "remote",
    );
    if let ModelReference::Endpoint(endpoint) = &mut record.reference {
        endpoint.state = EndpointState::Declared;
    }
    let new_id = match &record.reference {
        ModelReference::Endpoint(endpoint) => ModelId::from_endpoint(endpoint),
        _ => unreachable!(),
    };
    record.id = new_id;
    let declared = inspect(
        &model,
        &record,
        &record.id,
        &target,
        &probe(&target),
        &application(&model),
    );
    assert!(!declared.readiness.discovered);
    assert!(!declared.readiness.runnable_preflight);
}

#[test]
fn typed_offline_architecture_fixture_and_untrusted_json_are_bounded() {
    let model =
        ModelManifest::from_json(include_str!("fixtures/phase5-offline-decoder.json")).unwrap();
    assert_eq!(
        model.architecture,
        ModelArchitecture::DecoderOnlyTransformer
    );
    assert_eq!(model.tasks, vec![TaskKind::Chat]);
    let mut invalid = model.to_json_pretty().unwrap();
    invalid.push_str(" trailing-malformed-content");
    assert!(ModelManifest::from_json(&invalid).is_err());

    let mut metadata = model;
    metadata.architecture = ModelArchitecture::ServedEndpoint;
    assert!(metadata.validate().is_err());
}
