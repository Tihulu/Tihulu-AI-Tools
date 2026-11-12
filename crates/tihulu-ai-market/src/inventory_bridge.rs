//! Phase 5: read-only bridge to the actual Registry ModelRecord/ModelId.
//!
//! Market never reads the file, guesses a neural architecture from a suffix,
//! downloads weights, starts a runtime, or creates resource leases. An opaque
//! UI handle must be issued and authorized by a local Registry adapter;
//! private Registry ModelId::Artifact paths must not leave that adapter.

use tihulu_ai_registry::registry::{
    ArtifactValidation as RegistryArtifactValidation, EndpointState, FileFormat, ModelId,
    ModelOwner, ModelRecord, ModelReference, RuntimeVerification as RegistryVerification,
    SourceAdapter,
};

use crate::manifest::{ModelArchitecture, ModelFormat, ModelManifest, ModelStatus, TaskKind};
use crate::matching::{MatchResult, MatchVerdict};
use crate::registry_contract::{
    match_registry_model, ApplicationTask, ArtifactEvidence, CatalogIdentityBinding,
    RegistryMatchDecision, RegistryMatchRequest, RegistryModelId, RegistryModelSnapshot,
    RegistryOwner, RegistryPresence, RegistryRuntimeEvidence, RuntimeTarget, TrustedHardwareProbe,
};

/// The caller supplies one local, permission-scoped observation. Inputs are
/// data, not proof that the producer is trusted: the coordinator must ensure
/// only a trusted Registry/runtime adapter can populate the evidence fields.
pub struct InventoryInspection<'a> {
    pub catalog: &'a ModelManifest,
    pub registry: &'a ModelRecord,
    /// The exact typed source identity selected by Registry, not a filename.
    pub selected_model_id: &'a ModelId,
    /// Adapter-issued opaque handle; never a serialized ModelId or file path.
    pub opaque_handle: RegistryModelId,
    /// Evidence from an independent, approved catalog-to-source binding.
    pub catalog_binding_evidence: Option<&'a str>,
    pub application: &'a ApplicationTask,
    pub target: &'a RuntimeTarget,
    pub probe: &'a TrustedHardwareProbe,
}

/// Separate observations. Installed is ownership/catalog status, not a
/// synonym for discovered, artifact-verified or runnable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryReadiness {
    pub catalog_status: ModelStatus,
    pub discovered: bool,
    pub artifact_verified: bool,
    pub target_runtime_verified: bool,
    /// Deterministic preflight only, never a physical inference claim.
    pub runnable_preflight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryCompatibilityReport {
    pub readiness: InventoryReadiness,
    /// Exact typed Registry owner; independent TIHULU app ID is not erased.
    pub source_owner: RegistryOwner,
    /// This is only the Registry's extension/name format HINT.
    pub observed_format: Option<ModelFormat>,
    /// Versioned catalog claim, not architecture read from model weights.
    pub declared_architecture: ModelArchitecture,
    pub declared_tasks: Vec<TaskKind>,
    /// Contains no paths, credentials, raw Registry ModelId or model content.
    pub decision: RegistryMatchDecision,
}

pub fn registry_format_hint(format: FileFormat) -> ModelFormat {
    match format {
        FileFormat::Gguf => ModelFormat::Gguf,
        FileFormat::Onnx => ModelFormat::Onnx,
        FileFormat::Safetensors => ModelFormat::Safetensors,
        FileFormat::Tflite => ModelFormat::Tflite,
        FileFormat::HailoHef => ModelFormat::HailoHef,
        FileFormat::WhisperGgml => ModelFormat::WhisperGgml,
    }
}

fn registry_owner(owner: &ModelOwner) -> RegistryOwner {
    match owner {
        ModelOwner::AiTools => RegistryOwner::AiTools,
        ModelOwner::JanDesktop => RegistryOwner::JanDesktop,
        ModelOwner::Ollama => RegistryOwner::Ollama,
        ModelOwner::HuggingFace => RegistryOwner::HuggingFace,
        ModelOwner::TihuluApp(app_id) => RegistryOwner::TihuluApp(app_id.clone()),
        ModelOwner::UserNamed(name) => RegistryOwner::UserNamed(name.clone()),
    }
}

