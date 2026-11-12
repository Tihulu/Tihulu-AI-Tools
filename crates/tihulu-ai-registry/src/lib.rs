//! Permission-gated model inventory, provenance IDs and a core-ledger adapter
//! contract. No independent runtime lease authority or model process control.
//!
//! The crate never downloads, copies, opens model weights, starts inference,
//! connects to a server or manipulates an external application.

pub mod app_paths;
pub mod consent_discovery;
#[cfg(feature = "core-ledger")]
pub mod core_adapter;
pub mod discovery;
pub mod leases;
pub mod registry;

pub use consent_discovery::{
    discover_approved_models, ApprovedModelScope, ConsentDecision, DiscoveredModel,
    DiscoveryBounds, DiscoveryError, DiscoveryInventory, DiscoveryReadOnlyPort, DiscoveryWarning,
    DiscoveryWarningKind, FileEvidence, ModelSource,
};
pub use discovery::{
    observe_local_endpoint, scan_approved_source, DirectorySource, EndpointAdapter,
    EndpointRequest, ReadApproval, ScanLimits, ScanReport,
};
pub use leases::{
    attach_task, register_initial_task, stop_app, stop_app_with_receipts, AppSessionId,
    ConsumerTask, ContextAuthority, LedgerCleanupRequest, LedgerOwnerRef, LedgerResourceKind,
    LedgerResourceRef, ModelRuntimeBinding, ResourceLedgerPort, RuntimeInstanceId,
    TaskLeaseReceipt, TaskRelease, TaskSessionId,
};
pub use registry::{
    default_model_library, ArtifactObservation, ArtifactValidation, EndpointObservation,
    EndpointState, FileFormat, ModelId, ModelOwner, ModelRecord, ModelReference, ModelRegistry,
    RuntimeVerification, SourceAdapter,
};

// Pure, side-effect-free app model-path convention and migration planning.
// A native adapter MUST validate actual filesystem paths, ownership, consent,
// config capabilities, and rollback before acting on any returned plan.
pub use app_paths::{
    app_default_model_root, app_shared_model_root, plan_app_model_path_change,
    user_visible_candidates, AppModelPathRecord, CandidatePath, CandidateSource, ChangeMode,
    ModelLocationOwnership, ModelPathChangePlan, PathPlanError,
};
