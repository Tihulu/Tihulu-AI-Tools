//! QA-CROSS-01: actual Registry PR #20 owner types plus Market PR #22.
//! Synthetic only: no model file IO, inference, runtime grant or device access.

use std::path::PathBuf;

use tihulu_ai_market::{
    inspect_registered_model, ApplicationId, ApplicationTask, HardwareTarget, HostId,
    InventoryCompatibilityReport, InventoryInspection, MatchVerdict, ModelArchitecture,
    ModelManifest, ModelSource, ModelStatus, Modality, RegistryModelId, RegistryOwner, Runtime,
    RuntimeTarget,
    TaskKind, TaskRequest, TrustedHardwareProbe,
};
use tihulu_ai_registry::registry::{
    ArtifactObservation, ArtifactValidation, FileFormat, ModelId, ModelOwner, ModelRecord,
    ModelReference, RuntimeVerification, SourceAdapter,
};

const OWNER_APP: &str = "tihulu-media-center";
const CONSUMER_APP: &str = "tihulu-text-center";

fn manifest() -> ModelManifest {
    let mut model = ModelManifest::from_json(include_str!("fixtures/installed-chat.json"))
        .expect("versioned offline fixture");
    model.source = ModelSource::TihuluApp {
        app_id: OWNER_APP.into(),
    };
    model.availability = ModelStatus::External; // NOT AI Tools owned
    model.architecture = ModelArchitecture::DecoderOnlyTransformer;
    model
}

fn record(owner: ModelOwner, adapter: SourceAdapter, verified: bool) -> ModelRecord {
    let observation = ArtifactObservation {
        source_path: PathBuf::from("/synthetic/tihulu/models/chat.gguf"),
        canonical_path: PathBuf::from("/synthetic/tihulu/models/chat.gguf"),
        owner: owner.clone(),
        adapter,
        format_hint: FileFormat::Gguf,
        size_bytes: 4096,
    };
    ModelRecord {
        id: ModelId::from_artifact(&observation),
        owner,
        reference: ModelReference::File(observation),
        artifact_validation: if verified {
            ArtifactValidation::Validated {
                validator: "synthetic-validator".into(),
                evidence: "synthetic-artifact".into(),
            }
        } else {
            ArtifactValidation::Unchecked
        },
        runtime_verification: if verified {
            RuntimeVerification::Verified {
                runtime: "llama_cpp".into(),
                host: "local".into(),
                evidence: "synthetic-fixture-probe".into(),
            }
        } else {
            RuntimeVerification::Unchecked
        },
        missing: false,
    }
}

fn report(model: &ModelManifest, record: &ModelRecord, consumer: &str) -> InventoryCompatibilityReport {
    let target = RuntimeTarget {
        host_id: HostId::new("local").unwrap(),
        runtime: Runtime::LlamaCpp,
        hardware: HardwareTarget::Cpu,
    };
    let probe = TrustedHardwareProbe {
        target: target.clone(),
        hardware_verified: true, // synthetic, not an actual hardware attestation
        runtime_ready: true,
        available_ram_mib: Some(16384),
        available_vram_mib: Some(16384),
        available_npu_mib: Some(8192),
        paired_host_authorized: false,
    };
    let app = ApplicationTask {
        application_id: ApplicationId::new(consumer).unwrap(),
        adapter_verified: true,
        request: TaskRequest {
            task: TaskKind::Chat,
            input: Modality::Text,
            output: Modality::Text,
            languages: vec!["en".into()],
            translation: None,
            quantization: None,
            requires_tool_calling: false,
            offline_only: true,
            license_allowlist: vec!["Apache-2.0".into()],
        },
    };
    inspect_registered_model(&InventoryInspection {
        catalog: model,
        registry: record,
        selected_model_id: &record.id,
        opaque_handle: RegistryModelId::new("private-fixture-handle").unwrap(),
        catalog_binding_evidence: Some("synthetic-consent-scoped-catalog-binding"),
        application: &app,
        target: &target,
        probe: &probe,
    })
}

