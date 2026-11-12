//! Offline-first Model Market contracts.
//!
//! This crate performs only deterministic in-memory validation and ranking.
//! It does not scan disks, download weights, launch inference or access a GPU.

pub mod advisor;
pub mod lifecycle;
pub mod manifest;
pub mod matching;
pub mod registry_contract;

pub use advisor::{
    validate_advisor_proposal, AdvisorPolicy, AdvisorProposal, AdvisorValidation,
    Citation, ProposalError,
};
pub use lifecycle::{
    CleanupAction, MarketPreviewSupervisor, PreviewResource, SupervisorError,
};
pub use manifest::{
    HardwareTarget, LanguagePair, License, MemoryRequirements, ModelFormat,
    ModelManifest, ModelSource, ModelStatus, Modality, Runtime, RuntimeVerification,
    TaskKind,
};
pub use matching::{
    match_model, rank_models, CompatibilityContext, MatchResult, MatchVerdict,
    TaskRequest, TrustedRuntimeProbe,
};

// Phase 2 Market-to-Registry preflight types; no runtime/lease authority.
pub use registry_contract::{
    match_registry_model, ApplicationId, ApplicationTask, ArtifactEvidence,
    CatalogIdentityBinding, ContractError, HostId, RegistryMatchDecision,
    RegistryMatchRequest, RegistryModelId, RegistryModelSnapshot, RegistryOwner,
    RegistryPresence, RegistryRuntimeEvidence, RuntimeTarget, TrustedHardwareProbe,
};
