//! TIHULU AI Tools Agent 04: offline-only remote-compute lifecycle contracts.
//!
//! No listener, cryptographic verifier, model staging service or GPU executor exists.
//! PeerEvidence remains a test-only trust assertion, not proof of real TLS.
//! Host cleanup decisions come exclusively from core ResourceLedger.
pub mod capabilities;
pub mod host;
pub mod ledger_bridge;
pub mod mock_transport;
pub mod pairing;
pub mod protocol;

pub use capabilities::{
    AcceleratorBackend, CapabilityError, CapabilityEvidence, GpuCapabilityProbe,
    GpuCapabilitySnapshot, GpuDeviceHint, MockGpuCapabilityProbe,
};
pub use host::MockComputeHost;
pub use ledger_bridge::{
    AuthoritativeMockLedgerBridge, BridgeError, CleanupTicket, MockCleanupFailure,
    MockCleanupObservation, MockCleanupReport,
};
pub use mock_transport::{ComputeTransport, MockTransport, WireCommand, WireResponse};
pub use pairing::{
    MockOutOfBandApproval, PairingBroker, PairingError, PairingOffer,
};
pub use protocol::*;
