//! Synthetic, non-effectful AI-07 APP-PATH-01 contract tests.
//! This imports the new module without editing common module exports.
#![allow(dead_code)]

#[path = "../src/app_adapters.rs"]
mod app_adapters;

use app_adapters::*;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Default)]
struct FakeProbe {
    paths: BTreeMap<PathBuf, Result<PathEvidence, ProbeError>>,
    reads: Cell<usize>,
}

impl FakeProbe {
    fn directory(&mut self, path: &str) {
        self.paths.insert(
            path.into(),
            Ok(PathEvidence {
                canonical_path: path.into(),
                kind: NodeKind::Directory,
                owner: OsOwner::CurrentUser,
                readable: true,
                symlink_chain_verified: true,
            }),
        );
    }

    fn redirect(&mut self, from: &str, to: &str) {
        self.directory(from);
        self.paths
            .get_mut(Path::new(from))
            .unwrap()
            .as_mut()
            .unwrap()
            .canonical_path = to.into();
    }

    fn unreadable(&mut self, path: &str) {
        self.directory(path);
        self.paths
            .get_mut(Path::new(path))
            .unwrap()
            .as_mut()
            .unwrap()
            .readable = false;
    }

    fn foreign_owner(&mut self, path: &str) {
        self.directory(path);
        self.paths
            .get_mut(Path::new(path))
            .unwrap()
            .as_mut()
            .unwrap()
            .owner = OsOwner::AnotherUser;
    }
}

impl ReadOnlyPathProbe for FakeProbe {
    fn inspect_metadata(&self, path: &Path) -> Result<PathEvidence, ProbeError> {
        self.reads.set(self.reads.get() + 1);
        self.paths
            .get(path)
            .cloned()
            .unwrap_or(Err(ProbeError::Missing))
    }
}

struct FakeAdapter {
    app: SupportedApp,
    record: Option<AppDeclaration>,
}

impl TrustedAppAdapter for FakeAdapter {
    fn expected_app(&self) -> SupportedApp {
        self.app
    }

    fn read_attested_declaration(&self) -> Result<Option<AppDeclaration>, AdapterError> {
        Ok(self.record.clone())
    }
}

fn declaration(app: SupportedApp) -> AppDeclaration {
    AppDeclaration {
        stable_app_id: app.stable_id().into(),
        version: "1.0.0".into(),
        identity_evidence: IdentityEvidence::OsPackageDatabase,
        install_method: InstallMethod::OsManaged,
        executable_path: "/opt/tihulu/bin/app".into(),
        installation_path: "/opt/tihulu".into(),
        config_root: Some("/home/u/.config/tihulu".into()),
        user_model_override: None,
        active_model_root: Some("/home/u/models/active".into()),
        standalone_default_root: Some("/home/u/models/apps/tihulu".into()),
        legacy_model_roots: vec!["/home/u/models/legacy".into()],
        model_owner: ModelOwner::AppManaged,
    }
}

fn adapter(app: SupportedApp) -> FakeAdapter {
    FakeAdapter {
        app,
        record: Some(declaration(app)),
    }
}

fn grant(app: SupportedApp, purpose: ScopePurpose, root: &str) -> ReadGrant {
    ReadGrant {
        app,
        purpose,
        root: root.into(),
        approval_reference: "synthetic-approval".into(),
    }
}

fn grants(app: SupportedApp) -> Vec<ReadGrant> {
    vec![
        grant(app, ScopePurpose::ConfigMetadata, "/home/u/.config/tihulu"),
        grant(app, ScopePurpose::ModelMetadata, "/home/u/models"),
    ]
}

fn base_probe() -> FakeProbe {
    let mut probe = FakeProbe::default();
    for path in [
        "/home/u/.config/tihulu",
        "/home/u/models",
        "/home/u/models/active",
        "/home/u/models/apps/tihulu",
        "/home/u/models/legacy",
        "/home/u/models/target",
    ] {
        probe.directory(path);
    }
    probe
}

fn inventoried(app: SupportedApp) -> (AppInventory, FakeProbe, Vec<ReadGrant>) {
    let probe = base_probe();
    let approval = grants(app);
    let inventory = inventory_from_adapter(&adapter(app), &probe, &approval).unwrap();
    (inventory, probe, approval)
}

