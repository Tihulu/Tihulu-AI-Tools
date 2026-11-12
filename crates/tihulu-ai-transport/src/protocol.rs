//! Typed control-plane contracts. NO socket, cryptography, credential vault or GPU backend.
//! PeerEvidence MUST come from a future authenticated TLS adapter, never an HTTP body.
use std::collections::HashSet;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    InvalidIdentifier,
    InvalidConfiguration,
    PairingRequired,
    UnverifiedChannel,
    UnverifiedApplication,
    TransportUnavailable,
    IdentityMismatch,
    PermissionDenied,
    UnknownModel,
    UnknownLease,
    UnknownRequest,
    NotOwner,
    QuotaExceeded,
    RequestTooLarge,
    IdentifierConflict,
    LeaseExpired,
    ShuttingDown,
    StagingNotVerified,
    RecordCapacityReached,
    LedgerBindingRequired,
    LedgerCleanupPending,
    LedgerAckRejected,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.as_bytes().iter().all(|c| {
            c.is_ascii_alphanumeric() || matches!(*c, b'-' | b'_' | b'.' | b':')
        })
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && !value.contains("..")
}
macro_rules! opaque_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub struct $name(String);
        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, Error> {
                let value = value.into();
                if !valid_id(&value) { return Err(Error::InvalidIdentifier); }
                Ok(Self(value))
            }
            pub fn as_str(&self) -> &str { &self.0 }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
