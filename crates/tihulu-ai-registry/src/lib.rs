//! Permission-gated model inventory, provenance IDs and a core-ledger adapter
//! contract. No independent runtime lease authority or model process control.
//!
//! The crate never downloads, copies, opens model weights, starts inference,
//! connects to a server or manipulates an external application.

#[cfg(feature = "core-ledger")]
pub mod core_adapter;
pub mod discovery;
pub mod leases;
pub mod registry;

pub use discovery::{
    observe_local_endpoint, scan_approved_source, DirectorySource, EndpointAdapter,
    EndpointRequest, ReadApproval, ScanLimits, ScanReport,
};
pub use leases::{
    attach_task, register_initial_task, stop_app, stop_app_with_receipts, AppSessionId,
    ConsumerTask, ContextAuthority, LedgerCleanupRequest, LedgerOwnerRef, LedgerResourceKind,
    LedgerResourceRef, ModelRuntimeBinding, ResourceLedgerPort, RuntimeInstanceId, TaskLeaseReceipt,
    TaskRelease, TaskSessionId,
};
pub use registry::{
    default_model_library, ArtifactObservation, ArtifactValidation, EndpointObservation,
    EndpointState, FileFormat, ModelId, ModelOwner, ModelRecord, ModelReference, ModelRegistry,
    RuntimeVerification, SourceAdapter,
};
