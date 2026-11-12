//! A fully in-memory transport for deterministic cross-client tests.
//! Not a network adapter. Never construct PeerEvidence from untrusted wire input.
use crate::{host::MockComputeHost, protocol::*};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireCommand {
    OpenLease { lease: LeaseId, model: ModelId },
    Heartbeat { lease: LeaseId },
    Submit { request: RequestSpec },
    Cancel { request: RequestId },
    CloseLease { lease: LeaseId },
    CloseApp,
    CloseTask,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireResponse {
    Accepted,
    Job(JobStatus),
}
pub trait ComputeTransport {
    fn exchange(
        &mut self,
        peer: &PeerEvidence,
        command: WireCommand,
        now_ms: u64,
    ) -> Result<WireResponse, Error>;
}

/// Simulates lost links while host-side TTL and graceful drain keep running.
/// Does not launch a daemon, bind a port, connect to the Internet or use a GPU.
pub struct MockTransport {
    pub host: MockComputeHost,
    connected: bool,
    disconnected_clients: HashSet<ClientId>,
}
impl MockTransport {
    pub fn new(host: MockComputeHost) -> Self {
        Self {
            host,
            connected: true,
            disconnected_clients: HashSet::new(),
        }
    }
    /// Simulate a whole transport outage (e.g., host's local interface down).
    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }
    /// Simulate one Pi losing its connection while other Pi clients remain online.
    pub fn set_client_connected(&mut self, client: ClientId, connected: bool) {
        if connected {
            self.disconnected_clients.remove(&client);
        } else {
            self.disconnected_clients.insert(client);
        }
    }
}
impl ComputeTransport for MockTransport {
    fn exchange(
        &mut self,
        peer: &PeerEvidence,
        command: WireCommand,
        now_ms: u64,
    ) -> Result<WireResponse, Error> {
        if !self.connected || self.disconnected_clients.contains(&peer.client) {
            return Err(Error::TransportUnavailable);
        }
        match command {
            WireCommand::OpenLease { lease, model } => {
                self.host.open_lease(peer, lease, model, now_ms)?;
                Ok(WireResponse::Accepted)
            }
            WireCommand::Heartbeat { lease } => {
                self.host.heartbeat(peer, &lease, now_ms)?;
                Ok(WireResponse::Accepted)
            }
            WireCommand::Submit { request } => self
                .host
                .submit(peer, request, now_ms)
                .map(WireResponse::Job),
            WireCommand::Cancel { request } => self
                .host
                .cancel_request(peer, &request, now_ms)
                .map(WireResponse::Job),
            WireCommand::CloseLease { lease } => {
                self.host.close_lease(peer, &lease, now_ms)?;
                Ok(WireResponse::Accepted)
            }
            WireCommand::CloseApp => {
                self.host.close_app(peer, now_ms)?;
                Ok(WireResponse::Accepted)
            }
            WireCommand::CloseTask => {
                self.host.close_task(peer, now_ms)?;
                Ok(WireResponse::Accepted)
            }
        }
    }
}