#[test]
fn only_six_supported_app_ids_are_accepted() {
    assert_eq!(SupportedApp::ALL.len(), 6);
    for app in SupportedApp::ALL {
        assert_eq!(SupportedApp::parse(app.stable_id()), Some(app));
        let (inventory, _, _) = inventoried(app);
        assert_eq!(inventory.app, app);
        assert_eq!(
            inventory.installation_status,
            InstallationStatus::IdentityAttested
        );
    }
    assert_eq!(SupportedApp::parse("jan"), None);
    assert_eq!(SupportedApp::parse("../media"), None);
}

#[test]
fn absent_app_never_probes_user_files() {
    let probe = FakeProbe::default();
    let missing = FakeAdapter {
        app: SupportedApp::SatelliteCenter,
        record: None,
    };
    let inventory = inventory_from_adapter(&missing, &probe, &[]).unwrap();
    assert_eq!(
        inventory.installation_status,
        InstallationStatus::NotReported
    );
    assert_eq!(inventory.executable_path, None);
    assert!(inventory.model_directories.is_empty());
    assert_eq!(probe.reads.get(), 0);
    assert!(!inventory.disk_scan_performed);
}

#[test]
fn mismatched_and_unverified_claims_are_rejected() {
    let mut fake = adapter(SupportedApp::MediaCenter);
    fake.record.as_mut().unwrap().stable_app_id = "tihulu-text-center".into();
    assert_eq!(
        inventory_from_adapter(&fake, &base_probe(), &[]),
        Err(InventoryError::AttestationMismatch)
    );
    fake.record.as_mut().unwrap().stable_app_id = "tihulu-media-center".into();
    fake.record.as_mut().unwrap().identity_evidence = IdentityEvidence::UnverifiedClaim;
    assert_eq!(
        inventory_from_adapter(&fake, &base_probe(), &[]),
        Err(InventoryError::UnverifiedIdentity)
    );
}

#[test]
fn install_binary_and_model_paths_are_distinct_even_without_consent() {
    let probe = FakeProbe::default();
    let inventory =
        inventory_from_adapter(&adapter(SupportedApp::TextCenter), &probe, &[]).unwrap();
    assert_eq!(
        inventory.executable_path.as_deref(),
        Some(Path::new("/opt/tihulu/bin/app"))
    );
    assert_eq!(
        inventory.installation_path.as_deref(),
        Some(Path::new("/opt/tihulu"))
    );
    assert_eq!(inventory.config_root, None);
    assert!(inventory.model_directories.is_empty());
    assert_eq!(inventory.suppressed_unapproved_or_unverifiable, 4);
    assert_eq!(probe.reads.get(), 0);
    assert!(!inventory.changes_applied);
}

#[test]
fn override_active_convention_and_legacy_precedence_is_deduped() {
    let app = SupportedApp::FlightRadar;
    let mut fake = adapter(app);
    let record = fake.record.as_mut().unwrap();
    record.user_model_override = Some("/home/u/models/target".into());
    record
        .legacy_model_roots
        .push("/home/u/models/active".into());
    let inventory = inventory_from_adapter(&fake, &base_probe(), &grants(app)).unwrap();
    assert_eq!(inventory.model_directories.len(), 4);
    assert_eq!(
        inventory.model_directories[0].role,
        CandidateRole::UserOverride
    );
    assert_eq!(
        inventory.model_directories[1].role,
        CandidateRole::AppReportedActive
    );
    assert_eq!(
        inventory.model_directories[2].role,
        CandidateRole::StandaloneConvention
    );
    assert_eq!(
        inventory.model_directories[3].role,
        CandidateRole::LegacyReadOnly
    );
    assert_eq!(
        inventory.model_directories[3].owner,
        ModelOwner::BorrowedExternal
    );
    assert!(inventory
        .model_directories
        .iter()
        .all(|item| item.metadata_only));
    assert_eq!(
        inventory.effective_model_root().unwrap().role,
        CandidateRole::UserOverride
    );
}

#[test]
fn inaccessible_override_must_not_silently_fall_back() {
    let app = SupportedApp::MediaCenter;
    let mut fake = adapter(app);
    fake.record.as_mut().unwrap().user_model_override = Some("/mnt/private/models".into());
    let inventory = inventory_from_adapter(&fake, &base_probe(), &grants(app)).unwrap();
    assert!(inventory.unresolved_declared_override);
    assert!(inventory.effective_model_root().is_none());
}

