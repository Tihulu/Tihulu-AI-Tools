//! Explicitly approved, read-only local model discovery.
//!
//! No default home scan, network call, model download, write, runtime load,
//! or application reconfiguration. No grant is obtained automatically.
//! Evidence for an on-disk file is not runtime or hardware compatibility.
//!
//! Threat boundary: this cross-platform std::fs implementation checks
//! canonical containment and skips symlink directories, but cannot prevent
//! adversarial TOCTOU directory replacement. Untrusted writable roots require
//! platform handle-relative opening in a separately reviewed native backend.

use crate::registry::{FileFormat, ModelId, ModelOwner, SourceAdapter};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_ENTRIES: usize = 10_000;
const MAX_MODELS: usize = 500;
const MAX_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelSource {
    JanDesktop,
    OllamaStore,
    HuggingFaceCache,
    /// Each TIHULU app has an independently approved model directory.
    TihuluApp { app_id: String },
    UserFolder { owner_label: String },
}

impl ModelSource {
    fn owner_and_adapter(&self) -> (ModelOwner, SourceAdapter) {
        match self {
            Self::JanDesktop => (ModelOwner::JanDesktop, SourceAdapter::JanDesktop),
            Self::OllamaStore => (ModelOwner::Ollama, SourceAdapter::OllamaStore),
            Self::HuggingFaceCache => (ModelOwner::HuggingFace, SourceAdapter::HuggingFaceCache),
            Self::TihuluApp { app_id } => (
                ModelOwner::TihuluApp(app_id.clone()),
                SourceAdapter::TihuluAppDirectory,
            ),
            Self::UserFolder { owner_label } => (
                ModelOwner::UserNamed(owner_label.clone()),
                SourceAdapter::CustomFolder,
            ),
        }
    }

