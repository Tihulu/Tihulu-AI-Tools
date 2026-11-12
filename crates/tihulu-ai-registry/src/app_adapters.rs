//! AI-07 / APP-PATH-01: read-only standalone application path adapters.
//!
//! This module has no filesystem, network, process, installer, or config I/O.
//! Native adapters must verify package/signature identity and supply a scoped,
//! no-follow path probe. This module does NOT implement those native adapters.
//! No shared Registry exports are changed until coordinator acceptance.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

pub const MAX_LEGACY_ROOTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedApp {
    MediaCenter,
    TextCenter,
    SatelliteCenter,
    FlightRadar,
    RfGeolocation,
    SurvivalKit,
}

impl SupportedApp {
    pub const ALL: [Self; 6] = [
        Self::MediaCenter,
        Self::TextCenter,
        Self::SatelliteCenter,
        Self::FlightRadar,
        Self::RfGeolocation,
        Self::SurvivalKit,
    ];

    pub const fn stable_id(self) -> &'static str {
        match self {
            Self::MediaCenter => "tihulu-media-center",
            Self::TextCenter => "tihulu-text-center",
            Self::SatelliteCenter => "tihulu-satellite-center",
            Self::FlightRadar => "tihulu-flight-radar",
            Self::RfGeolocation => "tihulu-rf-geolocation",
            Self::SurvivalKit => "tihulu-survival-kit",
        }
    }

    pub fn parse(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|app| app.stable_id() == id)
    }
}

/// The trusted native adapter must VERIFY this evidence before emitting it.
/// An enum value alone is not cryptographic proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentityEvidence {
    OsPackageDatabase,
    VerifiedSignedManifest,
    VerifiedPortableManifest,
    UnverifiedClaim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMethod {
    OsManaged,
    SignedAppBundle,
    Portable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelOwner {
    AppManaged,
    UserManaged,
    BorrowedExternal,
}

/// Paths are app-declared, not discovered through generic disk scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppDeclaration {
    pub stable_app_id: String,
    pub version: String,
    pub identity_evidence: IdentityEvidence,
    pub install_method: InstallMethod,
    pub executable_path: PathBuf,
    pub installation_path: PathBuf,
    pub config_root: Option<PathBuf>,
    pub user_model_override: Option<PathBuf>,
    pub active_model_root: Option<PathBuf>,
    pub standalone_default_root: Option<PathBuf>,
    pub legacy_model_roots: Vec<PathBuf>,
    pub model_owner: ModelOwner,
}