#[test]
fn symlink_escape_and_redirected_grant_are_blocked() {
    let app = SupportedApp::SurvivalKit;
    let mut probe = base_probe();
    probe.redirect("/home/u/models/legacy", "/home/u/.ssh");
    let inventory = inventory_from_adapter(&adapter(app), &probe, &grants(app)).unwrap();
    assert_eq!(inventory.model_directories.len(), 2);
    assert_eq!(inventory.suppressed_unapproved_or_unverifiable, 1);
    probe.redirect("/home/u/models", "/outside");
    let second = inventory_from_adapter(&adapter(app), &probe, &grants(app)).unwrap();
    assert!(second.model_directories.is_empty());
}

#[test]
fn wrong_app_scope_unreadable_and_foreign_owned_folders_are_blocked() {
    let app = SupportedApp::MediaCenter;
    let mut probe = base_probe();
    probe.unreadable("/home/u/models/active");
    probe.foreign_owner("/home/u/models/legacy");
    let wrong_grants = grants(SupportedApp::TextCenter);
    let wrong = inventory_from_adapter(&adapter(app), &probe, &wrong_grants).unwrap();
    assert!(wrong.model_directories.is_empty());
    let approved = inventory_from_adapter(&adapter(app), &probe, &grants(app)).unwrap();
    assert_eq!(approved.model_directories.len(), 1);
    assert_eq!(
        approved.model_directories[0].role,
        CandidateRole::StandaloneConvention
    );
}

#[test]
fn relative_traversal_and_unbounded_legacy_inputs_fail_closed() {
    let app = SupportedApp::RfGeolocation;
    let mut fake = adapter(app);
    fake.record.as_mut().unwrap().active_model_root = Some("/home/u/models/../secrets".into());
    fake.record.as_mut().unwrap().legacy_model_roots = vec!["relative/root".into()];
    let inventory = inventory_from_adapter(&fake, &base_probe(), &grants(app)).unwrap();
    assert_eq!(inventory.model_directories.len(), 1);
    assert_eq!(
        inventory.model_directories[0].role,
        CandidateRole::StandaloneConvention
    );
    fake.record.as_mut().unwrap().executable_path = "relative/app".into();
    assert_eq!(
        inventory_from_adapter(&fake, &base_probe(), &[]),
        Err(InventoryError::InvalidInstallRecord)
    );
    let mut many = adapter(app);
    many.record.as_mut().unwrap().legacy_model_roots = (0..17)
        .map(|index| format!("/old/{index}").into())
        .collect();
    let probe = base_probe();
    assert_eq!(
        inventory_from_adapter(&many, &probe, &grants(app)),
        Err(InventoryError::ExcessiveDeclaredPaths)
    );
    assert_eq!(probe.reads.get(), 0);
}

#[test]
fn custom_external_ssd_model_root_requires_its_own_scope() {
    let app = SupportedApp::MediaCenter;
    let mut fake = adapter(app);
    fake.record.as_mut().unwrap().user_model_override = Some("/mnt/ssd/tihulu/weights".into());
    let mut probe = base_probe();
    probe.directory("/mnt/ssd/tihulu");
    probe.directory("/mnt/ssd/tihulu/weights");
    let without = inventory_from_adapter(&fake, &probe, &grants(app)).unwrap();
    assert!(without.unresolved_declared_override);
    let mut approved = grants(app);
    approved.push(grant(app, ScopePurpose::ModelMetadata, "/mnt/ssd/tihulu"));
    let with = inventory_from_adapter(&fake, &probe, &approved).unwrap();
    assert_eq!(
        with.effective_model_root()
            .unwrap()
            .directory
            .canonical_path,
        PathBuf::from("/mnt/ssd/tihulu/weights")
    );
}