#[test]
fn tihulu_application_model_stays_external_and_discovered_is_not_runnable() {
    let model = manifest();
    let owner = ModelOwner::TihuluApp(OWNER_APP.into());
    let observed = record(owner.clone(), SourceAdapter::TihuluAppDirectory, false);
    let result = report(&model, &observed, OWNER_APP);
    assert_eq!(result.readiness.catalog_status, ModelStatus::External);
    assert_eq!(result.source_owner, RegistryOwner::TihuluApp(OWNER_APP.into()));
    assert!(result.readiness.discovered);
    assert!(!result.readiness.artifact_verified);
    assert!(!result.readiness.target_runtime_verified);
    assert!(!result.readiness.runnable_preflight);
    assert!(!result.decision.can_propose_assignment);
    assert!(result.decision.read_only);

    let verified = record(owner.clone(), SourceAdapter::TihuluAppDirectory, true);
    let preflight = report(&model, &verified, OWNER_APP);
    assert_eq!(preflight.decision.compatibility.verdict, MatchVerdict::Recommended);
    assert_eq!(preflight.readiness.catalog_status, ModelStatus::External);
    assert!(preflight.readiness.runnable_preflight);
    assert!(preflight.decision.read_only);
    assert_eq!(verified.owner, owner); // no file ownership transfer
}

#[test]
fn distinct_tihulu_app_ids_and_usernamed_aliases_never_match() {
    let model = manifest();
    let other_owner = record(
        ModelOwner::TihuluApp(CONSUMER_APP.into()),
        SourceAdapter::TihuluAppDirectory,
        true,
    );
    let mismatch = report(&model, &other_owner, OWNER_APP);
    assert_eq!(mismatch.decision.compatibility.verdict, MatchVerdict::Incompatible);
    assert!(!mismatch.decision.can_propose_assignment);

    let generic_alias = record(
        ModelOwner::UserNamed(OWNER_APP.into()),
        SourceAdapter::CustomFolder,
        true,
    );
    let alias = report(&model, &generic_alias, OWNER_APP);
    assert_eq!(alias.decision.compatibility.verdict, MatchVerdict::Incompatible);
    assert!(!alias.readiness.runnable_preflight);

    let mut forged_source = model;
    forged_source.source = ModelSource::External {
        application: OWNER_APP.into(),
    };
    let real_owner = record(
        ModelOwner::TihuluApp(OWNER_APP.into()),
        SourceAdapter::TihuluAppDirectory,
        true,
    );
    let forged = report(&forged_source, &real_owner, OWNER_APP);
    assert_eq!(forged.decision.compatibility.verdict, MatchVerdict::Incompatible);
}

#[test]
fn another_app_cannot_assign_model_even_with_verified_synthetic_probe() {
    let model = manifest();
    let borrowed = record(
        ModelOwner::TihuluApp(OWNER_APP.into()),
        SourceAdapter::TihuluAppDirectory,
        true,
    );
    let denied = report(&model, &borrowed, CONSUMER_APP);
    assert_eq!(denied.readiness.catalog_status, ModelStatus::External);
    assert_eq!(denied.decision.compatibility.verdict, MatchVerdict::Unknown);
    assert!(!denied.readiness.runnable_preflight);
    assert!(!denied.decision.can_propose_assignment);

    // A generic custom folder must not impersonate an approved per-app root.
    let unapproved = record(
        ModelOwner::TihuluApp(OWNER_APP.into()),
        SourceAdapter::CustomFolder,
        true,
    );
    let wrong_scope = report(&model, &unapproved, OWNER_APP);
    assert_eq!(wrong_scope.decision.compatibility.verdict, MatchVerdict::Unknown);
    assert!(!wrong_scope.decision.can_propose_assignment);
}

#[test]
fn app_owner_catalog_ids_must_be_valid_and_cannot_claim_ai_tools_installation() {
    let mut model = manifest();
    model.availability = ModelStatus::Installed;
    assert!(model.validate().is_err());

    model.availability = ModelStatus::External;
    model.source = ModelSource::TihuluApp { app_id: "tihulu_media_center".into() };
    assert!(model.validate().is_err());
    model.source = ModelSource::TihuluApp { app_id: String::new() };
    assert!(model.validate().is_err());
    model.source = ModelSource::TihuluApp { app_id: OWNER_APP.into() };
    assert!(model.validate().is_ok());
}