/// Only coordinator-allowlisted implementations may provide attestation.
/// Looking up an executable name, PATH or unverified config is insufficient.
pub trait TrustedAppAdapter {
    fn expected_app(&self) -> SupportedApp;
    fn read_attested_declaration(&self) -> Result<Option<AppDeclaration>, AdapterError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterError {
    Unavailable,
    EvidenceInvalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopePurpose {
    ConfigMetadata,
    ModelMetadata,
}

/// An approval reference must come from the native user-consent broker.
/// Read grants never imply config-write, model-copy, or installer permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadGrant {
    pub app: SupportedApp,
    pub purpose: ScopePurpose,
    pub root: PathBuf,
    pub approval_reference: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsOwner {
    CurrentUser,
    AnotherUser,
    Unknown,
}

/// The path probe must canonicalize with no-follow checks and verify every
/// symlink component. It must never inspect file contents or model payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathEvidence {
    pub canonical_path: PathBuf,
    pub kind: NodeKind,
    pub owner: OsOwner,
    pub readable: bool,
    pub symlink_chain_verified: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeError {
    Missing,
    PermissionDenied,
    Unverifiable,
}

/// Interface only. No implementation that accesses real disks is provided.
pub trait ReadOnlyPathProbe {
    fn inspect_metadata(&self, path: &Path) -> Result<PathEvidence, ProbeError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateRole {
    UserOverride,
    AppReportedActive,
    StandaloneConvention,
    LegacyReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedDirectory {
    pub declared_path: PathBuf,
    pub canonical_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDirectory {
    pub directory: VerifiedDirectory,
    pub role: CandidateRole,
    pub owner: ModelOwner,
    /// Does not imply a model was found, verified, or proven runnable.
    pub metadata_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallationStatus {
    NotReported,
    IdentityAttested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppInventory {
    pub app: SupportedApp,
    pub installation_status: InstallationStatus,
    pub executable_path: Option<PathBuf>,
    pub installation_path: Option<PathBuf>,
    pub install_method: Option<InstallMethod>,
    pub version: Option<String>,
    pub config_root: Option<VerifiedDirectory>,
    pub unresolved_declared_override: bool,
    pub model_directories: Vec<ModelDirectory>,
    pub suppressed_unapproved_or_unverifiable: usize,
    pub disk_scan_performed: bool,
    pub changes_applied: bool,
}

impl AppInventory {
    pub fn effective_model_root(&self) -> Option<&ModelDirectory> {
        if self.unresolved_declared_override {
            return None;
        }
        self.model_directories.iter().find(|candidate| {
            matches!(
                candidate.role,
                CandidateRole::UserOverride | CandidateRole::AppReportedActive
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryError {
    AdapterUnavailable,
    AttestationMismatch,
    UnverifiedIdentity,
    InvalidInstallRecord,
    ExcessiveDeclaredPaths,
}

fn valid_absolute(path: &Path) -> bool {
    path.is_absolute()
        && !path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
}

fn safe_scope_root(path: &Path) -> bool {
    valid_absolute(path)
        && path
            .components()
            .any(|component| matches!(component, Component::Normal(_)))
}

fn inspected_directory(probe: &dyn ReadOnlyPathProbe, path: &Path) -> Option<PathEvidence> {
    if !valid_absolute(path) {
        return None;
    }
    let evidence = probe.inspect_metadata(path).ok()?;
    if !safe_scope_root(&evidence.canonical_path)
        || evidence.kind != NodeKind::Directory
        || evidence.owner != OsOwner::CurrentUser
        || !evidence.readable
        || !evidence.symlink_chain_verified
    {
        return None;
    }
    Some(evidence)
}

fn authorized_directory(
    probe: &dyn ReadOnlyPathProbe,
    grants: &[ReadGrant],
    app: SupportedApp,
    purpose: ScopePurpose,
    requested: &Path,
) -> Option<VerifiedDirectory> {
    if !valid_absolute(requested) {
        return None;
    }
    for grant in grants {
        if grant.app != app
            || grant.purpose != purpose
            || grant.approval_reference.trim().is_empty()
            || !safe_scope_root(&grant.root)
            || !requested.starts_with(&grant.root)
        {
            continue;
        }
        let Some(scope) = inspected_directory(probe, &grant.root) else {
            continue;
        };
        // Grant the real external SSD mount, never a redirecting grant root.
        if scope.canonical_path != grant.root {
            continue;
        }
        let Some(target) = inspected_directory(probe, requested) else {
            continue;
        };
        if target.canonical_path.starts_with(&scope.canonical_path) {
            return Some(VerifiedDirectory {
                declared_path: requested.to_path_buf(),
                canonical_path: target.canonical_path,
            });
        }
    }
    None
}

/// Returns ONLY approved, metadata-verified roots, without any filesystem
/// traversal or app launch. No AI Tools dependency is introduced into apps.
pub fn inventory_from_adapter(
    adapter: &dyn TrustedAppAdapter,
    probe: &dyn ReadOnlyPathProbe,
    grants: &[ReadGrant],
) -> Result<AppInventory, InventoryError> {
    let app = adapter.expected_app();
    let mut out = AppInventory {
        app,
        installation_status: InstallationStatus::NotReported,
        executable_path: None,
        installation_path: None,
        install_method: None,
        version: None,
        config_root: None,
        unresolved_declared_override: false,
        model_directories: Vec::new(),
        suppressed_unapproved_or_unverifiable: 0,
        disk_scan_performed: false,
        changes_applied: false,
    };
    let Some(declaration) = adapter
        .read_attested_declaration()
        .map_err(|_| InventoryError::AdapterUnavailable)?
    else {
        return Ok(out);
    };
    if SupportedApp::parse(&declaration.stable_app_id) != Some(app) {
        return Err(InventoryError::AttestationMismatch);
    }
    if declaration.identity_evidence == IdentityEvidence::UnverifiedClaim {
        return Err(InventoryError::UnverifiedIdentity);
    }
    if !valid_absolute(&declaration.executable_path)
        || !valid_absolute(&declaration.installation_path)
        || declaration.version.is_empty()
        || declaration.version.len() > 128
    {
        return Err(InventoryError::InvalidInstallRecord);
    }
    if declaration.legacy_model_roots.len() > MAX_LEGACY_ROOTS {
        return Err(InventoryError::ExcessiveDeclaredPaths);
    }

    out.installation_status = InstallationStatus::IdentityAttested;
    out.executable_path = Some(declaration.executable_path);
    out.installation_path = Some(declaration.installation_path);
    out.install_method = Some(declaration.install_method);
    out.version = Some(declaration.version);

    if let Some(config) = declaration.config_root {
        out.config_root =
            authorized_directory(probe, grants, app, ScopePurpose::ConfigMetadata, &config);
        if out.config_root.is_none() {
            out.suppressed_unapproved_or_unverifiable += 1;
        }
    }

    let mut seen = BTreeSet::new();
    let declared = [
        (declaration.user_model_override, CandidateRole::UserOverride),
        (
            declaration.active_model_root,
            CandidateRole::AppReportedActive,
        ),
        (
            declaration.standalone_default_root,
            CandidateRole::StandaloneConvention,
        ),
    ];
    for (path, role) in declared.into_iter().chain(
        declaration
            .legacy_model_roots
            .into_iter()
            .map(|path| (Some(path), CandidateRole::LegacyReadOnly)),
    ) {
        let Some(path) = path else { continue };
        if !seen.insert(path.clone()) {
            continue;
        }
        match authorized_directory(probe, grants, app, ScopePurpose::ModelMetadata, &path) {
            Some(directory) => out.model_directories.push(ModelDirectory {
                directory,
                role,
                owner: match role {
                    CandidateRole::LegacyReadOnly => ModelOwner::BorrowedExternal,
                    CandidateRole::StandaloneConvention => ModelOwner::UserManaged,
                    _ => declaration.model_owner,
                },
                metadata_only: true,
            }),
            None => {
                if role == CandidateRole::UserOverride {
                    out.unresolved_declared_override = true;
                }
                out.suppressed_unapproved_or_unverifiable += 1;
            }
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathChangeKind {
    RepointOnly,
    StagedCopyPreview,
    StagedMovePreview,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathChangePreview {
    pub app: SupportedApp,
    pub current_root: VerifiedDirectory,
    pub requested_root: VerifiedDirectory,
    pub kind: PathChangeKind,
    pub install_directory_untouched: bool,
    pub source_preserved: bool,
    pub backup_config_required: bool,
    pub atomic_config_update_required: bool,
    pub idle_and_ledger_check_required: bool,
    pub staged_copy_and_digest_check_required: bool,
    pub runtime_smoke_test_required: bool,
    pub rollback_to: VerifiedDirectory,
    pub separate_source_cleanup_approval_required: bool,
    pub operation_executed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewError {
    ExplicitConsentRequired,
    NotAttested,
    MissingApprovedConfigOrModelRoot,
    DestinationUnapprovedOrUnverifiable,
    IdenticalRoot,
    BorrowedSourceCannotTransfer,
    SharedSourceCannotMove,
}

/// Planning only: does not authorize or perform a write, copy, move or delete.
/// Any future apply must re-check grants, ResourceLedger/idle state, disk space,
/// checksum, config backup, atomic update, runtime smoke test and rollback.
pub fn preview_model_root_change(
    inventory: &AppInventory,
    probe: &dyn ReadOnlyPathProbe,
    grants: &[ReadGrant],
    requested_root: &Path,
    kind: PathChangeKind,
    approved_by_user: bool,
    authorized_consumers: usize,
) -> Result<PathChangePreview, PreviewError> {
    if !approved_by_user {
        return Err(PreviewError::ExplicitConsentRequired);
    }
    if inventory.installation_status != InstallationStatus::IdentityAttested {
        return Err(PreviewError::NotAttested);
    }
    if inventory.config_root.is_none() {
        return Err(PreviewError::MissingApprovedConfigOrModelRoot);
    }
    let current = inventory
        .effective_model_root()
        .ok_or(PreviewError::MissingApprovedConfigOrModelRoot)?;
    if current.owner == ModelOwner::BorrowedExternal && kind != PathChangeKind::RepointOnly {
        return Err(PreviewError::BorrowedSourceCannotTransfer);
    }
    if kind == PathChangeKind::StagedMovePreview && authorized_consumers != 1 {
        return Err(PreviewError::SharedSourceCannotMove);
    }
    let requested = authorized_directory(
        probe,
        grants,
        inventory.app,
        ScopePurpose::ModelMetadata,
        requested_root,
    )
    .ok_or(PreviewError::DestinationUnapprovedOrUnverifiable)?;
    if requested.canonical_path == current.directory.canonical_path {
        return Err(PreviewError::IdenticalRoot);
    }
    Ok(PathChangePreview {
        app: inventory.app,
        current_root: current.directory.clone(),
        requested_root: requested,
        kind,
        install_directory_untouched: true,
        source_preserved: true,
        backup_config_required: true,
        atomic_config_update_required: true,
        idle_and_ledger_check_required: true,
        staged_copy_and_digest_check_required: kind != PathChangeKind::RepointOnly,
        runtime_smoke_test_required: true,
        rollback_to: current.directory.clone(),
        separate_source_cleanup_approval_required: true,
        operation_executed: false,
    })
}
