//! Versioned, typed, untrusted model metadata. JSON parsing has no side effects.

use serde::{Deserialize, Serialize};

pub const MANIFEST_SCHEMA_VERSION: u32 = 1;
/// Reject oversized, untrusted catalog entries before JSON allocation.
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    Installed,
    External,
    Downloadable,
    Candidate,
    Incompatible,
    Unknown,
    Unverified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelSource {
    Tihulu,
    External { application: String },
    Market,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Gguf,
    WhisperGgml,
    Onnx,
    Mlx,
    Ctranslate2,
    HailoHef,
    Endpoint,
    Safetensors,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Runtime {
    LlamaCpp,
    WhisperCpp,
    OnnxRuntime,
    Mlx,
    Ctranslate2,
    HailoRt,
    JanApi,
    OllamaApi,
    OpenAiApi,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareTarget {
    Cpu,
    Cuda,
    AppleSilicon,
    RaspberryPiCpu,
    Hailo,
    PairedGpuHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Text,
    Audio,
    Image,
    Embedding,
    RadioSignal,
    StructuredData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Chat,
    RagEmbedding,
    Asr,
    Translation,
    Vision,
    ToolCalling,
    SignalClassification,
    FlightExplanation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum License {
    Known { spdx: String },
    Unknown,
    Restricted { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LanguagePair {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryRequirements {
    /// Host RAM; use Some(0) for a served-only endpoint.
    pub ram_mib: Option<u64>,
    /// Device VRAM on NVIDIA or a paired GPU host (not Apple unified memory).
    pub vram_mib: Option<u64>,
    /// Dedicated Hailo accelerator memory (not CUDA VRAM).
    pub npu_mib: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeVerification {
    pub runtime: Runtime,
    pub hardware: HardwareTarget,
    /// An opaque identifier for an actual successful validation; not a claim
    /// that tests ran merely because a model file exists.
    pub evidence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub id: String,
    pub display_name: String,
    pub source: ModelSource,
    /// Inventory/market state; final compatibility state is computed separately.
    pub availability: ModelStatus,
    pub format: ModelFormat,
    pub runtimes: Vec<Runtime>,
    pub hardware_targets: Vec<HardwareTarget>,
    pub tasks: Vec<TaskKind>,
    pub input: Modality,
    pub output: Modality,
    /// Explicit BCP-47 language tags. Empty means unknown, NOT all languages.
    pub languages: Vec<String>,
    /// Required for translation; a language list alone proves no direction.
    pub translation_pairs: Vec<LanguagePair>,
    /// An exact declared quantization identifier, e.g. Q4_K_M.
    pub quantization: Option<String>,
    pub memory: MemoryRequirements,
    pub license: License,
    pub integrity_verified: bool,
    pub verified_runtimes: Vec<RuntimeVerification>,
    pub supports_tool_calling: bool,
    pub internet_required: bool,
}

impl ModelManifest {
    pub fn from_json(input: &str) -> Result<Self, String> {
        if input.len() > MAX_MANIFEST_BYTES {
            return Err("manifest JSON exceeds the 64 KiB limit".into());
        }
        let manifest: Self = serde_json::from_str(input).map_err(|e| e.to_string())?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_json_pretty(&self) -> Result<String, String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != MANIFEST_SCHEMA_VERSION {
            return Err(format!(
                "unsupported schema version {}",
                self.schema_version
            ));
        }
        if self.id.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err("model id and display name must be nonempty".into());
        }
        if self.id.len() > 512 || self.display_name.len() > 256 {
            return Err("model identifier or display name is too long".into());
        }
        if self.runtimes.len() > 64
            || self.hardware_targets.len() > 32
            || self.tasks.len() > 64
            || self.languages.len() > 128
            || self.translation_pairs.len() > 128
            || self.verified_runtimes.len() > 128
        {
            return Err("manifest contains too many capabilities or evidence entries".into());
        }
        if matches!(self.availability, ModelStatus::Incompatible) {
            return Err("incompatible is a computed matching result, not inventory".into());
        }
        match (&self.source, self.availability) {
            (ModelSource::External { application }, ModelStatus::External)
                if !application.trim().is_empty() => {}
            (ModelSource::Tihulu, ModelStatus::Installed)
            | (ModelSource::Market, ModelStatus::Downloadable)
            | (_, ModelStatus::Candidate | ModelStatus::Unknown | ModelStatus::Unverified) => {}
            _ => return Err("source and availability contradict each other".into()),
        }
        if let License::Known { spdx } = &self.license {
            if spdx.trim().is_empty()
                || spdx.eq_ignore_ascii_case("unknown")
                || spdx.eq_ignore_ascii_case("noassertion")
                || spdx.eq_ignore_ascii_case("none")
            {
                return Err("known license requires an actual, declared SPDX identifier".into());
            }
        }
        for pair in &self.translation_pairs {
            if pair.from.trim().is_empty()
                || pair.to.trim().is_empty()
                || pair.from.len() > 64
                || pair.to.len() > 64
            {
                return Err("translation pairs must have both language tags".into());
            }
        }
        for verification in &self.verified_runtimes {
            if verification.evidence_id.trim().is_empty()
                || !self.runtimes.contains(&verification.runtime)
                || !self.hardware_targets.contains(&verification.hardware)
            {
                return Err("runtime verification must reference declared runtime and hardware with evidence".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_manifest_and_unknown_fields_are_rejected() {
        assert!(ModelManifest::from_json("{}").is_err());
        let example = r#"{"schema_version":1,"id":"x","unknown_field":true}"#;
        assert!(ModelManifest::from_json(example).is_err());
    }

    #[test]
    fn external_state_cannot_claim_owned_installation() {
        let mut manifest = ModelManifest {
            schema_version: 1,
            id: "example".into(),
            display_name: "Example".into(),
            source: ModelSource::External {
                application: "Jan".into(),
            },
            availability: ModelStatus::Installed,
            format: ModelFormat::Gguf,
            runtimes: vec![Runtime::LlamaCpp],
            hardware_targets: vec![HardwareTarget::Cpu],
            tasks: vec![TaskKind::Chat],
            input: Modality::Text,
            output: Modality::Text,
            languages: vec!["en".into()],
            translation_pairs: vec![],
            quantization: None,
            memory: MemoryRequirements::default(),
            license: License::Unknown,
            integrity_verified: false,
            verified_runtimes: vec![],
            supports_tool_calling: false,
            internet_required: false,
        };
        assert!(manifest.validate().is_err());
        manifest.availability = ModelStatus::External;
        assert!(manifest.validate().is_ok());
    }
}