fn format_presence(record: &ModelRecord) -> (RegistryPresence, Option<ModelFormat>) {
    if record.missing {
        return (RegistryPresence::Missing, None);
    }
    match &record.reference {
        ModelReference::File(artifact) => {
            let hint = registry_format_hint(artifact.format_hint);
            (
                RegistryPresence::FileObserved { format_hint: hint },
                Some(hint),
            )
        }
        ModelReference::Endpoint(endpoint) => {
            let presence = match endpoint.state {
                EndpointState::Declared => RegistryPresence::EndpointDeclared,
                EndpointState::ListedByAuthorizedClient => RegistryPresence::EndpointListed,
            };
            (presence, Some(ModelFormat::Endpoint))
        }
    }
}

fn exact_runtime_name(runtime: crate::manifest::Runtime) -> &'static str {
    use crate::manifest::Runtime;
    match runtime {
        Runtime::LlamaCpp => "llama_cpp",
        Runtime::WhisperCpp => "whisper_cpp",
        Runtime::OnnxRuntime => "onnx_runtime",
        Runtime::Mlx => "mlx",
        Runtime::Ctranslate2 => "ctranslate2",
        Runtime::HailoRt => "hailo_rt",
        Runtime::JanApi => "jan_api",
        Runtime::OllamaApi => "ollama_api",
        Runtime::OpenAiApi => "open_ai_api",
        Runtime::Unknown => "",
    }
}

fn source_consistent(record: &ModelRecord) -> bool {
    if record.id
        != match &record.reference {
            ModelReference::File(file) => ModelId::from_artifact(file),
            ModelReference::Endpoint(endpoint) => ModelId::from_endpoint(endpoint),
        }
    {
        return false;
    }
    match (&record.owner, &record.reference) {
        // An independent app's model can only come from its dedicated,
        // consent-scoped directory adapter, never a generic custom folder.
        (ModelOwner::TihuluApp(app_id), ModelReference::File(file)) => {
            record.owner == file.owner
                && file.adapter == SourceAdapter::TihuluAppDirectory
                && !app_id.is_empty()
        }
        (ModelOwner::TihuluApp(_), ModelReference::Endpoint(_)) => false,
        (owner, ModelReference::File(file)) => *owner == file.owner,
        (owner, ModelReference::Endpoint(endpoint)) => *owner == endpoint.owner,
    }
}

fn task_architecture_conflict(architecture: ModelArchitecture, task: TaskKind) -> bool {
    match architecture {
        ModelArchitecture::Whisper => !matches!(task, TaskKind::Asr | TaskKind::Translation),
        ModelArchitecture::EmbeddingEncoder => task != TaskKind::RagEmbedding,
        // Do not assume any other neural family cannot support a task.
        _ => false,
    }
}

fn unavailable(
    input: &InventoryInspection<'_>,
    presence: RegistryPresence,
    observed_format: Option<ModelFormat>,
    verdict: MatchVerdict,
    reason: &str,
) -> InventoryCompatibilityReport {
    let status = match verdict {
        MatchVerdict::Incompatible => ModelStatus::Incompatible,
        _ => ModelStatus::Unknown,
    };
    let decision = RegistryMatchDecision {
        catalog_model_id: input.catalog.id.clone(),
        registry_model_id: input.opaque_handle.clone(),
        application_id: input.application.application_id.clone(),
        target: input.target.clone(),
        inventory_status: input.catalog.availability,
        observed_presence: presence.clone(),
        compatibility: MatchResult {
            model_id: input.catalog.id.clone(),
            status,
            verdict,
            reasons: vec![reason.to_owned()],
        },
        can_propose_assignment: false,
        read_only: true,
    };
    InventoryCompatibilityReport {
        readiness: InventoryReadiness {
            catalog_status: input.catalog.availability,
            discovered: !matches!(
                presence,
                RegistryPresence::Missing | RegistryPresence::EndpointDeclared
            ),
            artifact_verified: false,
            target_runtime_verified: false,
            runnable_preflight: false,
        },
        observed_format,
        source_owner: registry_owner(&input.registry.owner),
        declared_architecture: input.catalog.architecture,
        declared_tasks: input.catalog.tasks.clone(),
        decision,
    }
}

