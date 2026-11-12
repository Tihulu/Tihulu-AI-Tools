//! Bounded, read-only, explicitly scoped adapter discovery.
//!
//! No automatic home-directory probing and no HTTP requests occur here.

use crate::registry::{
    ArtifactObservation, EndpointObservation, EndpointState, FileFormat, ModelOwner, SourceAdapter,
};
use std::collections::VecDeque;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum DirectorySource {
    /// Caller supplies the current user-approved Jan data folder, not a
    /// guessed default or the application's private settings.
    JanDesktop { data_root: PathBuf },
    /// Caller supplies an explicitly selected HF cache root.
    HuggingFaceCache { cache_root: PathBuf },
    CustomFolder { root: PathBuf, owner_label: String },
}

impl DirectorySource {
    fn root(&self) -> &Path {
        match self {
            Self::JanDesktop { data_root } => data_root,
            Self::HuggingFaceCache { cache_root } => cache_root,
            Self::CustomFolder { root, .. } => root,
        }
    }

    fn adapter_and_owner(&self) -> (SourceAdapter, ModelOwner) {
        match self {
            Self::JanDesktop { .. } => (SourceAdapter::JanDesktop, ModelOwner::JanDesktop),
            Self::HuggingFaceCache { .. } => {
                (SourceAdapter::HuggingFaceCache, ModelOwner::HuggingFace)
            }
            Self::CustomFolder { owner_label, .. } => (
                SourceAdapter::CustomFolder,
                ModelOwner::UserNamed(owner_label.clone()),
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReadApproval {
    /// Must be provided by the UI only after an explicit user permission step.
    pub approved: bool,
    /// Must exactly match the selected source root after normalization.
    pub scope: PathBuf,
}

#[derive(Debug, Clone, Copy)]
pub struct ScanLimits {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_models: usize,
}

impl Default for ScanLimits {
    fn default() -> Self {
        Self {
            max_depth: 5,
            max_entries: 10_000,
            max_models: 500,
        }
    }
}

#[derive(Debug, Default)]
pub struct ScanReport {
    pub artifacts: Vec<ArtifactObservation>,
    pub scanned_entries: usize,
    pub skipped_symlinks: usize,
    pub missing_model_directories: usize,
    pub truncated: bool,
}

/// Metadata-only scan of an explicitly approved root.
///
/// For Jan we enumerate *only* the model trees beneath the granted data
/// directory, not chats, keys, history or extensions. HF snapshot file
/// symlinks are accepted only when their canonical targets stay within the
/// approved cache root. Other symlinks are skipped. No payload is opened.
pub fn scan_approved_source(
    source: &DirectorySource,
    approval: &ReadApproval,
    limits: ScanLimits,
) -> io::Result<ScanReport> {
    if !approval.approved {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "explicit read permission is required",
        ));
    }
    if limits.max_entries == 0 || limits.max_models == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "scan entry/model limits must be positive",
        ));
    }
    let granted = approval.scope.canonicalize()?;
    let selected = source.root().canonicalize()?;
    if granted != selected || !selected.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "approval must match the selected directory exactly",
        ));
    }
    let (adapter, owner) = source.adapter_and_owner();
    if matches!(&owner, ModelOwner::UserNamed(name) if name.trim().is_empty()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "custom source requires a nonempty owner label",
        ));
    }

    let mut report = ScanReport::default();
    let scan_roots = match source {
        DirectorySource::JanDesktop { .. } => {
            vec![
                selected.join("llamacpp").join("models"),
                selected.join("mlx").join("models"),
            ]
        }
        DirectorySource::HuggingFaceCache { .. } | DirectorySource::CustomFolder { .. } => {
            vec![selected.clone()]
        }
    };

    let mut queue = VecDeque::new();
    for root in scan_roots {
        match fs::symlink_metadata(&root) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                let resolved = root.canonicalize()?;
                if resolved.starts_with(&selected) {
                    queue.push_back((resolved, 0usize));
                }
            }
            Ok(_) => report.skipped_symlinks += 1,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                report.missing_model_directories += 1;
            }
            Err(error) => return Err(error),
        }
    }

    'scan: while let Some((directory, depth)) = queue.pop_front() {
        for item in fs::read_dir(&directory)? {
            if report.scanned_entries >= limits.max_entries
                || report.artifacts.len() >= limits.max_models
            {
                report.truncated = true;
                break 'scan;
            }
            let item = item?;
            report.scanned_entries += 1;
            let path = item.path();
            let file_type = item.file_type()?;

            if file_type.is_dir() {
                if depth < limits.max_depth {
                    queue.push_back((path, depth + 1));
                }
                continue;
            }

            let is_link = file_type.is_symlink();
            if is_link && adapter != SourceAdapter::HuggingFaceCache {
                report.skipped_symlinks += 1;
                continue;
            }
            let Some(format_hint) = FileFormat::from_path(&path) else {
                continue;
            };
            // canonicalize() rejects missing/dangling files; never follow an
            // external symlink (even a HF snapshot blob link).
            let resolved = match path.canonicalize() {
                Ok(target) => target,
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    report.skipped_symlinks += 1;
                    continue;
                }
                Err(error) => return Err(error),
            };
            if !resolved.starts_with(&selected) || !resolved.is_file() {
                report.skipped_symlinks += 1;
                continue;
            }
            let size_bytes = fs::metadata(&resolved)?.len();
            report.artifacts.push(ArtifactObservation {
                source_path: path,
                canonical_path: resolved,
                owner: owner.clone(),
                adapter,
                format_hint,
                size_bytes,
            });
        }
    }
    report
        .artifacts
        .sort_by(|left, right| left.source_path.cmp(&right.source_path));
    Ok(report)
}