    fn is_valid(&self) -> bool {
        match self {
            Self::TihuluApp { app_id } => {
                app_id.len() <= 64
                    && app_id.bytes().next().is_some_and(|c| c.is_ascii_lowercase())
                    && app_id.bytes().all(|c| {
                        c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-'
                    })
                    && !app_id.contains("..")
            }
            Self::UserFolder { owner_label } => {
                !owner_label.trim().is_empty()
                    && owner_label.len() <= 128
                    && !owner_label.chars().any(char::is_control)
            }
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsentDecision {
    pub user_approved_read: bool,
    /// Hidden descendants are omitted unless this separate choice is true.
    /// An explicitly selected root may itself be under a hidden OS folder.
    pub include_hidden_descendants: bool,
}

#[derive(Debug, Clone)]
pub struct ApprovedModelScope {
    selected_root: PathBuf,
    canonical_root: PathBuf,
    source: ModelSource,
    include_hidden: bool,
    active: Arc<AtomicBool>,
}

#[derive(Debug)]
pub enum DiscoveryError {
    ConsentRequired,
    Revoked,
    UnsafeScope,
    InvalidSource,
    Io(io::Error),
}

impl From<io::Error> for DiscoveryError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl ApprovedModelScope {
    /// Only the trusted native consent controller may call this after showing
    /// the exact path, resolved path, owner and hidden-directory option.
    /// This constructor is not OS permission authorization by itself.
    pub fn approve(
        selected_root: &Path,
        source: ModelSource,
        consent: ConsentDecision,
    ) -> Result<Self, DiscoveryError> {
        if !consent.user_approved_read {
            return Err(DiscoveryError::ConsentRequired);
        }
        if !source.is_valid()
            || selected_root.as_os_str().is_empty()
            || !selected_root.is_absolute()
            || selected_root
                .components()
                .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(DiscoveryError::InvalidSource);
        }
        // Never interpret an entire filesystem or drive root as a model root.
        if selected_root.parent().is_none() {
            return Err(DiscoveryError::UnsafeScope);
        }
        // Require the selected root itself to be real, not a redirect.
        if fs::symlink_metadata(selected_root)?.file_type().is_symlink() {
            return Err(DiscoveryError::UnsafeScope);
        }
        let canonical_root = selected_root.canonicalize()?;
        if !canonical_root.is_dir() || canonical_root.parent().is_none() {
            return Err(DiscoveryError::UnsafeScope);
        }
        Ok(Self {
            selected_root: selected_root.to_path_buf(),
            canonical_root,
            source,
            include_hidden: consent.include_hidden_descendants,
            active: Arc::new(AtomicBool::new(true)),
        })
    }

    /// Revocation is shared with all clones of this grant. No new reads are
    /// permitted once a scan observes revocation; partial results are dropped.
    pub fn revoke(&self) {
        self.active.store(false, Ordering::SeqCst);
    }

    pub fn approved_root(&self) -> &Path {
        &self.canonical_root
    }

    pub fn selected_root(&self) -> &Path {
        &self.selected_root
    }

    pub fn source(&self) -> &ModelSource {
        &self.source
    }

    fn ensure_active(&self) -> Result<(), DiscoveryError> {
        if self.active.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(DiscoveryError::Revoked)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiscoveryBounds {
    pub max_depth: usize,
    pub max_entries: usize,
    pub max_models: usize,
}

impl Default for DiscoveryBounds {
    fn default() -> Self {
        Self {
            max_depth: 6,
            max_entries: 4_000,
            max_models: 250,
        }
    }
}

impl DiscoveryBounds {
    fn effective(self) -> Self {
        Self {
            max_depth: self.max_depth.min(MAX_DEPTH),
            max_entries: self.max_entries.min(MAX_ENTRIES),
            max_models: self.max_models.min(MAX_MODELS),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEvidence {
    /// Extension or a trusted provider's model manifest is only a candidate.
    UnverifiedCandidate,
    /// Only a bounded GGUF/SafeTensors container-header check succeeded.
    /// No integrity, model completeness or runtime validation was performed.
    FormatHeaderVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoveryWarningKind {
    MissingModelFolder,
    SymlinkSkipped,
    OutsideScope,
    Inaccessible,
    MalformedMetadata,
    InvalidModelReference,
}

#[derive(Debug, Clone)]
pub struct DiscoveryWarning {
    pub kind: DiscoveryWarningKind,
    /// Private path; the UI must show this only after user consent.
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    pub id: ModelId,
    pub display_name: String,
    pub location: PathBuf,
    pub provenance_path: PathBuf,
    pub owner: ModelOwner,
    pub source: ModelSource,
    pub format_hint: Option<FileFormat>,
    pub size_bytes: u64,
    pub evidence: FileEvidence,
    /// Whitelisted, bounded and untrusted; never a compatibility assertion.
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Default)]
pub struct DiscoveryInventory {
    pub models: Vec<DiscoveredModel>,
    pub warnings: Vec<DiscoveryWarning>,
    pub scanned_entries: usize,
    pub truncated: bool,
}

impl DiscoveryInventory {
    /// Duplicate names/quantizations in distinct app directories survive:
    /// only exact typed provenance IDs are deduplicated.
    fn insert(&mut self, model: DiscoveredModel, seen: &mut BTreeSet<ModelId>) {
        if seen.insert(model.id.clone()) {
            self.models.push(model);
        }
    }

    fn warn(&mut self, kind: DiscoveryWarningKind, path: &Path) {
        self.warnings.push(DiscoveryWarning {
            kind,
            path: path.to_path_buf(),
        });
    }
}

/// The read-only Tauri adapter may map this DTO into its own versioned
/// request/response schema. This crate does not register native IPC commands.
pub struct DiscoveryReadOnlyPort;

impl DiscoveryReadOnlyPort {
    pub fn inventory(
        grant: &ApprovedModelScope,
        bounds: DiscoveryBounds,
    ) -> Result<DiscoveryInventory, DiscoveryError> {
        discover_approved_models(grant, bounds)
    }
}

/// Enumerate only *selected*, approved model roots. Reads model-file headers
/// (up to 64 KiB) and the known model metadata JSON/manifest files only.
pub fn discover_approved_models(
    grant: &ApprovedModelScope,
    bounds: DiscoveryBounds,
) -> Result<DiscoveryInventory, DiscoveryError> {
    grant.ensure_active()?;
    let bounds = bounds.effective();
    if bounds.max_entries == 0 || bounds.max_models == 0 {
        return Err(DiscoveryError::UnsafeScope);
    }

    let mut result = DiscoveryInventory::default();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();

    match &grant.source {
        ModelSource::JanDesktop => {
            for relative in ["llamacpp/models", "mlx/models"] {
                let dir = grant.canonical_root.join(relative);
                enqueue_known_subdir(grant, &dir, &mut queue, &mut result)?;
            }
        }
        ModelSource::OllamaStore => {
            enqueue_known_subdir(
                grant,
                &grant.canonical_root.join("manifests"),
                &mut queue,
                &mut result,
            )?;
        }
        _ => queue.push_back((grant.canonical_root.clone(), 0)),
    }

    while let Some((directory, depth)) = queue.pop_front() {
        grant.ensure_active()?;
        if result.scanned_entries >= bounds.max_entries || result.models.len() >= bounds.max_models {
            result.truncated = true;
            break;
        }
        // Never read outside of canonical approved scope.
        if !directory.starts_with(&grant.canonical_root) {
            return Err(DiscoveryError::UnsafeScope);
        }
        let iter = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                if directory == grant.canonical_root {
                    return Err(error.into());
                }
                result.warn(DiscoveryWarningKind::Inaccessible, &directory);
                continue;
            }
        };
        let remaining = bounds.max_entries - result.scanned_entries;
        // Materialize at most remaining+1 entries, not the whole directory.
        let mut entries = Vec::new();
        for candidate in iter.take(remaining.saturating_add(1)) {
            grant.ensure_active()?;
            match candidate {
                Ok(item) => entries.push(item),
                Err(_) => result.warn(DiscoveryWarningKind::Inaccessible, &directory),
            }
        }
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            grant.ensure_active()?;
            if result.scanned_entries >= bounds.max_entries
                || result.models.len() >= bounds.max_models
            {
                result.truncated = true;
                break;
            }
            result.scanned_entries += 1;
            let path = entry.path();
            let ty = match entry.file_type() {
                Ok(kind) => kind,
                Err(_) => {
                    result.warn(DiscoveryWarningKind::Inaccessible, &path);
                    continue;
                }
            };
            if !grant.include_hidden && is_hidden(&path) {
                continue;
            }
            if ty.is_dir() {
                if depth < bounds.max_depth && eligible_directory(grant, &path, depth) {
                    queue.push_back((path, depth + 1));
                }
                continue;
            }
            if ty.is_symlink() && !is_hf_snapshot_file(grant, &path) {
                result.warn(DiscoveryWarningKind::SymlinkSkipped, &path);
                continue;
            }
            if !ty.is_file() && !ty.is_symlink() {
                continue;
            }
            if grant.source == ModelSource::OllamaStore {
                read_ollama_manifest(grant, &path, &mut result, &mut seen)?;
            } else {
                read_model_candidate(grant, &path, &mut result, &mut seen)?;
            }
        }
        if result.truncated {
            break;
        }
    }
    grant.ensure_active()?;
    result.models.sort_by(|a, b| a.id.cmp(&b.id));
    result.warnings.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

fn enqueue_known_subdir(
    grant: &ApprovedModelScope,
    path: &Path,
    queue: &mut VecDeque<(PathBuf, usize)>,
    report: &mut DiscoveryInventory,
) -> Result<(), DiscoveryError> {
    grant.ensure_active()?;
    // Check intermediate components so a Jan "llamacpp" directory cannot be
    // a symlink that escapes before reaching its "models" directory.
    let relative = path
        .strip_prefix(&grant.canonical_root)
        .map_err(|_| DiscoveryError::UnsafeScope)?;
    let mut current = grant.canonical_root.clone();
    for component in relative.components() {
        current.push(component);
        let meta = match fs::symlink_metadata(&current) {
            Ok(meta) => meta,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                report.warn(DiscoveryWarningKind::MissingModelFolder, &current);
                return Ok(());
            }
            Err(_) => {
                report.warn(DiscoveryWarningKind::Inaccessible, &current);
                return Ok(());
            }
        };
        if meta.file_type().is_symlink() {
            report.warn(DiscoveryWarningKind::SymlinkSkipped, &current);
            return Ok(());
        }
        if !meta.is_dir() {
            report.warn(DiscoveryWarningKind::InvalidModelReference, &current);
            return Ok(());
        }
    }
    queue.push_back((path.to_path_buf(), 0));
    Ok(())
}

fn is_hidden(path: &Path) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
    {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.file_attributes() & 0x6 != 0 {
                return true;
            }
        }
    }
    false
}

fn eligible_directory(grant: &ApprovedModelScope, path: &Path, depth: usize) -> bool {
    match grant.source {
        ModelSource::HuggingFaceCache => {
            let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
            (depth == 0 && name.starts_with("models--"))
                || (depth == 1 && name == "snapshots")
                || depth >= 2
        }
        _ => true,
    }
}

fn is_hf_snapshot_file(grant: &ApprovedModelScope, path: &Path) -> bool {
    grant.source == ModelSource::HuggingFaceCache
        && path
            .strip_prefix(&grant.canonical_root)
            .ok()
            .is_some_and(|relative| relative.components().any(|part| part.as_os_str() == "snapshots"))
}

fn checked_file(
    grant: &ApprovedModelScope,
    path: &Path,
    allow_snapshot_symlink: bool,
) -> Result<(PathBuf, u64), DiscoveryWarningKind> {
    if !grant.active.load(Ordering::SeqCst) {
        return Err(DiscoveryWarningKind::Inaccessible);
    }
    let meta = fs::symlink_metadata(path).map_err(|_| DiscoveryWarningKind::Inaccessible)?;
    if meta.file_type().is_symlink() && !allow_snapshot_symlink {
        return Err(DiscoveryWarningKind::SymlinkSkipped);
    }
    if !meta.is_file() && !meta.file_type().is_symlink() {
        return Err(DiscoveryWarningKind::InvalidModelReference);
    }
    let canonical = path
        .canonicalize()
        .map_err(|_| DiscoveryWarningKind::Inaccessible)?;
    if !canonical.starts_with(&grant.canonical_root) {
        return Err(DiscoveryWarningKind::OutsideScope);
    }
    let file_meta = fs::metadata(&canonical).map_err(|_| DiscoveryWarningKind::Inaccessible)?;
    if !file_meta.is_file() {
        return Err(DiscoveryWarningKind::InvalidModelReference);
    }
    Ok((canonical, file_meta.len()))
}

fn read_model_candidate(
    grant: &ApprovedModelScope,
    path: &Path,
    inventory: &mut DiscoveryInventory,
    seen: &mut BTreeSet<ModelId>,
) -> Result<(), DiscoveryError> {
    let Some(format_hint) = FileFormat::from_path(path) else {
        return Ok(());
    };
    grant.ensure_active()?;
    let (canonical, size) = match checked_file(grant, path, is_hf_snapshot_file(grant, path)) {
        Ok(result) => result,
        Err(warning) => {
            inventory.warn(warning, path);
            return Ok(());
        }
    };
    let evidence = match verify_header(grant, &canonical, size, format_hint) {
        Ok(true) => FileEvidence::FormatHeaderVerified,
        Ok(false) => FileEvidence::UnverifiedCandidate,
        Err(_) => {
            inventory.warn(DiscoveryWarningKind::Inaccessible, path);
            FileEvidence::UnverifiedCandidate
        }
    };
    let (owner, adapter) = grant.source.owner_and_adapter();
    let id = ModelId::Artifact {
        adapter,
        owner: owner.clone(),
        source_path: path.to_path_buf(),
        canonical_path: canonical.clone(),
    };
    let metadata = read_selected_config(grant, path, inventory)?;
    let display_name = path
        .file_stem()
        .map(|name| name.to_string_lossy().chars().take(128).collect())
        .unwrap_or_else(|| "unknown-model".into());
    inventory.insert(
        DiscoveredModel {
            id,
            display_name,
            location: canonical,
            provenance_path: path.to_path_buf(),
            owner,
            source: grant.source.clone(),
            format_hint: Some(format_hint),
            size_bytes: size,
            evidence,
            metadata,
        },
        seen,
    );
    Ok(())
}

fn verify_header(
    grant: &ApprovedModelScope,
    file: &Path,
    file_size: u64,
    format: FileFormat,
) -> Result<bool, DiscoveryError> {
    grant.ensure_active()?;
    match format {
        FileFormat::Gguf if file_size >= 12 => {
            let mut prefix = [0u8; 8];
            File::open(file)?.read_exact(&mut prefix)?;
            let version = u32::from_le_bytes(prefix[4..8].try_into().expect("four bytes"));
            Ok(&prefix[0..4] == b"GGUF" && (2..=3).contains(&version))
        }
        FileFormat::Safetensors if file_size >= 16 => {
            let mut input = File::open(file)?;
            let mut prefix = [0u8; 8];
            input.read_exact(&mut prefix)?;
            let header_len = u64::from_le_bytes(prefix);
            if header_len < 2
                || header_len > MAX_METADATA_BYTES as u64
                || header_len > file_size.saturating_sub(8)
            {
                return Ok(false);
            }
            let mut bytes = vec![0; header_len as usize];
            input.read_exact(&mut bytes)?;
            let header = match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(object)) => object,
                _ => return Ok(false),
            };
            let payload_size = file_size - 8 - header_len;
            let mut tensors = 0usize;
            for (name, tensor) in &header {
                if name == "__metadata__" {
                    continue;
                }
                let Some(descriptor) = tensor.as_object() else {
                    return Ok(false);
                };
                if !descriptor.get("dtype").is_some_and(Value::is_string)
                    || !descriptor.get("shape").is_some_and(Value::is_array)
                {
                    return Ok(false);
                }
                let offsets = descriptor.get("data_offsets").and_then(Value::as_array);
                let Some(offsets) = offsets else { return Ok(false) };
                if offsets.len() != 2 {
                    return Ok(false);
                }
                let Some(start) = offsets[0].as_u64() else { return Ok(false) };
                let Some(end) = offsets[1].as_u64() else { return Ok(false) };
                if start > end || end > payload_size {
                    return Ok(false);
                }
                tensors += 1;
            }
            Ok(tensors > 0)
        }
        _ => Ok(false),
    }
}

