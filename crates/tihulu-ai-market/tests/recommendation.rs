//! A synthetic app/task → Registry identity → Market recommendation flow.
//! No downloads, models, processes, hardware access, or leases are involved.

use tihulu_ai_market::{
    recommend_registered_models, ApplicationId, ApplicationTask, ArtifactEvidence,
    CatalogIdentityBinding, HardwareTarget, HostId, MatchVerdict, Modality, ModelFormat,
    ModelManifest, ModelSource, ModelStatus, RegistryCandidate, RegistryModelId,
    RegistryModelSnapshot, RegistryOwner, RegistryPresence, RegistryRuntimeEvidence, Runtime,
    RuntimeTarget, TaskKind, TaskRequest, TrustedHardwareProbe,
};

fn fixture(name: &str) -> ModelManifest {
    let json = match name {
        "owned" => include_str!("fixtures/installed-chat.json"),
        "jan" => include_str!("fixtures/jan-owned-gguf.json"),
        "cuda" => include_str!("fixtures/cuda-chat.json"),
        "hailo" => include_str!("fixtures/hailo-vision.json"),
        "unknown-license" => include_str!("fixtures/unknown-license.json"),
        "online" => include_str!("fixtures/untrusted-online.json"),
        _ => panic!("unknown fixture"),
    };
    ModelManifest::from_json(json).unwrap()
}

fn synthetic_owner(model: &ModelManifest) -> RegistryOwner {
    match &model.source {
        ModelSource::External { application } if application == "Jan Desktop" => {
            RegistryOwner::JanDesktop
        }
        _ => RegistryOwner::AiTools,
    }
}

fn snapshot(model: &ModelManifest, target: &RuntimeTarget) -> RegistryModelSnapshot {
    RegistryModelSnapshot {
        id: RegistryModelId::new(format!("private-synthetic:{}", model.id)).unwrap(),
        catalog_binding: Some(CatalogIdentityBinding {
            catalog_model_id: model.id.clone(),
            evidence_id: "synthetic-binding".into(),
        }),
        owner: synthetic_owner(model),
        presence: RegistryPresence::FileObserved {
            format_hint: model.format,
        },
        artifact: ArtifactEvidence::Validated {
            evidence_id: "synthetic-validated-artifact".into(),
        },
        runtime: RegistryRuntimeEvidence::Verified {
            runtime: target.runtime,
            hardware: target.hardware,
            host_id: target.host_id.clone(),
            evidence_id: "synthetic-fixture-probe".into(),
        },
    }
}

fn target(host: &str, runtime: Runtime, hardware: HardwareTarget) -> RuntimeTarget {
    RuntimeTarget {
        host_id: HostId::new(host).unwrap(),
        runtime,
        hardware,
    }
}

fn probe(target: &RuntimeTarget, gpu_vram: u64) -> TrustedHardwareProbe {
    TrustedHardwareProbe {
        target: target.clone(),
        hardware_verified: true,
        runtime_ready: true,
        available_ram_mib: Some(16_384),
        available_vram_mib: Some(gpu_vram),
        available_npu_mib: Some(8192),
        paired_host_authorized: false,
    }
}

fn chat_task() -> ApplicationTask {
    ApplicationTask {
        application_id: ApplicationId::new("survival-kit").unwrap(),
        request: TaskRequest {
            task: TaskKind::Chat,
            input: Modality::Text,
            output: Modality::Text,
            languages: vec!["en".into(), "tr".into()],
            translation: None,
            quantization: None,
            requires_tool_calling: false,
            offline_only: true,
            license_allowlist: vec!["Apache-2.0".into()],
        },
        adapter_verified: true,
    }
}

#[test]
fn app_task_prefers_verified_installed_then_verified_external_before_unknowns() {
    let models = [
        fixture("online"),
        fixture("cuda"),
        fixture("jan"),
        fixture("unknown-license"),
        fixture("owned"),
        fixture("hailo"),
    ];
    let targets = vec![
        target("local", Runtime::LlamaCpp, HardwareTarget::Cpu),
        target("pc", Runtime::LlamaCpp, HardwareTarget::Cuda),
        target("local", Runtime::LlamaCpp, HardwareTarget::Cpu),
        target("local", Runtime::LlamaCpp, HardwareTarget::Cpu),
        target("local", Runtime::LlamaCpp, HardwareTarget::Cpu),
        target("pi-hailo", Runtime::HailoRt, HardwareTarget::Hailo),
    ];
    let snapshots: Vec<_> = models
        .iter()
        .zip(&targets)
        .map(|(model, target)| snapshot(model, target))
        .collect();
    let probes: Vec<_> = targets.iter().map(|target| probe(target, 1024)).collect();
    let options: Vec<_> = models
        .iter()
        .zip(&snapshots)
        .zip(&targets)
        .zip(&probes)
        .map(
            |(((manifest, registry), target), probe)| RegistryCandidate {
                manifest,
                registry,
                target,
                probe,
            },
        )
        .collect();

    let results = recommend_registered_models(&chat_task(), &options);
    assert_eq!(results.len(), 6);
    assert_eq!(results[0].catalog_model_id, "fixture:owned-chat-q4");
    assert_eq!(results[0].inventory_status, ModelStatus::Installed);
    assert_eq!(results[0].compatibility.verdict, MatchVerdict::Recommended);
    assert_eq!(results[1].catalog_model_id, "fixture:jan-chat");
    assert_eq!(results[1].inventory_status, ModelStatus::External);
    assert_eq!(results[1].compatibility.verdict, MatchVerdict::Recommended);
    assert!(results[0].can_propose_assignment);
    assert!(results[1].can_propose_assignment);

    let unlicensed = results
        .iter()
        .find(|result| result.catalog_model_id == "fixture:unknown-license")
        .unwrap();
    assert_eq!(unlicensed.compatibility.verdict, MatchVerdict::Unknown);
    let cuda = results
        .iter()
        .find(|result| result.catalog_model_id == "fixture:cuda-chat")
        .unwrap();
    assert_eq!(cuda.compatibility.verdict, MatchVerdict::Incompatible);
    let hailo = results
        .iter()
        .find(|result| result.catalog_model_id == "fixture:hailo-vision")
        .unwrap();
    assert_eq!(hailo.compatibility.verdict, MatchVerdict::Incompatible);

    assert!(results.iter().all(|decision| decision.read_only));
    assert!(results
        .iter()
        .skip(2)
        .all(|decision| !decision.can_propose_assignment));

    let mut reverse = options;
    reverse.reverse();
    let shuffled = recommend_registered_models(&chat_task(), &reverse);
    assert_eq!(results, shuffled);
}

#[test]
fn untrusted_format_claims_do_not_override_registry_observation() {
    let model = fixture("owned");
    let selected = target("local", Runtime::LlamaCpp, HardwareTarget::Cpu);
    let mut registry = snapshot(&model, &selected);
    registry.presence = RegistryPresence::FileObserved {
        format_hint: ModelFormat::Onnx,
    };
    let hardware = probe(&selected, 16384);
    let choices = [RegistryCandidate {
        manifest: &model,
        registry: &registry,
        target: &selected,
        probe: &hardware,
    }];
    let results = recommend_registered_models(&chat_task(), &choices);
    assert_eq!(results[0].compatibility.verdict, MatchVerdict::Incompatible);
    assert!(!results[0].can_propose_assignment);
}