/// Pure deterministic evaluation; no IO, downloads or actual model load.
///
/// A verified Registry string is only admitted as runtime evidence if its
/// canonical token and host ID exactly match the separately trusted target
/// probe. Unknown strings, missing artifact validation and untrusted metadata
/// cannot become runnable. This is not hardware acceptance.
pub fn inspect_registered_model(input: &InventoryInspection<'_>) -> InventoryCompatibilityReport {
    let (presence, observed_format) = format_presence(input.registry);
    if input.registry.id != *input.selected_model_id || !source_consistent(input.registry) {
        return unavailable(
            input,
            presence,
            observed_format,
            MatchVerdict::Unknown,
            "Registry source identity or ownership is inconsistent",
        );
    }
    if let Err(_error) = input.catalog.validate() {
        return unavailable(
            input,
            presence,
            observed_format,
            MatchVerdict::Unknown,
            "Catalog manifest is invalid or corrupt",
        );
    }

    let binding = input
        .catalog_binding_evidence
        .filter(|evidence| !evidence.trim().is_empty())
        .map(|evidence| CatalogIdentityBinding {
            catalog_model_id: input.catalog.id.clone(),
            evidence_id: evidence.to_owned(),
        });

    let artifact = match &input.registry.artifact_validation {
        RegistryArtifactValidation::Validated {
            validator,
            evidence,
        } if !validator.trim().is_empty() && !evidence.trim().is_empty() => {
            ArtifactEvidence::Validated {
                evidence_id: evidence.clone(),
            }
        }
        _ => ArtifactEvidence::Unchecked,
    };

    let runtime = match &input.registry.runtime_verification {
        RegistryVerification::Verified {
            runtime,
            host,
            evidence,
        } if !evidence.trim().is_empty()
            && !exact_runtime_name(input.target.runtime).is_empty()
            && runtime == exact_runtime_name(input.target.runtime)
            && host == input.target.host_id.as_str()
            && input.probe.target == *input.target
            && input.probe.hardware_verified =>
        {
            RegistryRuntimeEvidence::Verified {
                runtime: input.target.runtime,
                hardware: input.target.hardware,
                host_id: input.target.host_id.clone(),
                evidence_id: evidence.clone(),
            }
        }
        RegistryVerification::Incompatible { reason } => RegistryRuntimeEvidence::Incompatible {
            reason: reason.clone(),
        },
        _ => RegistryRuntimeEvidence::Unchecked,
    };

    let snapshot = RegistryModelSnapshot {
        id: input.opaque_handle.clone(),
        catalog_binding: binding,
        owner: registry_owner(&input.registry.owner),
        presence: presence.clone(),
        artifact,
        runtime,
    };
    let request = RegistryMatchRequest {
        catalog_model_id: input.catalog.id.clone(),
        registry_model_id: input.opaque_handle.clone(),
        application_task: input.application.clone(),
        target: input.target.clone(),
    };
    let artifact_verified = matches!(snapshot.artifact, ArtifactEvidence::Validated { .. });
    let target_runtime_verified =
        matches!(snapshot.runtime, RegistryRuntimeEvidence::Verified { .. });
    let mut decision = match_registry_model(input.catalog, &snapshot, &request, input.probe);
    if decision.compatibility.verdict == MatchVerdict::Recommended {
        if input.catalog.architecture == ModelArchitecture::Unknown
            && input.catalog.format != ModelFormat::Endpoint
        {
            decision.compatibility.verdict = MatchVerdict::Unknown;
            decision.compatibility.status = ModelStatus::Unknown;
            decision
                .compatibility
                .reasons
                .push("Model architecture was not declared or independently inspected".into());
            decision.can_propose_assignment = false;
        } else if task_architecture_conflict(
            input.catalog.architecture,
            input.application.request.task,
        ) {
            decision.compatibility.verdict = MatchVerdict::Incompatible;
            decision.compatibility.status = ModelStatus::Incompatible;
            decision
                .compatibility
                .reasons
                .push("Declared neural architecture contradicts application task".into());
            decision.can_propose_assignment = false;
        }
    }
    let discovered = !matches!(
        presence,
        RegistryPresence::Missing | RegistryPresence::EndpointDeclared
    );
    let runnable_preflight = discovered
        && artifact_verified
        && target_runtime_verified
        && decision.can_propose_assignment;
    InventoryCompatibilityReport {
        readiness: InventoryReadiness {
            catalog_status: input.catalog.availability,
            discovered,
            artifact_verified: discovered && artifact_verified,
            target_runtime_verified: discovered && artifact_verified && target_runtime_verified,
            runnable_preflight,
        },
        observed_format,
        source_owner: registry_owner(&input.registry.owner),
        declared_architecture: input.catalog.architecture,
        declared_tasks: input.catalog.tasks.clone(),
        decision,
    }
}
