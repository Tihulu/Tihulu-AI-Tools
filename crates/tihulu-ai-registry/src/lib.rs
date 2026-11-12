//! Permission-gated registry and pure runtime-lease bookkeeping.
//!
//! The crate never downloads, copies, opens model weights, starts inference,
//! connects to a server or manipulates an external application. Endpoint
//! observations must be supplied by a separately authorized client.

pub mod discovery;
pub mod leases;
pub mod registry;

pub use discovery::{
    observe_local_endpoint, scan_approved_source, DirectorySource, EndpointAdapter,
    EndpointRequest, ReadApproval, ScanLimits, ScanReport,
};
pub use leases::{
    Consumer, LeaseError, LeaseRegistry, LeaseToken, ReleaseAction, RuntimeOwner,
};
pub use registry::{
    default_model_library, ArtifactObservation, ArtifactValidation, EndpointObservation,
    EndpointState, FileFormat, ModelOwner, ModelRecord, ModelReference, ModelRegistry,
    RuntimeVerification, SourceAdapter,
};