#[derive(Debug, Clone)]
pub enum EndpointAdapter {
    JanDesktop,
    Ollama,
    OpenAiCompatible { owner_label: String },
}

#[derive(Debug, Clone)]
pub struct EndpointRequest {
    pub adapter: EndpointAdapter,
    pub base_url: String,
    /// Set only after the UI has explicitly approved this exact local URL.
    pub approved: bool,
    /// A separately authorized, trusted client reports a successful model
    /// listing. Merely passing a fixture/model name must not imply this.
    pub listing_confirmed: bool,
    /// Already-fetched model IDs only. This crate makes no network request.
    pub model_ids: Vec<String>,
}

/// Import a previously authorized local API listing without connecting to it.
///
/// Only literal loopback HTTP URLs are accepted; arbitrary URLs, credentials,
/// URL escapes, fragments and traversal are prohibited. This also prevents
/// accidentally sending a probe to an unapproved remote host.
pub fn observe_local_endpoint(request: EndpointRequest) -> io::Result<Vec<EndpointObservation>> {
    if !request.approved {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "local endpoint requires explicit approval",
        ));
    }
    validate_loopback_url(&request.base_url)?;
    if request.model_ids.len() > 128 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "endpoint model listing exceeds 128 IDs",
        ));
    }
    let (adapter, owner) = match request.adapter {
        EndpointAdapter::JanDesktop => (SourceAdapter::JanDesktop, ModelOwner::JanDesktop),
        EndpointAdapter::Ollama => (SourceAdapter::OllamaApi, ModelOwner::Ollama),
        EndpointAdapter::OpenAiCompatible { owner_label } => {
            if owner_label.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "endpoint owner is required",
                ));
            }
            (
                SourceAdapter::LocalInferenceApi,
                ModelOwner::UserNamed(owner_label),
            )
        }
    };
    let mut models = Vec::new();
    for model_id in request.model_ids {
        if model_id.is_empty()
            || model_id.len() > 256
            || model_id.chars().any(char::is_control)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid model identifier",
            ));
        }
        if models.iter().any(|model: &EndpointObservation| model.served_model_id == model_id) {
            continue;
        }
        models.push(EndpointObservation {
            base_url: request.base_url.clone(),
            served_model_id: model_id,
            owner: owner.clone(),
            adapter,
            state: if request.listing_confirmed {
                EndpointState::ListedByAuthorizedClient
            } else {
                EndpointState::Declared
            },
        });
    }
    Ok(models)
}

