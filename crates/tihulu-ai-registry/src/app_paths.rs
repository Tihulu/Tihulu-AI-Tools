//! Side-effect-free model path conventions for independently installable TIHULU apps.
//! Tracked in APP-PATH-01 (GitHub Issue #18).
//!
//! This module deliberately does not scan, move, delete, create, write or
//! canonicalize files. In particular, a path-change plan is NOT permission
//! to edit an installed application or another model owner's directory.
//!
//! An app must work without the AI Tools GUI, using its own model path and
//! optionally a persistent local, versioned manifest (or its legacy setting).

use crate::registry::default_model_library;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandidateSource {
    /// Installer convention; the directory need not exist yet.
    PortableDefault,
    /// Current path reported by the installed app itself.
    AppReported,
    /// A user-selected override (has precedence only when validated by adapter).
    UserOverride,
    /// An old known location; read-only discovery, never an automatic migration.
    LegacyReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidatePath {
    pub path: PathBuf,
    pub source: CandidateSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelLocationOwnership {
    /// The target TIHULU application owns its files and configuration.
    AppManaged,
    /// The AI Tools managed library (but not the entire home directory).
    AiToolsManaged,
    /// Jan, Ollama, HF cache, another application, or a user-maintained folder.
    BorrowedExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeMode {
    /// Repoint supported config; do not transfer data or delete originals.
    RepointInPlace,
    /// Plan a verified, copy-first migration without deleting the old source.
    CopyToManagedRoot,
    /// Moving is deferred until verified copy, rollback and explicit cleanup.
    PlanMoveWithRollback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppModelPathRecord {
    pub app_id: String,
    /// Executable/install location is informational. Moving installed binaries
    /// requires the OS installer or package manager, never a model path plan.
    pub installation_path: Option<PathBuf>,
    pub current_model_root: PathBuf,
    pub current_owner: ModelLocationOwnership,
    pub legacy_model_roots: Vec<PathBuf>,
    /// Count of authorized consumers including the current app. A physical
    /// move is forbidden when multiple clients depend on the source.
    pub authorized_consumers: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPathChangePlan {
    pub app_id: String,
    pub previous_root: PathBuf,
    pub requested_root: PathBuf,
    pub mode: ChangeMode,
    /// No operation is performed by the planner.
    pub effectful_operation_executed: bool,
    /// Backend must canonicalize both roots, refuse symlink escape, check free
    /// space/checksum and ownership before an app adapter writes config.
    pub native_verification_required: bool,
    pub preserve_original: bool,
    pub backup_and_rollback_required: bool,
    pub requires_app_adapter: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathPlanError {
    InvalidAppId,
    InvalidPath,
    UserApprovalRequired,
    OutsideApprovedDestination,
    BorrowedModelCannotMove,
    SharedModelCannotMove,
}

fn valid_app_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.contains("--")
}

fn valid_lexical_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
}

/// Stable convention for standalone installers. AI Tools does not need to be
/// installed. The convention reuses the existing default managed-library root
/// so older apps can continue reading their own unchanged paths.
pub fn app_default_model_root(home: &Path, app_id: &str) -> Result<PathBuf, PathPlanError> {
    if !valid_app_id(app_id) {
        return Err(PathPlanError::InvalidAppId);
    }
    if !valid_lexical_path(home) {
        return Err(PathPlanError::InvalidPath);
    }
    Ok(default_model_library(home).join("apps").join(app_id))
}

/// An optional common pool. Sharing weights does not imply a shared runtime
/// lease, trusted binary format, writable ownership or automatic deduplication.
pub fn app_shared_model_root(home: &Path) -> Result<PathBuf, PathPlanError> {
    if !valid_lexical_path(home) {
        return Err(PathPlanError::InvalidPath);
    }
    Ok(default_model_library(home).join("shared"))
}

/// Produce suggestions only. All candidates still require explicit native
/// consent, canonical-path and directory checks before discovery.
/// The app's active path is first; its historical paths are never overwritten.
pub fn user_visible_candidates(
    record: &AppModelPathRecord,
    convention: &Path,
    user_override: Option<&Path>,
) -> Vec<CandidatePath> {
    let mut paths = Vec::new();
    let mut push_unique = |path: &Path, source| {
        if valid_lexical_path(path) && !paths.iter().any(|p: &CandidatePath| p.path == path) {
            paths.push(CandidatePath {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if let Some(path) = user_override {
        push_unique(path, CandidateSource::UserOverride);
    }
    push_unique(&record.current_model_root, CandidateSource::AppReported);
    push_unique(convention, CandidateSource::PortableDefault);
    for legacy in &record.legacy_model_roots {
        push_unique(legacy, CandidateSource::LegacyReadOnly);
    }
    paths
}

/// Planning only. This validates *lexical* user scope and safe ownership
/// invariants; an authenticated native adapter MUST still perform realpath,
/// symlink, install lock, grants, space, integrity and app-config checks.
/// It must not touch the filesystem on planner success alone.
pub fn plan_app_model_path_change(
    record: &AppModelPathRecord,
    requested_root: &Path,
    approved_destination_scope: &Path,
    mode: ChangeMode,
    user_approved: bool,
) -> Result<ModelPathChangePlan, PathPlanError> {
    if !valid_app_id(&record.app_id) {
        return Err(PathPlanError::InvalidAppId);
    }
    if !valid_lexical_path(&record.current_model_root)
        || !valid_lexical_path(requested_root)
        || !valid_lexical_path(approved_destination_scope)
    {
        return Err(PathPlanError::InvalidPath);
    }
    if !user_approved {
        return Err(PathPlanError::UserApprovalRequired);
    }
    if !requested_root.starts_with(approved_destination_scope) {
        return Err(PathPlanError::OutsideApprovedDestination);
    }
    if mode != ChangeMode::RepointInPlace
        && record.current_owner == ModelLocationOwnership::BorrowedExternal
    {
        return Err(PathPlanError::BorrowedModelCannotMove);
    }
    if mode == ChangeMode::PlanMoveWithRollback && record.authorized_consumers > 1 {
        return Err(PathPlanError::SharedModelCannotMove);
    }
    Ok(ModelPathChangePlan {
        app_id: record.app_id.clone(),
        previous_root: record.current_model_root.clone(),
        requested_root: requested_root.to_path_buf(),
        mode,
        effectful_operation_executed: false,
        native_verification_required: true,
        preserve_original: true,
        backup_and_rollback_required: true,
        requires_app_adapter: true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> AppModelPathRecord {
        AppModelPathRecord {
            app_id: "tihulu-media-center".into(),
            installation_path: Some(PathBuf::from("installed/apps/media")),
            current_model_root: PathBuf::from("user/old-media-models"),
            current_owner: ModelLocationOwnership::AppManaged,
            legacy_model_roots: vec![
                PathBuf::from("user/old-media-models"),
                PathBuf::from("user/MediaCenter/models"),
            ],
            authorized_consumers: 1,
        }
    }

    #[test]
    fn independent_installer_uses_consistent_model_convention() {
        let result = app_default_model_root(Path::new("user"), "tihulu-media-center").unwrap();
        assert_eq!(
            result,
            PathBuf::from("user/TIHULU AI Tools/Models/apps/tihulu-media-center")
        );
        let shared = app_shared_model_root(Path::new("user")).unwrap();
        assert_eq!(shared, PathBuf::from("user/TIHULU AI Tools/Models/shared"));
    }

    #[test]
    fn invalid_app_id_never_becomes_a_model_directory() {
        for id in ["", "../media", "a/b", "UpperCase", "a..b", "x--y", "-abc"] {
            assert_eq!(
                app_default_model_root(Path::new("user"), id),
                Err(PathPlanError::InvalidAppId)
            );
        }
    }

    #[test]
    fn candidates_prioritize_user_override_and_preserve_legacy() {
        let record = fixture();
        let candidates = user_visible_candidates(
            &record,
            Path::new("user/standard"),
            Some(Path::new("user/custom")),
        );
        assert_eq!(candidates.len(), 4);
        assert_eq!(candidates[0].source, CandidateSource::UserOverride);
        assert_eq!(candidates[1].source, CandidateSource::AppReported);
        assert_eq!(candidates[2].source, CandidateSource::PortableDefault);
        assert_eq!(candidates[3].source, CandidateSource::LegacyReadOnly);
        assert_eq!(candidates[3].path, PathBuf::from("user/MediaCenter/models"));
    }

    #[test]
    fn no_approval_means_no_path_change_plan() {
        let record = fixture();
        assert_eq!(
            plan_app_model_path_change(
                &record,
                Path::new("user/new"),
                Path::new("user"),
                ChangeMode::CopyToManagedRoot,
                false
            ),
            Err(PathPlanError::UserApprovalRequired)
        );
    }

    #[test]
    fn traversal_and_scope_escape_are_rejected() {
        let record = fixture();
        assert_eq!(
            plan_app_model_path_change(
                &record,
                Path::new("user/../secrets"),
                Path::new("user"),
                ChangeMode::RepointInPlace,
                true
            ),
            Err(PathPlanError::InvalidPath)
        );
        assert_eq!(
            plan_app_model_path_change(
                &record,
                Path::new("foreign/models"),
                Path::new("user"),
                ChangeMode::RepointInPlace,
                true
            ),
            Err(PathPlanError::OutsideApprovedDestination)
        );
    }

    #[test]
    fn borrowed_external_files_are_never_planned_for_copy_or_move() {
        let mut record = fixture();
        record.current_owner = ModelLocationOwnership::BorrowedExternal;
        for mode in [
            ChangeMode::CopyToManagedRoot,
            ChangeMode::PlanMoveWithRollback,
        ] {
            assert_eq!(
                plan_app_model_path_change(
                    &record,
                    Path::new("user/models2"),
                    Path::new("user"),
                    mode,
                    true
                ),
                Err(PathPlanError::BorrowedModelCannotMove)
            );
        }
        assert!(plan_app_model_path_change(
            &record,
            Path::new("user/external"),
            Path::new("user"),
            ChangeMode::RepointInPlace,
            true
        )
        .is_ok());
    }

    #[test]
    fn shared_model_cannot_be_moved_away_from_other_clients() {
        let mut record = fixture();
        record.authorized_consumers = 2;
        assert_eq!(
            plan_app_model_path_change(
                &record,
                Path::new("user/new"),
                Path::new("user"),
                ChangeMode::PlanMoveWithRollback,
                true
            ),
            Err(PathPlanError::SharedModelCannotMove)
        );
        assert!(plan_app_model_path_change(
            &record,
            Path::new("user/new"),
            Path::new("user"),
            ChangeMode::CopyToManagedRoot,
            true
        )
        .is_ok());
    }

    #[test]
    fn approved_plan_never_moves_a_file_or_rewrites_installed_app() {
        let record = fixture();
        let result = plan_app_model_path_change(
            &record,
            Path::new("user/new"),
            Path::new("user"),
            ChangeMode::CopyToManagedRoot,
            true,
        )
        .unwrap();
        assert!(!result.effectful_operation_executed);
        assert!(result.native_verification_required);
        assert!(result.preserve_original);
        assert!(result.backup_and_rollback_required);
        assert!(result.requires_app_adapter);
        assert_eq!(
            record.installation_path,
            Some(PathBuf::from("installed/apps/media"))
        );
        assert_eq!(
            record.current_model_root,
            PathBuf::from("user/old-media-models")
        );
    }

    #[test]
    fn rejected_legacy_candidates_do_not_expand_read_scope() {
        let mut record = fixture();
        record
            .legacy_model_roots
            .push(PathBuf::from("user/../../escape"));
        record.legacy_model_roots.push(PathBuf::new());
        let candidates = user_visible_candidates(&record, Path::new("user/new"), None);
        assert_eq!(candidates.len(), 3);
    }
}
