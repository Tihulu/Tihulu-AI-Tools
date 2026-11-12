//! Platform-neutral primitives for TIHULU AI Tools.
//!
//! No network listener, model download, inference runtime or application writes
//! are activated by importing this crate.

pub mod discovery;
pub mod hardware;
pub mod routing;

pub use discovery::{scan_directory, ModelCandidate, ModelFormat, ScanPolicy, ScanReport};
pub use hardware::{DetectedHost, HardwarePreset};
pub use routing::{choose_route, ComputePreference, Route, RuntimeReadiness};