fn validate_loopback_url(url: &str) -> io::Result<()> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidInput, "only literal local HTTP endpoints");
    let rest = url.strip_prefix("http://").ok_or_else(invalid)?;
    if rest.contains(['@', '?', '#', '%', '\\']) || rest.contains("..") {
        return Err(invalid());
    }
    let authority = rest.split('/').next().ok_or_else(invalid)?;
    let port = if let Some(port) = authority.strip_prefix("localhost:") {
        port
    } else if let Some(port) = authority.strip_prefix("127.0.0.1:") {
        port
    } else if let Some(port) = authority.strip_prefix("[::1]:") {
        port
    } else {
        return Err(invalid());
    };
    if port.is_empty() || !port.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid());
    }
    let port_number: u16 = port.parse().map_err(|_| invalid())?;
    if port_number == 0 || !rest.is_ascii() || rest.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "tihulu-registry-fixture-{}-{nanos}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn approval(&self) -> ReadApproval {
            ReadApproval { approved: true, scope: self.0.clone() }
        }

        fn custom(&self) -> DirectorySource {
            DirectorySource::CustomFolder {
                root: self.0.clone(),
                owner_label: "test-owner".into(),
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn unauthorized_requests_do_not_enumerate() {
        let fixture = Fixture::new();
        let mut denied = fixture.approval();
        denied.approved = false;
        let result = scan_approved_source(&fixture.custom(), &denied, ScanLimits::default());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn jan_only_scans_approved_model_subfolders() {
        let fixture = Fixture::new();
        let models = fixture.0.join("llamacpp/models");
        fs::create_dir_all(&models).unwrap();
        fs::write(models.join("chat.gguf"), b"fixture").unwrap();
        fs::write(fixture.0.join("chat.gguf"), b"outside-model-tree").unwrap();
        fs::write(fixture.0.join("private-chat.txt"), b"secret").unwrap();
        let result = scan_approved_source(
            &DirectorySource::JanDesktop { data_root: fixture.0.clone() },
            &fixture.approval(),
            ScanLimits::default(),
        )
        .unwrap();
        assert_eq!(result.artifacts.len(), 1);
        assert_eq!(result.missing_model_directories, 1);
        assert!(result.artifacts[0].source_path.starts_with(models));
    }

    #[test]
    fn bounded_scan_and_format_hints_are_not_validation() {
        let fixture = Fixture::new();
        for index in 0..8 {
            fs::write(fixture.0.join(format!("{index}.gguf")), b"x").unwrap();
        }
        fs::write(fixture.0.join("unknown.bin"), b"x").unwrap();
        let report = scan_approved_source(
            &fixture.custom(),
            &fixture.approval(),
            ScanLimits { max_entries: 3, ..ScanLimits::default() },
        )
        .unwrap();
        assert!(report.truncated);
        assert!(report.scanned_entries <= 3);
        assert!(report.artifacts.len() <= 3);
    }

    #[test]
    fn scope_mismatch_and_missing_sources_fail_closed() {
        let one = Fixture::new();
        let two = Fixture::new();
        let result = scan_approved_source(&one.custom(), &two.approval(), ScanLimits::default());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        let missing = DirectorySource::CustomFolder {
            root: one.0.join("missing"), owner_label: "test".into()
        };
        assert_eq!(
            scan_approved_source(&missing, &one.approval(), ScanLimits::default())
                .unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[cfg(unix)]
    #[test]
    fn hf_internal_snapshot_symlinks_are_allowed_but_escapes_are_not() {
        use std::os::unix::fs::symlink;
        let fixture = Fixture::new();
        let outside = Fixture::new();
        let blobs = fixture.0.join("hub/models--example/blobs");
        let snap = fixture.0.join("hub/models--example/snapshots/revision");
        fs::create_dir_all(&blobs).unwrap();
        fs::create_dir_all(&snap).unwrap();
        fs::write(blobs.join("hash"), b"synthetic").unwrap();
        fs::write(outside.0.join("secret.gguf"), b"private").unwrap();
        symlink(blobs.join("hash"), snap.join("model.safetensors")).unwrap();
        symlink(outside.0.join("secret.gguf"), snap.join("outside.gguf")).unwrap();
        let report = scan_approved_source(
            &DirectorySource::HuggingFaceCache { cache_root: fixture.0.clone() },
            &fixture.approval(),
            ScanLimits::default(),
        )
        .unwrap();
        assert_eq!(report.artifacts.len(), 1);
        assert_eq!(report.artifacts[0].format_hint, FileFormat::Safetensors);
        assert!(report.skipped_symlinks >= 1);
    }

    #[test]
    fn endpoint_permissions_and_availability_are_separate() {
        let request = EndpointRequest {
            adapter: EndpointAdapter::Ollama,
            base_url: "http://localhost:11434".into(),
            approved: true,
            listing_confirmed: false,
            model_ids: vec!["chat:latest".into(), "chat:latest".into()],
        };
        let entries = observe_local_endpoint(request.clone()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].state, EndpointState::Declared);
        let entries = observe_local_endpoint(EndpointRequest {
            listing_confirmed: true, ..request
        }).unwrap();
        assert_eq!(entries[0].state, EndpointState::ListedByAuthorizedClient);
    }

    #[test]
    fn no_remote_or_credential_urls() {
        for url in [
            "https://localhost:11434",
            "http://example.com:11434",
            "http://127.0.0.1:11434@attacker.test",
            "http://0.0.0.0:11434",
            "http://localhost:0",
            "http://localhost:65536",
            "http://localhost:11434/../private",
            "http://localhost:11434/%2e%2e",
        ] {
            let result = observe_local_endpoint(EndpointRequest {
                adapter: EndpointAdapter::JanDesktop,
                base_url: url.into(),
                approved: true,
                listing_confirmed: true,
                model_ids: vec!["test".into()],
            });
            assert!(result.is_err(), "must reject {url}");
        }
    }
}
