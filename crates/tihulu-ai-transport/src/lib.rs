//! TIHULU AI Tools Agent 04: offline-only remote-compute protocol foundation.
//!
//! No listener, cryptographic verifier, model staging service or GPU executor exists.
//! PeerEvidence is a contract for a *future* TLS trust boundary, not proof of TLS.
pub mod host;
pub mod mock_transport;
pub mod protocol;

pub use host::MockComputeHost;
pub use mock_transport::{ComputeTransport, MockTransport, WireCommand, WireResponse};
pub use protocol::*;
