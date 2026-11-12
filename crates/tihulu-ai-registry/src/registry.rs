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

/// Stable, typed *source-reference* identity. Never a content hash, GPU
/// resource handle, runtime instance, authorization token or public URL.
///
/// Full path components are retained without lossy UTF-8 conversion. These
/// identities can disclose private paths and must stay in a private registry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ModelId {
    Artifact {
        adapter: SourceAdapter,
        owner: ModelOwner,
        source_path: PathBuf,
        canonical_path: PathBuf,
    },
    Endpoint {
        adapter: SourceAdapter,
        owner: ModelOwner,
        base_url: String,
        served_model_id: String,
    },
}

impl ModelId {
    pub fn from_artifact(value: &ArtifactObservation) -> Self {
        Self::Artifact {
            adapter: value.adapter,
            owner: value.owner.clone(),
            source_path: value.source_path.clone(),
            canonical_path: value.canonical_path.clone(),
        }
    }

    pub fn from_endpoint(value: &EndpointObservation) -> Self {
        Self::Endpoint {
            adapter: value.adapter,
            owner: value.owner.clone(),
            base_url: value.base_url.clone(),
            served_model_id: value.served_model_id.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRecord {
    /// Typed provenance identity; never a running ResourceLedger ResourceId.
    pub id: ModelId,
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
    entries: BTreeMap<ModelId, ModelRecord>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Re-registering exactly the same source/path preserves the typed ID.
    /// Different provenance, path or endpoint IDs are distinct, regardless of
    /// filename or unverified size. No content hash is fabricated.
    pub fn register_file(&mut self, observation: ArtifactObservation) -> ModelId {
        let id = ModelId::from_artifact(&observation);
        self.register(id, observation.owner.clone(), ModelReference::File(observation))
    }

    pub fn register_endpoint(&mut self, observation: EndpointObservation) -> ModelId {
        let id = ModelId::from_endpoint(&observation);
        self.register(
            id,
            observation.owner.clone(),
            ModelReference::Endpoint(observation),
        )
    }

    fn register(
        &mut self,
        id: ModelId,
        owner: ModelOwner,
        reference: ModelReference,
    ) -> ModelId {
        use std::collections::btree_map::Entry;
        match self.entries.entry(id.clone()) {
            Entry::Occupied(mut occupied) => {
                let record = occupied.get_mut();
                record.reference = reference;
                record.missing = false;
            }
            Entry::Vacant(vacant) => {
                vacant.insert(ModelRecord {
                    id: id.clone(),
                    owner,
                    reference,
                    artifact_validation: ArtifactValidation::Unchecked,
                    runtime_verification: RuntimeVerification::Unchecked,
                    missing: false,
                });
            }
        }
        id
    }

    pub fn get(&self, id: &ModelId) -> Option<&ModelRecord> {
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

    /// Mark missing without deleting the registry record or external file.
    pub fn mark_missing(&mut self, id: &ModelId) -> bool {
        if let Some(record) = self.entries.get_mut(id) {
            record.missing = true;
            true
        } else {
            false
        }
    }
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
    #[test]
    fn identity_is_typed_and_independent_of_registration_order() {
        let a = observation("/fixtures/one.gguf", ModelOwner::JanDesktop);
        let b = observation("/fixtures/two.gguf", ModelOwner::Ollama);
        let mut first = ModelRegistry::new();
        let one = first.register_file(a.clone());
        first.register_file(b.clone());
        let mut reverse = ModelRegistry::new();
        reverse.register_file(b);
        let two = reverse.register_file(a);
        assert_eq!(one, two);
        assert!(matches!(one, ModelId::Artifact { .. }));
    }

    #[test]
    fn model_identity_does_not_claim_binary_or_runtime_validation() {
        let artifact = observation("/fixtures/model.gguf", ModelOwner::JanDesktop);
        let id = ModelId::from_artifact(&artifact);
        let mut registry = ModelRegistry::new();
        assert_eq!(registry.register_file(artifact), id);
        let entry = registry.get(&id).unwrap();
        assert!(entry.discovered_file());
        assert!(!entry.validated_model());
        assert!(!entry.verified_runtime_compatibility());
    }

}
