use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static UNIQUE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "tihulu-consent-fixture-{}-{stamp}-{}",
            std::process::id(),
            UNIQUE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self { root }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    fn write(&self, relative: &str, data: &[u8]) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    }

    fn gguf(&self, relative: &str) -> PathBuf {
        let mut bytes = b"GGUF".to_vec();
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        self.write(relative, &bytes)
    }

    fn safetensors(&self, relative: &str) -> PathBuf {
        let header = br#"{"tensor":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}"#;
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend_from_slice(header);
        bytes.extend_from_slice(&[0, 0, 0, 0]);
        self.write(relative, &bytes)
    }

    fn approve(&self, root: &Path, source: ModelSource) -> ApprovedModelScope {
        ApprovedModelScope::approve(
            root,
            source,
            ConsentDecision {
                user_approved_read: true,
                include_hidden_descendants: false,
            },
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(windows)]
fn symlink_file(target: &Path, link: &Path) -> io::Result<()> {
    std::os::windows::fs::symlink_file(target, link)
}

#[test]
fn explicit_consent_required_and_revocation_is_shared_across_clones() {
    let fixture = Fixture::new();
    let denied = ApprovedModelScope::approve(
        &fixture.root,
        ModelSource::UserFolder {
            owner_label: "custom".into(),
        },
        ConsentDecision {
            user_approved_read: false,
            include_hidden_descendants: false,
        },
    );
    assert!(matches!(denied, Err(DiscoveryError::ConsentRequired)));
    let scope = fixture.approve(
        &fixture.root,
        ModelSource::UserFolder {
            owner_label: "custom".into(),
        },
    );
    let copy = scope.clone();
    fixture.gguf("model.gguf");
    assert_eq!(
        discover_approved_models(&scope, DiscoveryBounds::default())
            .unwrap()
            .models
            .len(),
        1
    );
    scope.revoke();
    assert!(matches!(
        discover_approved_models(&copy, DiscoveryBounds::default()),
        Err(DiscoveryError::Revoked)
    ));
}

#[test]
fn reject_parent_traversal_and_filesystem_root_grants() {
    let fixture = Fixture::new();
    let parent_path = fixture.root.join("nested").join("..");
    assert!(matches!(
        ApprovedModelScope::approve(
            &parent_path,
            ModelSource::JanDesktop,
            ConsentDecision {
                user_approved_read: true,
                include_hidden_descendants: false,
            }
        ),
        Err(DiscoveryError::InvalidSource)
    ));
    #[cfg(unix)]
    assert!(matches!(
        ApprovedModelScope::approve(
            Path::new("/"),
            ModelSource::JanDesktop,
            ConsentDecision {
                user_approved_read: true,
                include_hidden_descendants: false,
            }
        ),
        Err(DiscoveryError::UnsafeScope)
    ));
}

#[test]
fn jan_scans_only_explicitly_granted_engine_model_folders() {
    let fixture = Fixture::new();
    fixture.gguf("jan-data/llamacpp/models/chat.gguf");
    fixture.gguf("jan-data/mlx/models/vision.gguf");
    fixture.gguf("jan-data/private/chat-history.gguf");
    fixture.gguf("jan-data/.secrets/secret.gguf");
    fixture.write(
        "jan-data/llamacpp/models/config.json",
        br#"{"model_type":"llama","architectures":["LlamaForCausalLM"]}"#,
    );
    let grant = fixture.approve(&fixture.path("jan-data"), ModelSource::JanDesktop);
    let inventory = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(inventory.models.len(), 2);
    assert!(inventory
        .models
        .iter()
        .all(|m| m.owner == ModelOwner::JanDesktop));
    assert!(inventory
        .models
        .iter()
        .all(|m| m.evidence == FileEvidence::FormatHeaderVerified));
    let approved_jan = grant.approved_root();
    assert!(inventory.models.iter().all(|model| {
        model
            .location
            .starts_with(approved_jan.join("llamacpp/models"))
            || model.location.starts_with(approved_jan.join("mlx/models"))
    }));
    assert!(inventory
        .models
        .iter()
        .any(|m| m.metadata.get("model_type") == Some(&"llama".into())));
}

#[test]
fn ollama_inventory_only_uses_approved_manifests_and_model_blobs() {
    let fixture = Fixture::new();
    let digest = "a".repeat(64);
    fixture.write(&format!("ollama/blobs/sha256-{digest}"), b"payload");
    fixture.write("ollama/blobs/unrelated-file", b"not-a-model");
    let manifest = format!(
        r#"{{"schemaVersion":2,"layers":[{{"mediaType":"application/vnd.ollama.image.model","digest":"sha256:{digest}","size":7}},{{"mediaType":"application/vnd.ollama.image.template","digest":"sha256:{}","size":1}}]}}"#,
        "b".repeat(64)
    );
    fixture.write(
        "ollama/manifests/registry.ollama.ai/library/synthetic/latest",
        manifest.as_bytes(),
    );
    let grant = fixture.approve(&fixture.path("ollama"), ModelSource::OllamaStore);
    let inventory = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(inventory.models.len(), 1);
    let model = &inventory.models[0];
    assert_eq!(model.owner, ModelOwner::Ollama);
    assert_eq!(model.display_name, "synthetic:latest");
    assert_eq!(model.evidence, FileEvidence::UnverifiedCandidate);
    assert!(model.format_hint.is_none());
    assert_eq!(model.size_bytes, 7);
    assert!(model.provenance_path.ends_with("synthetic/latest"));
    assert_eq!(
        model.metadata.get("declared_digest"),
        Some(&format!("sha256:{digest}"))
    );
}

#[test]
fn malformed_ollama_manifest_does_not_become_installed_model() {
    let fixture = Fixture::new();
    fixture.write(
        "ollama/manifests/registry.ollama.ai/library/model/latest",
        b"{no-json",
    );
    let grant = fixture.approve(&fixture.path("ollama"), ModelSource::OllamaStore);
    let report = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert!(report.models.is_empty());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.kind == DiscoveryWarningKind::MalformedMetadata));
}