/// Read only whitelisted config.json fields from the same approved model
/// directory. Never open prompts, histories, tokenizer files or arbitrary JSON.
fn read_selected_config(
    grant: &ApprovedModelScope,
    model_path: &Path,
    inventory: &mut DiscoveryInventory,
) -> Result<BTreeMap<String, String>, DiscoveryError> {
    let mut result = BTreeMap::new();
    let Some(parent) = model_path.parent() else { return Ok(result) };
    let config = parent.join("config.json");
    grant.ensure_active()?;
    let (canonical, bytes) = match checked_file(grant, &config, false) {
        Ok(entry) => entry,
        Err(DiscoveryWarningKind::Inaccessible) if !config.exists() => return Ok(result),
        Err(_) => {
            inventory.warn(DiscoveryWarningKind::MalformedMetadata, &config);
            return Ok(result);
        }
    };
    if bytes as usize > MAX_METADATA_BYTES {
        inventory.warn(DiscoveryWarningKind::MalformedMetadata, &config);
        return Ok(result);
    }
    let data = match bounded_read(grant, &canonical, MAX_METADATA_BYTES) {
        Ok(data) => data,
        Err(_) => {
            inventory.warn(DiscoveryWarningKind::MalformedMetadata, &config);
            return Ok(result);
        }
    };
    let value = match serde_json::from_slice::<Value>(&data) {
        Ok(value) => value,
        Err(_) => {
            inventory.warn(DiscoveryWarningKind::MalformedMetadata, &config);
            return Ok(result);
        }
    };
    for key in ["model_type", "torch_dtype", "quantization_method"] {
        if let Some(text) = value.get(key).and_then(Value::as_str) {
            result.insert(key.into(), text.chars().take(128).collect());
        }
    }
    if let Some(arch) = value.get("architectures").and_then(Value::as_array) {
        let values: Vec<_> = arch
            .iter()
            .take(8)
            .filter_map(Value::as_str)
            .map(|s| s.chars().take(96).collect::<String>())
            .collect();
        if !values.is_empty() {
            result.insert("architectures".into(), values.join(", "));
        }
    }
    Ok(result)
}

