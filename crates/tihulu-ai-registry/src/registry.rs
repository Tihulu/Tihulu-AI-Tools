//! Model references are not model loads, and are never file ownership transfers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModelOwner {
    JanDesktop,
    Ollama,
    HuggingFace,
    UserNamed(String),
    AiTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceAdapter {
    JanDesktop,
    OllamaApi,
    HuggingFaceCache,
    CustomFolder,
    LocalInferenceApi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Gguf,
    Onnx,
    Safetensors,
    Tflite,
    HailoHef,
    WhisperGgml,
}

impl FileFormat {
    /// An extension/name hint, not proof of format or compatible runtime.
    pub fn from_path(path: &Path) -> Option<Self> {
        let name = path.file_name()?.to_str()?.to_ascii_lowercase();
        let extension = path.extension()?.to_str()?.to_ascii_lowercase();
        match extension.as_str() {
            "gguf" => Some(Self::Gguf),
            "onnx" => Some(Self::Onnx),
            "safetensors" => Some(Self::Safetensors),
            "tflite" => Some(Self::Tflite),
            "hef" => Some(Self::HailoHef),
            "bin" if name.starts_with("ggml-") => Some(Self::WhisperGgml),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactObservation {
    /// Preserve the approved owner's path (including a HF snapshot alias).
    pub source_path: PathBuf,
    /// Canonical file target, confirmed inside the approved scope at scan time.
    pub canonical_path: PathBuf,
    pub owner: ModelOwner,
    pub adapter: SourceAdapter,
    pub format_hint: FileFormat,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointState {
    /// Configuration/fixture alone is not evidence a server is reachable.
    Declared,
    /// A separately authorized adapter reported a successful model listing.
    ListedByAuthorizedClient,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointObservation {
    pub base_url: String,
    pub served_model_id: String,
    pub owner: ModelOwner,
    pub adapter: SourceAdapter,
    pub state: EndpointState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelReference {
    File(ArtifactObservation),
    Endpoint(EndpointObservation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactValidation {
    /// No binary payload is read or checked by this crate.
    Unchecked,
    /// Reserved for a future trusted validator; inventory never sets it.
    Validated { validator: String, evidence: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeVerification {
    Unchecked,
    /// Reserved for a tested runtime on an actual target host.
    Verified {
        runtime: String,
        host: String,
        evidence: String,
    },
    Incompatible { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRecord {
    /// Stable *reference* key, not a SHA-256/content digest.
    pub id: String,
    pub owner: ModelOwner,
    pub reference: ModelReference,
    pub artifact_validation: ArtifactValidation,
    pub runtime_verification: RuntimeVerification,
    /// The source disappeared on a subsequent approved scan, but the record
    /// and source path are preserved instead of changing the foreign files.
    pub missing: bool,
}

impl ModelRecord {
    pub fn discovered_file(&self) -> bool {
        matches!(&self.reference, ModelReference::File(_)) && !self.missing
    }

    pub fn validated_model(&self) -> bool {
        matches!(&self.artifact_validation, ArtifactValidation::Validated { .. })
    }

    pub fn available_endpoint(&self) -> bool {
        matches!(
            &self.reference,
            ModelReference::Endpoint(EndpointObservation {
                state: EndpointState::ListedByAuthorizedClient,
                ..
            })
        ) && !self.missing
    }

    pub fn verified_runtime_compatibility(&self) -> bool {
        matches!(&self.runtime_verification, RuntimeVerification::Verified { .. })
    }
}

#[derive(Debug, Default)]
pub struct ModelRegistry {
    entries: BTreeMap<String, ModelRecord>,
    /// Collision-safe identity table: an ID hash is never treated as a digest.
    identities: BTreeMap<String, String>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Idempotent only for *identical provenance*. Same filename, different
    /// directory, owner or server remains a separate reference.
    pub fn register_file(&mut self, observation: ArtifactObservation) -> String {
        let identity = format!(
            "file-v1|{:?}|{:?}|{}|{}",
            observation.adapter,
            observation.owner,
            observation.source_path.display(),
            observation.canonical_path.display()
        );
        self.register(identity, observation.owner.clone(), ModelReference::File(observation))
    }

    pub fn register_endpoint(&mut self, observation: EndpointObservation) -> String {
        let identity = format!(
            "endpoint-v1|{:?}|{:?}|{}|{}",
            observation.adapter,
            observation.owner,
            observation.base_url,
            observation.served_model_id
        );
        self.register(
            identity,
            observation.owner.clone(),
            ModelReference::Endpoint(observation),
        )
    }

    fn register(
        &mut self,
        identity: String,
        owner: ModelOwner,
        reference: ModelReference,
    ) -> String {
        if let Some(id) = self.identities.get(&identity) {
            let record = self.entries.get_mut(id).expect("identity points to entry");
            record.reference = reference;
            record.missing = false;
            return id.clone();
        }
        // FNV-1a is an index key only. Resolve collisions by checking the
        // complete identity; never merge entries merely because hashes match.
        let digest = stable_reference_hash(identity.as_bytes());
        let mut id = format!("ref-v1-{digest:016x}");
        let mut suffix = 1u64;
        while self.entries.contains_key(&id) {
            id = format!("ref-v1-{digest:016x}-{suffix}");
            suffix += 1;
        }
        let record = ModelRecord {
            id: id.clone(),
            owner,
            reference,
            artifact_validation: ArtifactValidation::Unchecked,
            runtime_verification: RuntimeVerification::Unchecked,
            missing: false,
        };
        self.identities.insert(identity, id.clone());
        self.entries.insert(id.clone(), record);
        id
    }

    pub fn get(&self, id: &str) -> Option<&ModelRecord> {
        self.entries.get(id)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn values(&self) -> impl Iterator<Item = &ModelRecord> {
        self.entries.values()
    }

    /// Mark a missing path without deleting a registry record or file.
    pub fn mark_missing(&mut self, id: &str) -> bool {
        if let Some(record) = self.entries.get_mut(id) {
            record.missing = true;
            true
        } else {
            false
        }
    }
}

fn stable_reference_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Construct a location only. Never creates directories or changes files.
pub fn default_model_library(home: &Path) -> PathBuf {
    home.join("TIHULU AI Tools").join("Models")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(path: &str, owner: ModelOwner) -> ArtifactObservation {
        ArtifactObservation {
            source_path: PathBuf::from(path),
            canonical_path: PathBuf::from(path),
            owner,
            adapter: SourceAdapter::CustomFolder,
            format_hint: FileFormat::Gguf,
            size_bytes: 1,
        }
    }

    #[test]
    fn dedup_is_by_provenance_not_filename_or_unverified_hash() {
        let mut registry = ModelRegistry::new();
        let a = registry.register_file(observation("/one/model.gguf", ModelOwner::UserNamed("a".into())));
        let same = registry.register_file(observation("/one/model.gguf", ModelOwner::UserNamed("a".into())));
        let b = registry.register_file(observation("/two/model.gguf", ModelOwner::UserNamed("a".into())));
        let foreign = registry.register_file(observation("/one/model.gguf", ModelOwner::JanDesktop));
        assert_eq!(a, same);
        assert_ne!(a, b);
        assert_ne!(a, foreign);
        assert_eq!(registry.len(), 3);
    }

    #[test]
    fn changed_path_is_new_reference_and_missing_one_is_not_deleted() {
        let mut registry = ModelRegistry::new();
        let old = registry.register_file(observation("/old/model.gguf", ModelOwner::JanDesktop));
        assert!(registry.mark_missing(&old));
        let new = registry.register_file(observation("/new/model.gguf", ModelOwner::JanDesktop));
        assert_ne!(old, new);
        assert!(registry.get(&old).unwrap().missing);
        assert!(!registry.get(&new).unwrap().missing);
    }

    #[test]
    fn endpoint_listing_is_not_file_or_runtime_validation() {
        let mut registry = ModelRegistry::new();
        let id = registry.register_endpoint(EndpointObservation {
            base_url: "http://localhost:11434".into(),
            served_model_id: "chat:latest".into(),
            owner: ModelOwner::Ollama,
            adapter: SourceAdapter::OllamaApi,
            state: EndpointState::ListedByAuthorizedClient,
        });
        let model = registry.get(&id).unwrap();
        assert!(model.available_endpoint());
        assert!(!model.discovered_file());
        assert!(!model.validated_model());
        assert!(!model.verified_runtime_compatibility());
    }

    #[test]
    fn library_is_just_a_user_overridable_path() {
        let p = default_model_library(Path::new("/synthetic/home"));
        assert_eq!(p, PathBuf::from("/synthetic/home/TIHULU AI Tools/Models"));
    }
}