#[test]
fn hf_valid_snapshot_symlink_remains_inside_approved_cache() {
    let fixture = Fixture::new();
    let blob = fixture.safetensors("cache/models--org--model/blobs/blob-a");
    let alias = fixture.path("cache/models--org--model/snapshots/revision/model.safetensors");
    fs::create_dir_all(alias.parent().unwrap()).unwrap();
    if let Err(error) = symlink_file(&blob, &alias) {
        // Windows runner may lack symlink privilege; this is a distinct
        // integration prerequisite, not a reason to scan outside the scope.
        if error.kind() == io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("test symlink creation failed: {error}");
    }
    fixture.gguf("cache/datasets--private/snapshots/revision/private.gguf");
    let grant = fixture.approve(&fixture.path("cache"), ModelSource::HuggingFaceCache);
    let report = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(report.models.len(), 1);
    assert_eq!(
        report.models[0].evidence,
        FileEvidence::FormatHeaderVerified
    );
    assert_eq!(report.models[0].owner, ModelOwner::HuggingFace);
    assert_eq!(report.models[0].location, blob.canonicalize().unwrap());
}

#[test]
fn hf_external_symlink_escape_is_skipped() {
    let fixture = Fixture::new();
    let external = Fixture::new();
    let file = external.gguf("private.gguf");
    let alias = fixture.path("cache/models--org--model/snapshots/revision/model.gguf");
    fs::create_dir_all(alias.parent().unwrap()).unwrap();
    if let Err(error) = symlink_file(&file, &alias) {
        if error.kind() == io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("test symlink creation failed: {error}");
    }
    let grant = fixture.approve(&fixture.path("cache"), ModelSource::HuggingFaceCache);
    let report = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert!(report.models.is_empty());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.kind == DiscoveryWarningKind::OutsideScope));
}

#[test]
fn jan_symlinked_engine_folder_cannot_escape_selected_scope() {
    let fixture = Fixture::new();
    let external = Fixture::new();
    external.gguf("models/external.gguf");
    fs::create_dir_all(fixture.path("jan")).unwrap();
    #[cfg(unix)]
    let link = std::os::unix::fs::symlink(external.path("models"), fixture.path("jan/llamacpp"));
    #[cfg(windows)]
    let link =
        std::os::windows::fs::symlink_dir(external.path("models"), fixture.path("jan/llamacpp"));
    if let Err(error) = link {
        if error.kind() == io::ErrorKind::PermissionDenied {
            return;
        }
        panic!("test symlink creation failed: {error}");
    }
    let grant = fixture.approve(&fixture.path("jan"), ModelSource::JanDesktop);
    let report = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert!(report.models.is_empty());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.kind == DiscoveryWarningKind::SymlinkSkipped));
}