#[test]
fn preview_is_reversible_and_never_moves_or_writes_anything() {
    let app = SupportedApp::SatelliteCenter;
    let (inventory, probe, approved) = inventoried(app);
    let before = inventory.clone();
    let preview = preview_model_root_change(
        &inventory,
        &probe,
        &approved,
        Path::new("/home/u/models/target"),
        PathChangeKind::StagedCopyPreview,
        true,
        1,
    )
    .unwrap();
    assert_eq!(
        preview.current_root.canonical_path,
        PathBuf::from("/home/u/models/active")
    );
    assert_eq!(preview.rollback_to, preview.current_root);
    assert_eq!(
        preview.requested_root.canonical_path,
        PathBuf::from("/home/u/models/target")
    );
    assert!(preview.backup_config_required);
    assert!(preview.atomic_config_update_required);
    assert!(preview.idle_and_ledger_check_required);
    assert!(preview.staged_copy_and_digest_check_required);
    assert!(preview.runtime_smoke_test_required);
    assert!(preview.separate_source_cleanup_approval_required);
    assert!(preview.source_preserved);
    assert!(preview.install_directory_untouched);
    assert!(!preview.operation_executed);
    assert_eq!(inventory, before);
}

#[test]
fn consent_config_and_target_are_required_for_preview() {
    let app = SupportedApp::MediaCenter;
    let (inventory, probe, approved) = inventoried(app);
    let target = Path::new("/home/u/models/target");
    assert_eq!(
        preview_model_root_change(
            &inventory,
            &probe,
            &approved,
            target,
            PathChangeKind::RepointOnly,
            false,
            1
        ),
        Err(PreviewError::ExplicitConsentRequired)
    );
    let mut without_config = inventory.clone();
    without_config.config_root = None;
    assert_eq!(
        preview_model_root_change(
            &without_config,
            &probe,
            &approved,
            target,
            PathChangeKind::RepointOnly,
            true,
            1
        ),
        Err(PreviewError::MissingApprovedConfigOrModelRoot)
    );
    assert_eq!(
        preview_model_root_change(
            &inventory,
            &probe,
            &[],
            target,
            PathChangeKind::RepointOnly,
            true,
            1
        ),
        Err(PreviewError::DestinationUnapprovedOrUnverifiable)
    );
    assert_eq!(
        preview_model_root_change(
            &inventory,
            &probe,
            &approved,
            Path::new("/home/u/models/active"),
            PathChangeKind::RepointOnly,
            true,
            1
        ),
        Err(PreviewError::IdenticalRoot)
    );
}

#[test]
fn borrowed_model_and_shared_clients_block_generic_transfer() {
    let app = SupportedApp::MediaCenter;
    let (mut inventory, probe, approved) = inventoried(app);
    let target = Path::new("/home/u/models/target");
    inventory.model_directories[0].owner = ModelOwner::BorrowedExternal;
    for kind in [
        PathChangeKind::StagedCopyPreview,
        PathChangeKind::StagedMovePreview,
    ] {
        assert_eq!(
            preview_model_root_change(&inventory, &probe, &approved, target, kind, true, 1),
            Err(PreviewError::BorrowedSourceCannotTransfer)
        );
    }
    assert!(preview_model_root_change(
        &inventory,
        &probe,
        &approved,
        target,
        PathChangeKind::RepointOnly,
        true,
        1
    )
    .is_ok());
    inventory.model_directories[0].owner = ModelOwner::AppManaged;
    for consumers in [0, 2, 100] {
        assert_eq!(
            preview_model_root_change(
                &inventory,
                &probe,
                &approved,
                target,
                PathChangeKind::StagedMovePreview,
                true,
                consumers
            ),
            Err(PreviewError::SharedSourceCannotMove)
        );
    }
}

#[cfg(windows)]
#[test]
fn native_windows_drive_and_external_volume_paths() {
    let app = SupportedApp::MediaCenter;
    let mut fake = adapter(app);
    let record = fake.record.as_mut().unwrap();
    record.executable_path = r"C:\Program Files\TIHULU\media.exe".into();
    record.installation_path = r"C:\Program Files\TIHULU".into();
    record.config_root = None;
    record.active_model_root = Some(r"D:\TIHULU\models".into());
    record.standalone_default_root = None;
    record.legacy_model_roots.clear();
    let mut probe = FakeProbe::default();
    probe.directory(r"D:\TIHULU");
    probe.directory(r"D:\TIHULU\models");
    let approved = vec![grant(app, ScopePurpose::ModelMetadata, r"D:\TIHULU")];
    let inventory = inventory_from_adapter(&fake, &probe, &approved).unwrap();
    assert_eq!(inventory.model_directories.len(), 1);
}