fn bounded_read(
    grant: &ApprovedModelScope,
    path: &Path,
    max_bytes: usize,
) -> Result<Vec<u8>, DiscoveryError> {
    grant.ensure_active()?;
    let mut result = Vec::new();
    File::open(path)?
        .take(max_bytes.saturating_add(1) as u64)
        .read_to_end(&mut result)?;
    grant.ensure_active()?;
    if result.len() > max_bytes {
        return Err(DiscoveryError::UnsafeScope);
    }
    Ok(result)
}

/// Ollama blobs have no extension. Only referenced model layers in approved,
/// bounded manifest JSONs count as candidates; other blobs are ignored.
/// The descriptor digest is not a computed digest or weight verification.
fn read_ollama_manifest(
    grant: &ApprovedModelScope,
    manifest: &Path,
    inventory: &mut DiscoveryInventory,
    seen: &mut BTreeSet<ModelId>,
) -> Result<(), DiscoveryError> {
    grant.ensure_active()?;
    let (manifest_file, size) = match checked_file(grant, manifest, false) {
        Ok(found) => found,
        Err(kind) => {
            inventory.warn(kind, manifest);
            return Ok(());
        }
    };
    if size == 0 || size > MAX_METADATA_BYTES as u64 {
        inventory.warn(DiscoveryWarningKind::MalformedMetadata, manifest);
        return Ok(());
    }
    let bytes = match bounded_read(grant, &manifest_file, MAX_METADATA_BYTES) {
        Ok(content) => content,
        Err(_) => {
            inventory.warn(DiscoveryWarningKind::Inaccessible, manifest);
            return Ok(());
        }
    };
    let value = match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) => value,
        Err(_) => {
            inventory.warn(DiscoveryWarningKind::MalformedMetadata, manifest);
            return Ok(());
        }
    };
    let Some(layers) = value.get("layers").and_then(Value::as_array) else {
        inventory.warn(DiscoveryWarningKind::MalformedMetadata, manifest);
        return Ok(());
    };
    let relative = manifest.strip_prefix(grant.canonical_root.join("manifests"));
    let Ok(relative) = relative else {
        inventory.warn(DiscoveryWarningKind::OutsideScope, manifest);
        return Ok(());
    };
    let segments: Vec<_> = relative.components().collect();
    // Expected: registry host / namespace / model name / tag (or deeper
    // registries), with no traversal or special path interpretation.
    if segments.len() < 3 {
        inventory.warn(DiscoveryWarningKind::MalformedMetadata, manifest);
        return Ok(());
    }
    let model_name = format!(
        "{}:{}",
        segments[segments.len() - 2].as_os_str().to_string_lossy(),
        segments[segments.len() - 1].as_os_str().to_string_lossy()
    );
    for layer in layers.iter().take(128) {
        grant.ensure_active()?;
        if layer.get("mediaType").and_then(Value::as_str)
            != Some("application/vnd.ollama.image.model")
        {
            continue;
        }
        let Some(digest) = layer.get("digest").and_then(Value::as_str) else {
            inventory.warn(DiscoveryWarningKind::MalformedMetadata, manifest);
            continue;
        };
        let Some(hex) = digest.strip_prefix("sha256:") else {
            inventory.warn(DiscoveryWarningKind::InvalidModelReference, manifest);
            continue;
        };
        if hex.len() != 64 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            inventory.warn(DiscoveryWarningKind::InvalidModelReference, manifest);
            continue;
        }
        let blob = grant
            .canonical_root
            .join("blobs")
            .join(format!("sha256-{}", hex.to_ascii_lowercase()));
        let (resolved, file_size) = match checked_file(grant, &blob, false) {
            Ok(file) => file,
            Err(kind) => {
                inventory.warn(kind, &blob);
                continue;
            }
        };
        let (owner, adapter) = grant.source.owner_and_adapter();
        let id = ModelId::Artifact {
            adapter,
            owner: owner.clone(),
            // The provider's manifest is the provenance reference. The
            // actual immutable blob location is stored separately.
            source_path: manifest.to_path_buf(),
            canonical_path: resolved.clone(),
        };
        let mut metadata = BTreeMap::new();
        metadata.insert("manifest_model".into(), model_name.clone());
        metadata.insert("declared_digest".into(), digest.into());
        if let Some(expected) = layer.get("size").and_then(Value::as_u64) {
            metadata.insert("declared_size".into(), expected.to_string());
            if expected != file_size {
                inventory.warn(DiscoveryWarningKind::MalformedMetadata, manifest);
            }
        }
        inventory.insert(
            DiscoveredModel {
                id,
                display_name: model_name.chars().take(128).collect(),
                location: resolved,
                provenance_path: manifest.to_path_buf(),
                owner,
                source: grant.source.clone(),
                format_hint: None,
                size_bytes: file_size,
                evidence: FileEvidence::UnverifiedCandidate,
                metadata,
            },
            seen,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
