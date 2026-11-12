//! Deterministic compatibility gate: metadata never implies successful inference.
use crate::manifest::{
    HardwareTarget, LanguagePair, License, Modality, ModelFormat, ModelManifest, ModelStatus,
    Runtime, TaskKind,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRequest {
    pub task: TaskKind,
    pub input: Modality,
    pub output: Modality,
    /// Languages required by the consumer adapter; exact case-insensitive tags.
    pub languages: Vec<String>,
    pub translation: Option<LanguagePair>,
    pub quantization: Option<String>,
    pub requires_tool_calling: bool,
    /// Internet-free use; an explicitly paired LAN host may still be eligible.
    pub offline_only: bool,
    /// Empty means any *known*, unrestricted license; unknown stays unknown.
    pub license_allowlist: Vec<String>,
}

/// An observation produced by a trusted local validation adapter, not by an
/// imported model card or AI suggestion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedRuntimeProbe {
    pub model_id: String,
    pub runtime: Runtime,
    pub hardware: HardwareTarget,
    pub evidence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompatibilityContext {
    pub hardware: HardwareTarget,
    pub runtime: Runtime,
    /// Explicitly measured or configured target budget; None is unknown.
    pub available_ram_mib: Option<u64>,
    pub available_vram_mib: Option<u64>,
    pub available_npu_mib: Option<u64>,
    pub hardware_verified: bool,
    pub runtime_ready: bool,
    pub consumer_adapter_verified: bool,
    pub paired_host_authorized: bool,
    /// Independent, trusted model-specific probes supplied by an authorized
    /// runtime adapter. Untrusted manifest claims cannot populate this field.
    pub trusted_probes: Vec<TrustedRuntimeProbe>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchVerdict {
    Recommended,
    Candidate,
    Incompatible,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub model_id: String,
    pub status: ModelStatus,
    pub verdict: MatchVerdict,
    /// Human-readable *deterministic* explanations, not LLM conclusions.
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PairSupport {
    Supported,
    Unsupported,
    Undetermined,
}

fn format_runtime(format: ModelFormat, runtime: Runtime) -> PairSupport {
    use ModelFormat as F;
    use Runtime as R;
    match (format, runtime) {
        (F::Gguf, R::LlamaCpp)
        | (F::WhisperGgml, R::WhisperCpp)
        | (F::Onnx, R::OnnxRuntime)
        | (F::Mlx, R::Mlx)
        | (F::Ctranslate2, R::Ctranslate2)
        | (F::HailoHef, R::HailoRt)
        | (F::Endpoint, R::JanApi | R::OllamaApi | R::OpenAiApi) => PairSupport::Supported,
        (F::Safetensors | F::Tflite | F::Unknown, _) | (_, R::Unknown) => PairSupport::Undetermined,
        _ => PairSupport::Unsupported,
    }
}

fn runtime_hardware(runtime: Runtime, hardware: HardwareTarget) -> PairSupport {
    use HardwareTarget as H;
    use Runtime as R;
    match (runtime, hardware) {
        (R::Unknown, _) => PairSupport::Undetermined,
        (R::HailoRt, H::Hailo)
        | (R::Mlx, H::AppleSilicon)
        | (
            R::LlamaCpp | R::WhisperCpp | R::OnnxRuntime | R::Ctranslate2,
            H::Cpu | H::Cuda | H::AppleSilicon | H::RaspberryPiCpu,
        )
        | (
            R::JanApi | R::OllamaApi | R::OpenAiApi,
            H::Cpu | H::Cuda | H::AppleSilicon | H::RaspberryPiCpu | H::PairedGpuHost,
        ) => PairSupport::Supported,
        _ => PairSupport::Unsupported,
    }
}

fn check_memory(
    label: &str,
    needed: Option<u64>,
    available: Option<u64>,
    incompatible: &mut Vec<String>,
    unknown: &mut Vec<String>,
) {
    match (needed, available) {
        (Some(n), Some(a)) if n > a => {
            incompatible.push(format!("{label} requires {n} MiB; only {a} MiB declared"));
        }
        (Some(_), Some(_)) => {}
        _ => unknown.push(format!(
            "{label} requirement or available budget is unverified"
        )),
    }
}

pub fn match_model(
    model: &ModelManifest,
    task: &TaskRequest,
    ctx: &CompatibilityContext,
) -> MatchResult {
    let mut incompatible = Vec::new();
    let mut unknown = Vec::new();

    if let Err(problem) = model.validate() {
        unknown.push(format!("invalid or untrusted manifest: {problem}"));
    }
    if !model.tasks.contains(&task.task) || model.input != task.input || model.output != task.output
    {
        incompatible.push("task or input/output modality not declared".to_string());
    }
    if !model.runtimes.contains(&ctx.runtime) {
        if model.runtimes.is_empty() {
            unknown.push("runtime capability not declared".into());
        } else {
            incompatible.push("selected runtime not declared for this artifact".into());
        }
    }
    if !model.hardware_targets.contains(&ctx.hardware) {
        if model.hardware_targets.is_empty() {
            unknown.push("target hardware support is unknown".into());
        } else {
            incompatible.push("selected hardware is not declared for the model".into());
        }
    }
    for (label, result) in [
        (
            "artifact format and runtime",
            format_runtime(model.format, ctx.runtime),
        ),
        (
            "runtime and hardware",
            runtime_hardware(ctx.runtime, ctx.hardware),
        ),
    ] {
        match result {
            PairSupport::Supported => {}
            PairSupport::Unsupported => incompatible.push(format!("{label} are incompatible")),
            PairSupport::Undetermined => unknown.push(format!("{label} are unverified")),
        }
    }
    for requested in &task.languages {
        if model.languages.is_empty() {
            unknown.push(format!("language {requested} is unverified"));
        } else if !model
            .languages
            .iter()
            .any(|known| known.eq_ignore_ascii_case(requested))
        {
            incompatible.push(format!("language {requested} is not supported"));
        }
    }
    if task.task == TaskKind::Translation {
        match &task.translation {
            None => unknown.push("a translation direction must be selected".into()),
            Some(_) if model.translation_pairs.is_empty() => {
                unknown.push("translation direction not verified".into());
            }
            Some(pair)
                if !model.translation_pairs.iter().any(|supported| {
                    supported.from.eq_ignore_ascii_case(&pair.from)
                        && supported.to.eq_ignore_ascii_case(&pair.to)
                }) =>
            {
                incompatible.push(format!(
                    "{} to {} translation not supported",
                    pair.from, pair.to
                ));
            }
            Some(_) => {}
        }
    }
    if task.requires_tool_calling && !model.supports_tool_calling {
        incompatible.push("tool-calling capability not declared".into());
    }
    if let Some(required) = &task.quantization {
        match &model.quantization {
            None => unknown.push("quantization is unknown".into()),
            Some(value) if !required.eq_ignore_ascii_case(value) => {
                incompatible.push(format!("required quantization {required} not present"));
            }
            Some(_) => {}
        }
    }
    match &model.license {
        License::Unknown => unknown.push("model license is unknown".into()),
        License::Restricted { reason } => {
            incompatible.push(format!("license is restricted: {reason}"))
        }
        License::Known { spdx }
            if !task.license_allowlist.is_empty()
                && !task
                    .license_allowlist
                    .iter()
                    .any(|id| id.eq_ignore_ascii_case(spdx)) =>
        {
            incompatible.push(format!("license {spdx} is not authorized by task policy"));
        }
        License::Known { .. } => {}
    }
    if task.offline_only && model.internet_required {
        incompatible.push("model needs Internet but task is offline-only".into());
    }
    if ctx.hardware == HardwareTarget::PairedGpuHost && !ctx.paired_host_authorized {
        incompatible.push("paired host is not authorized".into());
    }

    check_memory(
        "RAM",
        model.memory.ram_mib,
        ctx.available_ram_mib,
        &mut incompatible,
        &mut unknown,
    );
    match ctx.hardware {
        HardwareTarget::Cuda | HardwareTarget::PairedGpuHost => check_memory(
            "GPU VRAM",
            model.memory.vram_mib,
            ctx.available_vram_mib,
            &mut incompatible,
            &mut unknown,
        ),
        HardwareTarget::Hailo => check_memory(
            "Hailo NPU memory",
            model.memory.npu_mib,
            ctx.available_npu_mib,
            &mut incompatible,
            &mut unknown,
        ),
        // Apple GPUs have unified memory; Pi/CPU have no separate VRAM budget.
        HardwareTarget::AppleSilicon | HardwareTarget::RaspberryPiCpu | HardwareTarget::Cpu => {}
    }
    if !ctx.hardware_verified {
        unknown.push("hardware/accelerator readiness not verified".into());
    }
    if !ctx.runtime_ready {
        unknown.push("inference runtime not verified ready".into());
    }
    if !ctx.consumer_adapter_verified {
        unknown.push("application task adapter not verified".into());
    }

    let (verdict, status, reasons) = if !incompatible.is_empty() {
        (
            MatchVerdict::Incompatible,
            ModelStatus::Incompatible,
            incompatible,
        )
    } else if !unknown.is_empty() || model.availability == ModelStatus::Unknown {
        if unknown.is_empty() {
            unknown.push("market inventory status is unknown".into());
        }
        (MatchVerdict::Unknown, ModelStatus::Unknown, unknown)
    } else {
        let has_matching_test = model.verified_runtimes.iter().any(|record| {
            record.runtime == ctx.runtime
                && record.hardware == ctx.hardware
                && !record.evidence_id.trim().is_empty()
                && ctx.trusted_probes.iter().any(|probe| {
                    probe.model_id == model.id
                        && probe.runtime == ctx.runtime
                        && probe.hardware == ctx.hardware
                        && probe.evidence_id == record.evidence_id
                })
        });
        if !model.integrity_verified
            || !has_matching_test
            || model.availability == ModelStatus::Unverified
        {
            (
                MatchVerdict::Candidate,
                ModelStatus::Unverified,
                vec!["file integrity or runtime test on this exact target is unverified".into()],
            )
        } else if matches!(
            model.availability,
            ModelStatus::Installed | ModelStatus::External
        ) {
            (
                MatchVerdict::Recommended,
                model.availability,
                vec!["installed/reference-in-place; runtime and consumer contract verified".into()],
            )
        } else {
            (
                MatchVerdict::Candidate,
                model.availability,
                vec!["potential match only; explicit download/import and testing required".into()],
            )
        }
    };
    MatchResult {
        model_id: model.id.clone(),
        status,
        verdict,
        reasons,
    }
}

fn verdict_rank(verdict: MatchVerdict) -> u8 {
    match verdict {
        MatchVerdict::Recommended => 0,
        MatchVerdict::Candidate => 1,
        MatchVerdict::Unknown => 2,
        MatchVerdict::Incompatible => 3,
    }
}

fn availability_rank(status: ModelStatus) -> u8 {
    match status {
        ModelStatus::Installed => 0,
        ModelStatus::External => 1,
        ModelStatus::Unverified => 2,
        ModelStatus::Candidate => 3,
        ModelStatus::Downloadable => 4,
        ModelStatus::Unknown => 5,
        ModelStatus::Incompatible => 6,
    }
}

/// Stable, offline sorting. Verified installed/externally managed models win;
/// an incompatible installed model never displaces a compatible candidate.
pub fn rank_models(
    models: &[ModelManifest],
    task: &TaskRequest,
    context: &CompatibilityContext,
) -> Vec<MatchResult> {
    let mut scored: Vec<(u8, u8, MatchResult)> = models
        .iter()
        .map(|model| {
            let result = match_model(model, task, context);
            (
                verdict_rank(result.verdict),
                availability_rank(model.availability),
                result,
            )
        })
        .collect();
    scored.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| a.1.cmp(&b.1))
            .then_with(|| a.2.model_id.cmp(&b.2.model_id))
    });
    scored.into_iter().map(|(_, _, result)| result).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_runtime_never_confuses_hailo_and_cuda() {
        assert_eq!(
            format_runtime(ModelFormat::HailoHef, Runtime::LlamaCpp),
            PairSupport::Unsupported
        );
        assert_eq!(
            runtime_hardware(Runtime::HailoRt, HardwareTarget::Cuda),
            PairSupport::Unsupported
        );
        assert_eq!(
            runtime_hardware(Runtime::LlamaCpp, HardwareTarget::Hailo),
            PairSupport::Unsupported
        );
        assert_eq!(
            format_runtime(ModelFormat::Safetensors, Runtime::LlamaCpp),
            PairSupport::Undetermined
        );
    }
}
