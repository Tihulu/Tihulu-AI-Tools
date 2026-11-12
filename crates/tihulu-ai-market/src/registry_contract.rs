//! Read-only typed Market → Registry matching boundary (Phase 2).
//!
//! Registry reference identity is NOT an artifact digest. Market never creates
//! model leases, unload decisions, downloads, or trusted hardware observations.
//! This contract accepts snapshots from a separately authorized Registry/Core
//! adapter; a struct supplied by a model card is NOT an authenticated probe.

use crate::manifest::{
    HardwareTarget, ModelFormat, ModelManifest, ModelSource, ModelStatus, Runtime,
};
use crate::matching::{
    match_model, CompatibilityContext, MatchResult, MatchVerdict, TaskRequest, TrustedRuntimeProbe,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RegistryModelId(String);

impl RegistryModelId {
    /// Accept an adapter-issued opaque handle for Registry's typed ModelId.
    /// Do not stringify ModelId::Artifact paths or expose raw endpoint URLs.
    pub fn new(id: impl Into<String>) -> Result<Self, ContractError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ContractError::EmptyIdentifier);
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApplicationId(String);

impl ApplicationId {
    pub fn new(id: impl Into<String>) -> Result<Self, ContractError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ContractError::EmptyIdentifier);
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HostId(String);

impl HostId {
    pub fn new(id: impl Into<String>) -> Result<Self, ContractError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ContractError::EmptyIdentifier);
        }
        Ok(Self(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractError {
    EmptyIdentifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryOwner {
    AiTools,
    JanDesktop,
    Ollama,
    HuggingFace,
    UserNamed(String),
    ExternalService(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryPresence {
    /// File metadata was discovered in user-approved scope. Just a format HINT.
    FileObserved {
        format_hint: ModelFormat,
    },
    /// A permitted client listed this model; this is not an inference test.
    EndpointListed,
    /// An endpoint was configured but has not been observed responding.
    EndpointDeclared,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactEvidence {
    Unchecked,
    Validated { evidence_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryRuntimeEvidence {
    Unchecked,
    Verified {
        runtime: Runtime,
        hardware: HardwareTarget,
        host_id: HostId,
        evidence_id: String,
    },
    Incompatible {
        reason: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIdentityBinding {
    /// Only an authorized Registry adapter can establish this provenance link.
    pub catalog_model_id: String,
    pub evidence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryModelSnapshot {
    /// Opaque local handle issued by a trusted adapter for Registry ModelId.
    /// Agent 02's ModelId is a provenance enum, not a ref-v1 hash/string.
    pub id: RegistryModelId,
    /// Explicit identity mapping: opaque handles are NOT model digests.
    pub catalog_binding: Option<CatalogIdentityBinding>,
    pub owner: RegistryOwner,
    pub presence: RegistryPresence,
    /// Registry's trusted artifact-validation state, not a model-card claim.
    pub artifact: ArtifactEvidence,
    /// Registry's independent runtime result, not a manifest's assertion.
    pub runtime: RegistryRuntimeEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationTask {
    pub application_id: ApplicationId,
    pub request: TaskRequest,
    /// This assertion must come from the authorized application adapter.
    pub adapter_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTarget {
    pub host_id: HostId,
    pub runtime: Runtime,
    pub hardware: HardwareTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedHardwareProbe {
    /// Trusted host/runtime adapter supplies this, never remote catalog text.
    pub target: RuntimeTarget,
    pub hardware_verified: bool,
    pub runtime_ready: bool,
    pub available_ram_mib: Option<u64>,
    pub available_vram_mib: Option<u64>,
    pub available_npu_mib: Option<u64>,
    /// Pairing approval is independent of a model's claimed capability.
    pub paired_host_authorized: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryMatchRequest {
    /// Catalog ID, not automatically a Registry ID or content digest.
    pub catalog_model_id: String,
    pub registry_model_id: RegistryModelId,
    pub application_task: ApplicationTask,
    pub target: RuntimeTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryMatchDecision {
    pub catalog_model_id: String,
    pub registry_model_id: RegistryModelId,
    pub application_id: ApplicationId,
    pub target: RuntimeTarget,
    /// Inventory is independent from whether this task can run.
    pub inventory_status: ModelStatus,
    /// A discovered/declared source is not necessarily reachable or usable.
    pub observed_presence: RegistryPresence,
    pub compatibility: MatchResult,
    /// A preflight verdict ONLY; not a grant to load a runtime or download.
    pub can_propose_assignment: bool,
    pub read_only: bool,
}

fn owner_agrees(manifest: &ModelManifest, observed: &RegistryOwner) -> bool {
    match (&manifest.source, observed) {
        (ModelSource::Tihulu, RegistryOwner::AiTools) => true,
        (ModelSource::External { application }, RegistryOwner::JanDesktop) => {
            application.eq_ignore_ascii_case("Jan Desktop")
        }
        (ModelSource::External { application }, RegistryOwner::Ollama) => {
            application.eq_ignore_ascii_case("Ollama")
        }
        (ModelSource::External { application }, RegistryOwner::HuggingFace) => {
            application.eq_ignore_ascii_case("Hugging Face")
        }
        (ModelSource::External { application }, RegistryOwner::UserNamed(label))
        | (ModelSource::External { application }, RegistryOwner::ExternalService(label)) => {
            application == label
        }
        _ => false,
    }
}

fn decision(
    request: &RegistryMatchRequest,
    registry: &RegistryModelSnapshot,
    manifest: &ModelManifest,
    compatibility: MatchResult,
) -> RegistryMatchDecision {
    let can_propose_assignment = compatibility.verdict == MatchVerdict::Recommended
        && matches!(
            manifest.availability,
            ModelStatus::Installed | ModelStatus::External
        );
    RegistryMatchDecision {
        catalog_model_id: request.catalog_model_id.clone(),
        registry_model_id: request.registry_model_id.clone(),
        application_id: request.application_task.application_id.clone(),
        target: request.target.clone(),
        inventory_status: manifest.availability,
        observed_presence: registry.presence.clone(),
        compatibility,
        can_propose_assignment,
        read_only: true,
    }
}

fn reject(
    request: &RegistryMatchRequest,
    registry: &RegistryModelSnapshot,
    manifest: &ModelManifest,
    verdict: MatchVerdict,
    status: ModelStatus,
    reason: impl Into<String>,
) -> RegistryMatchDecision {
    decision(
        request,
        registry,
        manifest,
        MatchResult {
            model_id: manifest.id.clone(),
            verdict,
            status,
            reasons: vec![reason.into()],
        },
    )
}

/// Evaluate exactly one catalog→Registry binding for one application task and
/// one target. This pure function cannot assign, download, load or unload.
/// Registry binding / probe authenticity MUST be checked by the caller.
/// A Registry lease is never owned or released by this module.
/// An authorized Registry adapter keeps Agent 02's typed ModelId (which may
/// contain private file paths) private and maps it to this opaque handle.
pub fn match_registry_model(
    manifest: &ModelManifest,
    registry: &RegistryModelSnapshot,
    request: &RegistryMatchRequest,
    probe: &TrustedHardwareProbe,
) -> RegistryMatchDecision {
    if manifest.id != request.catalog_model_id || registry.id != request.registry_model_id {
        return reject(
            request,
            registry,
            manifest,
            MatchVerdict::Unknown,
            ModelStatus::Unknown,
            "catalog and registry identities do not match the requested binding",
        );
    }
    if !matches!(
        &registry.catalog_binding,
        Some(binding)
            if binding.catalog_model_id == manifest.id
                && !binding.evidence_id.trim().is_empty()
    ) {
        return reject(
            request,
            registry,
            manifest,
            MatchVerdict::Unknown,
            ModelStatus::Unknown,
            "Registry has not verified the catalog-to-reference provenance binding",
        );
    }
    if manifest.validate().is_err() {
        return reject(
            request,
            registry,
            manifest,
            MatchVerdict::Unknown,
            ModelStatus::Unknown,
            "catalog manifest is invalid or contradictory",
        );
    }
    if !owner_agrees(manifest, &registry.owner) {
        return reject(
            request,
            registry,
            manifest,
            MatchVerdict::Incompatible,
            ModelStatus::Incompatible,
            "catalog source ownership contradicts the registry owner",
        );
    }
    if manifest.availability == ModelStatus::Downloadable {
        return reject(
            request,
            registry,
            manifest,
            MatchVerdict::Unknown,
            ModelStatus::Unknown,
            "downloadable catalog entry is not an installed registry model",
        );
    }
    match &registry.presence {
        RegistryPresence::Missing => {
            return reject(
                request,
                registry,
                manifest,
                MatchVerdict::Unknown,
                ModelStatus::Unknown,
                "registry model reference is missing",
            );
        }
        RegistryPresence::EndpointDeclared => {
            return reject(
                request,
                registry,
                manifest,
                MatchVerdict::Unknown,
                ModelStatus::Unknown,
                "declared endpoint is not listed by an authorized client",
            );
        }
        RegistryPresence::FileObserved { format_hint } => {
            if manifest.format == ModelFormat::Endpoint {
                return reject(
                    request,
                    registry,
                    manifest,
                    MatchVerdict::Incompatible,
                    ModelStatus::Incompatible,
                    "catalog declares endpoint but registry observes a file",
                );
            }
            if *format_hint == ModelFormat::Unknown {
                return reject(
                    request,
                    registry,
                    manifest,
                    MatchVerdict::Unknown,
                    ModelStatus::Unknown,
                    "registry has no trustworthy file format hint",
                );
            }
            if *format_hint != manifest.format {
                return reject(
                    request,
                    registry,
                    manifest,
                    MatchVerdict::Incompatible,
                    ModelStatus::Incompatible,
                    "catalog and registry file formats contradict each other",
                );
            }
        }
        RegistryPresence::EndpointListed => {
            if manifest.format != ModelFormat::Endpoint {
                return reject(
                    request,
                    registry,
                    manifest,
                    MatchVerdict::Incompatible,
                    ModelStatus::Incompatible,
                    "registry observes endpoint but catalog declares file format",
                );
            }
        }
    }
    if request.target != probe.target {
        return reject(
            request,
            registry,
            manifest,
            MatchVerdict::Unknown,
            ModelStatus::Unknown,
            "trusted hardware probe does not bind the requested host/runtime/accelerator",
        );
    }
    // Do not let the catalog forge a runtime probe, even if its evidence ID
    // looks valid. Only Registry's independently issued observation counts.
    let mut trusted_probes = Vec::new();
    match &registry.runtime {
        RegistryRuntimeEvidence::Verified {
            runtime,
            hardware,
            host_id,
            evidence_id,
        } if *runtime == request.target.runtime
            && *hardware == request.target.hardware
            && *host_id == request.target.host_id
            && !evidence_id.trim().is_empty() =>
        {
            trusted_probes.push(TrustedRuntimeProbe {
                model_id: manifest.id.clone(),
                runtime: *runtime,
                hardware: *hardware,
                evidence_id: evidence_id.clone(),
            });
        }
        RegistryRuntimeEvidence::Incompatible { reason } => {
            return reject(
                request,
                registry,
                manifest,
                MatchVerdict::Incompatible,
                ModelStatus::Incompatible,
                format!("registry rejected runtime: {reason}"),
            );
        }
        _ => {}
    }
    let artifact_verified = matches!(
        &registry.artifact,
        ArtifactEvidence::Validated { evidence_id } if !evidence_id.trim().is_empty()
    );
    let mut effective = manifest.clone();
    effective.integrity_verified &= artifact_verified;
    // Missing Registry evidence must never be upgraded by catalog metadata.
    effective.verified_runtimes.retain(|verification| {
        trusted_probes.iter().any(|trusted| {
            verification.runtime == trusted.runtime
                && verification.hardware == trusted.hardware
                && verification.evidence_id == trusted.evidence_id
        })
    });
    let ctx = CompatibilityContext {
        hardware: request.target.hardware,
        runtime: request.target.runtime,
        available_ram_mib: probe.available_ram_mib,
        available_vram_mib: probe.available_vram_mib,
        available_npu_mib: probe.available_npu_mib,
        hardware_verified: probe.hardware_verified,
        runtime_ready: probe.runtime_ready,
        consumer_adapter_verified: request.application_task.adapter_verified,
        paired_host_authorized: probe.paired_host_authorized,
        trusted_probes,
    };
    let mut result = match_model(&effective, &request.application_task.request, &ctx);
    // A Registry entry with unchecked artifact or runtime is observed, not
    // proven runnable. Surface this explicitly, even if catalog claims proof.
    if result.verdict == MatchVerdict::Candidate
        && (!artifact_verified || matches!(&registry.runtime, RegistryRuntimeEvidence::Unchecked))
    {
        result
            .reasons
            .push("Registry validation or target runtime evidence is incomplete".into());
    }
    decision(request, registry, manifest, result)
}