#[test]
fn independent_tihulu_app_folders_never_inherit_another_apps_models() {
    let fixture = Fixture::new();
    let media = fixture.gguf("storage/apps/tihulu-media-center/model.gguf");
    fixture.gguf("storage/apps/tihulu-text-center/model.gguf");
    let media_grant = fixture.approve(
        media.parent().unwrap(),
        ModelSource::TihuluApp {
            app_id: "tihulu-media-center".into(),
        },
    );
    let text_grant = fixture.approve(
        &fixture.path("storage/apps/tihulu-text-center"),
        ModelSource::TihuluApp {
            app_id: "tihulu-text-center".into(),
        },
    );
    let media_inventory =
        discover_approved_models(&media_grant, DiscoveryBounds::default()).unwrap();
    let text_inventory = discover_approved_models(&text_grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(media_inventory.models.len(), 1);
    assert_eq!(text_inventory.models.len(), 1);
    assert_ne!(media_inventory.models[0].id, text_inventory.models[0].id);
    assert_eq!(
        media_inventory.models[0].owner,
        ModelOwner::TihuluApp("tihulu-media-center".into())
    );
    assert_eq!(
        text_inventory.models[0].owner,
        ModelOwner::TihuluApp("tihulu-text-center".into())
    );
}

#[test]
fn selected_external_ssd_subfolder_is_supported_without_volume_root_scan() {
    let fixture = Fixture::new();
    fixture.gguf("external-ssd/My Models/model.gguf");
    fixture.gguf("external-ssd/Private/should-not-scan.gguf");
    let grant = fixture.approve(
        &fixture.path("external-ssd/My Models"),
        ModelSource::UserFolder {
            owner_label: "external SSD".into(),
        },
    );
    let report = DiscoveryReadOnlyPort::inventory(&grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(report.models.len(), 1);
    assert_eq!(report.models[0].display_name, "model");
}

#[test]
fn hidden_descendants_require_separate_explicit_consent() {
    let fixture = Fixture::new();
    fixture.gguf("models/visible.gguf");
    fixture.gguf("models/.private/secret.gguf");
    let source = ModelSource::UserFolder {
        owner_label: "custom".into(),
    };
    let grant = fixture.approve(&fixture.path("models"), source.clone());
    assert_eq!(
        discover_approved_models(&grant, DiscoveryBounds::default())
            .unwrap()
            .models
            .len(),
        1
    );
    let grant_with_hidden = ApprovedModelScope::approve(
        &fixture.path("models"),
        source,
        ConsentDecision {
            user_approved_read: true,
            include_hidden_descendants: true,
        },
    )
    .unwrap();
    assert_eq!(
        discover_approved_models(&grant_with_hidden, DiscoveryBounds::default())
            .unwrap()
            .models
            .len(),
        2
    );
}

#[test]
fn same_filename_in_different_folders_is_not_deduplicated() {
    let fixture = Fixture::new();
    fixture.gguf("models/a/model.gguf");
    fixture.gguf("models/b/model.gguf");
    let grant = fixture.approve(
        &fixture.path("models"),
        ModelSource::UserFolder {
            owner_label: "custom".into(),
        },
    );
    let inventory = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(inventory.models.len(), 2);
    assert_ne!(inventory.models[0].id, inventory.models[1].id);
}

#[test]
fn bogus_gguf_header_and_malformed_config_remain_unverified() {
    let fixture = Fixture::new();
    fixture.write("models/fake.gguf", b"invalid-header-123");
    fixture.write("models/config.json", b"{ malformed ");
    let grant = fixture.approve(
        &fixture.path("models"),
        ModelSource::UserFolder {
            owner_label: "custom".into(),
        },
    );
    let report = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    assert_eq!(report.models.len(), 1);
    assert_eq!(report.models[0].evidence, FileEvidence::UnverifiedCandidate);
    assert!(report.models[0].metadata.is_empty());
    assert!(report
        .warnings
        .iter()
        .any(|w| w.kind == DiscoveryWarningKind::MalformedMetadata));
}

#[test]
fn bounded_directory_enumeration_marks_truncated_results() {
    let fixture = Fixture::new();
    for n in 0..10 {
        fixture.gguf(&format!("models/{n}.gguf"));
    }
    let grant = fixture.approve(
        &fixture.path("models"),
        ModelSource::UserFolder {
            owner_label: "test".into(),
        },
    );
    let report = discover_approved_models(
        &grant,
        DiscoveryBounds {
            max_entries: 2,
            max_models: 50,
            max_depth: 4,
        },
    )
    .unwrap();
    assert!(report.truncated);
    assert!(report.scanned_entries <= 2);
    assert!(report.models.len() <= 2);
}

#[cfg(unix)]
#[test]
fn unreadable_synthetic_file_is_never_upgraded_to_verified() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let file = fixture.gguf("models/locked.gguf");
    let mut perms = fs::metadata(&file).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&file, perms).unwrap();
    let inaccessible = File::open(&file).is_err();
    let grant = fixture.approve(
        &fixture.path("models"),
        ModelSource::UserFolder {
            owner_label: "test".into(),
        },
    );
    let report = discover_approved_models(&grant, DiscoveryBounds::default()).unwrap();
    if inaccessible {
        assert!(report
            .models
            .iter()
            .all(|m| m.evidence == FileEvidence::UnverifiedCandidate));
        assert!(report
            .warnings
            .iter()
            .any(|w| w.kind == DiscoveryWarningKind::Inaccessible));
    }
    let mut perms = fs::metadata(&file).unwrap().permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&file, perms).unwrap();
}

#[test]
fn discovery_does_not_modify_source_weights_or_configs() {
    let fixture = Fixture::new();
    let file = fixture.gguf("models/model.gguf");
    let metadata = fixture.write("models/config.json", br#"{"model_type":"llama"}"#);
    let before = fs::read(&file).unwrap();
    let before_config = fs::read(&metadata).unwrap();
    let grant = fixture.approve(
        &fixture.path("models"),
        ModelSource::UserFolder {
            owner_label: "test".into(),
        },
    );
    assert_eq!(
        discover_approved_models(&grant, DiscoveryBounds::default())
            .unwrap()
            .models
            .len(),
        1
    );
    assert_eq!(before, fs::read(&file).unwrap());
    assert_eq!(before_config, fs::read(&metadata).unwrap());
}
