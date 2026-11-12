//! Platform-neutral primitives for TIHULU AI Tools.
//!
//! No network listener, model download, inference runtime or application writes
//! are activated by importing this crate.

pub mod discovery;
pub mod hardware;
pub mod resource_lifecycle;
pub mod routing;

pub use discovery::{scan_directory, ModelCandidate, ModelFormat, ScanPolicy, ScanReport};
pub use hardware::{DetectedHost, HardwarePreset};
pub use routing::{choose_route, ComputePreference, Route, RuntimeReadiness};

// Pure ownership/cleanup ledger; backends still implement actual teardown.
pub use resource_lifecycle::{
    CleanupRequest, LifecycleError, OwnerId, OwnerPhase, Provenance, ResourceId, ResourceKind,
    ResourceLedger, ResourcePhase,
};
