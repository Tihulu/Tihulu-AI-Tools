//! Offline-first Model Market contracts.
//!
//! This crate performs only deterministic in-memory validation and ranking.
//! It does not scan disks, download weights, launch inference or access a GPU.

pub mod advisor;
pub mod lifecycle;
pub mod manifest;
pub mod matching;

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
    TaskRequest,
};