opaque_id!(HostId);
opaque_id!(ClientId);
opaque_id!(AppId);
// A particular launch of an installed TIHULU application, not the app name.
opaque_id!(AppSessionId);
// A specific Advisor/Agent/app task session, not the TaskKind modality.
opaque_id!(TaskId);
// A Registry ModelRecord.id reference, NOT a filesystem path or content digest.
opaque_id!(ModelId);
// A loaded runtime instance on one host; distinct from Registry model identity.
opaque_id!(RuntimeId);
opaque_id!(LeaseId);
opaque_id!(RequestId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkRoute {
    Loopback,
    DirectEthernet,
    LocalNetwork,
    ApprovedVpn,
    PublicInternet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelSecurity {
    /// Evidence is asserted by a future mTLS implementation (not proven by this crate).
    VerifiedMutualTls,
    Unverified,
}

/// Trusted boundary: a real transport must construct this ONLY from verified mTLS
/// session identity and pinned certificate data, never from requester-supplied JSON.
/// A future identity broker must validate a per-application credential, not only
/// the shared Pi/device mTLS certificate. These flags are mock-only assertions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppAuthentication {
    VerifiedScopedCredential,
    Unverified,
}

#[derive(Clone, Debug)]
pub struct PeerEvidence {
    pub host: HostId,
    pub client: ClientId,
    pub app: AppId,
    /// Server-attested application launch; never inferred from app display name.
    pub app_session: AppSessionId,
    /// Server-attested task session. Multiple tasks of one app are isolated.
    pub task: TaskId,
    pub app_authentication: AppAuthentication,
    pub client_cert_sha256: String,
    pub server_cert_sha256: String,
    pub security: ChannelSecurity,
    pub route: NetworkRoute,
}

#[derive(Clone, Debug)]
pub struct PairingApproval {
    /// Out-of-band user-confirmed certificate fingerprint; never a network-supplied path.
    pub client: ClientId,
    pub client_cert_sha256: String,
    pub user_approved: bool,
}

pub fn valid_sha256_hex(hash: &str) -> bool {
    hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelLocation {
    /// Registered stable model ID, resolving to a PC-owned or PC-authorized asset.
    HostStored,
    /// Device-authoritative asset staged after explicit consent and verified integrity.
    /// These flags are HOST policy attestations, NOT fields to accept from a client.
    ClientStaged {
        sha256: String,
        transfer_approved: bool,
        integrity_verified: bool,
        runtime_compatible: bool,
    },
}
impl ModelLocation {
    pub fn executable(&self) -> bool {
        match self {
            Self::HostStored => true,
            Self::ClientStaged {
                sha256, transfer_approved, integrity_verified, runtime_compatible
            } => valid_sha256_hex(sha256)
                && *transfer_approved && *integrity_verified && *runtime_compatible,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeOwnership {
    /// This host is authorized to release the model runtime context on last lease.
    HostOwned,
    /// Pre-existing Jan/Ollama/another provider: NEVER unload or terminate it.
    BorrowedExternal,
}

#[derive(Clone, Debug)]
pub struct ModelRegistration {
    pub id: ModelId,
    pub location: ModelLocation,
    pub ownership: RuntimeOwnership,
    /// Simulated attributable runtime VRAM; not a device VRAM reading.
    pub estimated_vram_mib: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskKind {
    Chat,
    Embeddings,
    Transcription,
    Translation,
    Vision,
}

#[derive(Clone, Debug)]
pub struct AppGrant {
    pub allowed_models: HashSet<ModelId>,
    pub allowed_tasks: HashSet<TaskKind>,
    pub max_leases: usize,
    pub max_nonterminal_jobs: usize,
    pub max_payload_bytes: u64,
    pub max_model_vram_mib: u64,
}
impl AppGrant {
    pub fn validate(&self) -> Result<(), Error> {
        if self.allowed_models.is_empty() || self.allowed_tasks.is_empty()
            || self.max_leases == 0 || self.max_nonterminal_jobs == 0
            || self.max_payload_bytes == 0 { return Err(Error::InvalidConfiguration); }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestSpec {
    pub id: RequestId,
    pub lease: LeaseId,
    /// Workload modality; task session identity is bound to the lease/peer.
    pub task: TaskKind,
    pub payload_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobStatus {
    Queued,
    Running,
    Cancelling,
    Cancelled,
    Succeeded,
    Failed,
}
impl JobStatus {
    pub fn terminal(self) -> bool {
        matches!(self, Self::Cancelled | Self::Succeeded | Self::Failed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseStatus {
    Active,
    Closing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostStatus {
    Accepting,
    Draining,
    Drained,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuditAction {
    Paired,
    Granted,
    Revoked,
    LeaseOpened,
    LeaseRenewed,
    LeaseClosing,
    LeaseClosed,
    ModelLoaded,
    ModelUnloaded,
    JobQueued,
    JobStarted,
    JobCancellationRequested,
    JobCancelled,
    JobFinished,
    ForceCancelled,
    HostDraining,
    HostDrained,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuditEvent {
    pub at_ms: u64,
    pub action: AuditAction,
    pub client: Option<ClientId>,
    pub app: Option<AppId>,
    pub model: Option<ModelId>,
    pub lease: Option<LeaseId>,
    pub request: Option<RequestId>,
    /// Never contains model paths, TLS credentials, task payloads or outputs.
    pub detail: &'static str,
}


/// Numeric IDs are allocated by the coordinator, not parsed from remote text.
/// These are the local bridge equivalents of core OwnerId(u64)/ResourceId(u64).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LedgerOwnerId(pub u64);
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LedgerResourceId(pub u64);

/// Explicit runtime-instance → Registry model → coordinator ResourceLedger map.
/// The registry is reference-only; it never makes an independent unload decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerRuntimeBinding {
    pub model: ModelId,
    pub runtime: RuntimeId,
    pub resource: LedgerResourceId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LifecycleAuthority {
    /// Local stand-alone mock only; synthetic unload happens in host bookkeeping.
    MockLocal,
    /// Real integration: send reference events and accept a verified ledger cleanup ack.
    /// This crate NEVER decides to unload an owned model independently in this mode.
    CoordinatorLedger,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerLeaseAction {
    Acquired,
    Released,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerLeaseEvent {
    /// Stable, monotonic per-host event number; adapter must persist dedup on retries.
    pub sequence: u64,
    pub action: LedgerLeaseAction,
    pub host: HostId,
    pub client: ClientId,
    pub app: AppId,
    pub app_session: AppSessionId,
    pub task: TaskId,
    pub model: ModelId,
    pub runtime: RuntimeId,
    pub owner: LedgerOwnerId,
    pub resource: LedgerResourceId,
    pub lease: LeaseId,
}
