use tihulu_ai_market::{
    match_model, rank_models, validate_advisor_proposal, AdvisorPolicy, AdvisorProposal, Citation,
    CompatibilityContext, HardwareTarget as H, LanguagePair, License, MatchVerdict,
    MemoryRequirements, Modality as M, ModelFormat as F, ModelManifest, ModelSource,
    ModelStatus as S, ProposalError, Runtime as R, RuntimeVerification, TaskKind as T, TaskRequest,
    TrustedRuntimeProbe,
};

fn chat() -> ModelManifest {
    ModelManifest::from_json(include_str!("fixtures/installed-chat.json")).unwrap()
}

fn task(kind: T, input: M, output: M) -> TaskRequest {
    TaskRequest {
        task: kind,
        input,
        output,
        languages: vec!["en".into()],
        translation: None,
        quantization: None,
        requires_tool_calling: false,
        offline_only: true,
        license_allowlist: vec!["Apache-2.0".into()],
    }
}

fn ctx(model: &ModelManifest, runtime: R, hardware: H) -> CompatibilityContext {
    CompatibilityContext {
        hardware,
        runtime,
        available_ram_mib: Some(8192),
        available_vram_mib: Some(8192),
        available_npu_mib: Some(8192),
        hardware_verified: true,
        runtime_ready: true,
        consumer_adapter_verified: true,
        paired_host_authorized: true,
        trusted_probes: vec![TrustedRuntimeProbe {
            model_id: model.id.clone(),
            runtime,
            hardware,
            evidence_id: "synthetic-fixture-probe".into(),
        }],
    }
}

fn change_runtime(model: &mut ModelManifest, runtime: R, hardware: H) {
    model.runtimes = vec![runtime];
    model.hardware_targets = vec![hardware];
    model.verified_runtimes = vec![RuntimeVerification {
        runtime,
        hardware,
        evidence_id: "synthetic-fixture-probe".into(),
    }];
}

#[test]
fn manifest_serialization_is_offline_and_roundtrips() {
    let m = chat();
    let encoded = m.to_json_pretty().unwrap();
    assert_eq!(ModelManifest::from_json(&encoded).unwrap(), m);
    assert!(!encoded.contains("/home/"));
}

#[test]
fn installed_and_external_verified_models_precede_downloads() {
    let installed = chat();
    let mut external = chat();
    external.id = "fixture:jan-ref".into();
    external.source = ModelSource::External {
        application: "Jan Desktop".into(),
    };
    external.availability = S::External;
    let mut downloadable = chat();
    downloadable.id = "fixture:market-download".into();
    downloadable.source = ModelSource::Market;
    downloadable.availability = S::Downloadable;
    let mut environment = ctx(&installed, R::LlamaCpp, H::Cpu);
    environment.trusted_probes.push(TrustedRuntimeProbe {
        model_id: external.id.clone(),
        runtime: R::LlamaCpp,
        hardware: H::Cpu,
        evidence_id: "synthetic-fixture-probe".into(),
    });
    let models = vec![downloadable, external, installed];
    let results = rank_models(&models, &task(T::Chat, M::Text, M::Text), &environment);
    assert_eq!(results[0].model_id, "fixture:owned-chat-q4");
    assert_eq!(results[0].status, S::Installed);
    assert_eq!(results[1].status, S::External);
    assert_eq!(results[2].status, S::Unverified);
    assert_eq!(results[2].verdict, MatchVerdict::Candidate);
}

#[test]
fn self_asserted_probe_cannot_produce_recommended_status() {
    let model = chat();
    let mut environment = ctx(&model, R::LlamaCpp, H::Cpu);
    environment.trusted_probes.clear();
    let result = match_model(&model, &task(T::Chat, M::Text, M::Text), &environment);
    assert_eq!(result.status, S::Unverified);
    assert_eq!(result.verdict, MatchVerdict::Candidate);
    environment.hardware_verified = false;
    assert_eq!(
        match_model(&model, &task(T::Chat, M::Text, M::Text), &environment).status,
        S::Unknown
    );
}

#[test]
fn chat_is_not_rag_embedding_without_distinct_embedding_capability() {
    let model = chat();
    let environment = ctx(&model, R::LlamaCpp, H::Cpu);
    let result = match_model(
        &model,
        &task(T::RagEmbedding, M::Text, M::Embedding),
        &environment,
    );
    assert_eq!(result.verdict, MatchVerdict::Incompatible);
    let mut embedding = chat();
    embedding.id = "fixture:rag-embedding".into();
    embedding.tasks = vec![T::RagEmbedding];
    embedding.output = M::Embedding;
    let mut environment = ctx(&embedding, R::LlamaCpp, H::Cpu);
    assert_eq!(
        match_model(
            &embedding,
            &task(T::RagEmbedding, M::Text, M::Embedding),
            &environment
        )
        .verdict,
        MatchVerdict::Recommended
    );
    environment.consumer_adapter_verified = false;
    assert_eq!(
        match_model(
            &embedding,
            &task(T::RagEmbedding, M::Text, M::Embedding),
            &environment
        )
        .status,
        S::Unknown
    );
}

#[test]
fn media_text_asr_requires_audio_and_real_whisper_runtime() {
    let mut model = chat();
    model.id = "fixture:asr".into();
    model.format = F::WhisperGgml;
    model.input = M::Audio;
    model.output = M::Text;
    model.tasks = vec![T::Asr];
    change_runtime(&mut model, R::WhisperCpp, H::Cpu);
    let environment = ctx(&model, R::WhisperCpp, H::Cpu);
    assert_eq!(
        match_model(&model, &task(T::Asr, M::Audio, M::Text), &environment).verdict,
        MatchVerdict::Recommended
    );
    assert_eq!(
        match_model(&model, &task(T::Asr, M::Text, M::Text), &environment).verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn translation_must_verify_language_direction_not_just_tags() {
    let mut model = chat();
    model.id = "fixture:translation".into();
    model.format = F::Ctranslate2;
    model.tasks = vec![T::Translation];
    model.translation_pairs = vec![LanguagePair {
        from: "tr".into(),
        to: "de".into(),
    }];
    change_runtime(&mut model, R::Ctranslate2, H::Cpu);
    let environment = ctx(&model, R::Ctranslate2, H::Cpu);
    let mut request = task(T::Translation, M::Text, M::Text);
    request.languages = vec!["TR".into(), "DE".into()];
    request.translation = Some(LanguagePair {
        from: "TR".into(),
        to: "de".into(),
    });
    assert_eq!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Recommended
    );
    request.translation = Some(LanguagePair {
        from: "de".into(),
        to: "tr".into(),
    });
    assert_eq!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn satellite_vision_requires_image_modality_and_matching_runtime() {
    let mut model = chat();
    model.id = "fixture:satellite-vision".into();
    model.format = F::Onnx;
    model.tasks = vec![T::Vision];
    model.input = M::Image;
    change_runtime(&mut model, R::OnnxRuntime, H::RaspberryPiCpu);
    model.memory.ram_mib = Some(1024);
    let environment = ctx(&model, R::OnnxRuntime, H::RaspberryPiCpu);
    assert_eq!(
        match_model(&model, &task(T::Vision, M::Image, M::Text), &environment).verdict,
        MatchVerdict::Recommended
    );
    assert_eq!(
        match_model(&model, &task(T::Vision, M::Audio, M::Text), &environment).verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn tool_calling_is_a_distinct_verified_capability() {
    let mut model = chat();
    model.tasks.push(T::ToolCalling);
    let environment = ctx(&model, R::LlamaCpp, H::Cpu);
    let mut request = task(T::ToolCalling, M::Text, M::Text);
    request.requires_tool_calling = true;
    assert_eq!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Incompatible
    );
    model.supports_tool_calling = true;
    assert_eq!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Recommended
    );
}

#[test]
fn hailo_hef_is_not_cuda_gguf_nor_nvidia_vram() {
    let mut model = chat();
    model.id = "fixture:vision-hailo".into();
    model.format = F::HailoHef;
    model.tasks = vec![T::Vision];
    model.input = M::Image;
    model.memory = MemoryRequirements {
        ram_mib: Some(1024),
        vram_mib: None,
        npu_mib: Some(4096),
    };
    change_runtime(&mut model, R::HailoRt, H::Hailo);
    let environment = ctx(&model, R::HailoRt, H::Hailo);
    assert_eq!(
        match_model(&model, &task(T::Vision, M::Image, M::Text), &environment).verdict,
        MatchVerdict::Recommended
    );
    let mut cuda = ctx(&model, R::HailoRt, H::Cuda);
    cuda.trusted_probes = vec![TrustedRuntimeProbe {
        model_id: model.id.clone(),
        runtime: R::HailoRt,
        hardware: H::Cuda,
        evidence_id: "synthetic-fixture-probe".into(),
    }];
    assert_eq!(
        match_model(&model, &task(T::Vision, M::Image, M::Text), &cuda).status,
        S::Incompatible
    );
    let mut npu_starved = environment;
    npu_starved.available_npu_mib = Some(2048);
    assert_eq!(
        match_model(&model, &task(T::Vision, M::Image, M::Text), &npu_starved).verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn apple_silicon_mlx_and_raspberry_pi_cpu_are_distinct_targets() {
    let mut model = chat();
    model.id = "fixture:apple-mlx".into();
    model.format = F::Mlx;
    model.memory = MemoryRequirements {
        ram_mib: Some(3072),
        vram_mib: None,
        npu_mib: None,
    };
    change_runtime(&mut model, R::Mlx, H::AppleSilicon);
    assert_eq!(
        match_model(
            &model,
            &task(T::Chat, M::Text, M::Text),
            &ctx(&model, R::Mlx, H::AppleSilicon)
        )
        .verdict,
        MatchVerdict::Recommended
    );
    assert_eq!(
        match_model(
            &model,
            &task(T::Chat, M::Text, M::Text),
            &ctx(&model, R::Mlx, H::RaspberryPiCpu)
        )
        .verdict,
        MatchVerdict::Incompatible
    );
}

#[test]
fn paired_gpu_host_requires_explicit_authorized_endpoint() {
    let mut model = chat();
    model.id = "fixture:paired-endpoint".into();
    model.source = ModelSource::External {
        application: "Paired Host".into(),
    };
    model.availability = S::External;
    model.format = F::Endpoint;
    model.memory = MemoryRequirements {
        ram_mib: Some(0),
        vram_mib: Some(4096),
        npu_mib: None,
    };
    change_runtime(&mut model, R::OpenAiApi, H::PairedGpuHost);
    let mut environment = ctx(&model, R::OpenAiApi, H::PairedGpuHost);
    assert_eq!(
        match_model(&model, &task(T::Chat, M::Text, M::Text), &environment).status,
        S::External
    );
    environment.paired_host_authorized = false;
    assert_eq!(
        match_model(&model, &task(T::Chat, M::Text, M::Text), &environment).status,
        S::Incompatible
    );
}

#[test]
fn memory_quantization_license_and_internet_policy_are_hard_gates() {
    let mut model = chat();
    let mut request = task(T::Chat, M::Text, M::Text);
    let mut environment = ctx(&model, R::LlamaCpp, H::Cpu);
    environment.available_ram_mib = Some(2048);
    assert_eq!(
        match_model(&model, &request, &environment).status,
        S::Incompatible
    );
    environment.available_ram_mib = None;
    assert_eq!(
        match_model(&model, &request, &environment).status,
        S::Unknown
    );
    environment.available_ram_mib = Some(8192);
    request.quantization = Some("Q8_0".into());
    assert_eq!(
        match_model(&model, &request, &environment).status,
        S::Incompatible
    );
    request.quantization = None;
    model.license = License::Unknown;
    assert_eq!(
        match_model(&model, &request, &environment).status,
        S::Unknown
    );
    model.license = License::Restricted {
        reason: "not permitted".into(),
    };
    assert_eq!(
        match_model(&model, &request, &environment).status,
        S::Incompatible
    );
    model.license = License::Known {
        spdx: "Apache-2.0".into(),
    };
    model.internet_required = true;
    assert_eq!(
        match_model(&model, &request, &environment).status,
        S::Incompatible
    );
}

#[test]
fn proposed_high_confidence_ai_match_never_overrides_rust_gate() {
    let model = chat();
    let mut environment = ctx(&model, R::LlamaCpp, H::Cuda);
    environment.available_vram_mib = Some(16000);
    let mut suggestion = AdvisorProposal {
        model_id: model.id.clone(),
        task: T::Chat,
        explanation: "Unverified vendor claim".into(),
        confidence_pct: 99,
        citations: vec![Citation {
            title: "Synthetic model card".into(),
            url: "https://example.org/model-card".into(),
            supported_claim: "Claims a model is suitable".into(),
        }],
        internet_used: true,
    };
    let request = task(T::Chat, M::Text, M::Text);
    assert_eq!(
        validate_advisor_proposal(
            &suggestion,
            &AdvisorPolicy::default(),
            &[model.clone()],
            &request,
            &environment
        ),
        Err(ProposalError::InternetNotAuthorized)
    );
    let policy = AdvisorPolicy {
        internet_opt_in: true,
    };
    let accepted =
        validate_advisor_proposal(&suggestion, &policy, &[model], &request, &environment).unwrap();
    assert_eq!(
        accepted.deterministic_match.verdict,
        MatchVerdict::Incompatible
    );
    assert!(!accepted.executable);
    suggestion.citations.clear();
    assert_eq!(
        validate_advisor_proposal(&suggestion, &policy, &[chat()], &request, &environment),
        Err(ProposalError::MissingCitations)
    );
}

#[test]
fn no_download_is_ever_presented_as_an_installed_model() {
    let mut model = chat();
    model.source = ModelSource::Market;
    model.availability = S::Downloadable;
    let environment = ctx(&model, R::LlamaCpp, H::Cpu);
    let outcome = match_model(&model, &task(T::Chat, M::Text, M::Text), &environment);
    assert_eq!(outcome.verdict, MatchVerdict::Candidate);
    assert_eq!(outcome.status, S::Downloadable);
}

#[test]
fn malformed_or_overlarge_catalog_data_cannot_be_recommended() {
    assert!(ModelManifest::from_json(&" ".repeat(65_537)).is_err());
    let mut model = chat();
    model.id = "x".repeat(513);
    assert!(model.validate().is_err());
    model.id = "fixture:bounded".into();
    model.languages = vec!["en".into(); 129];
    assert!(model.validate().is_err());
    model.languages = vec!["en".into()];
    model.source = ModelSource::External {
        application: "Jan Desktop".into(),
    };
    model.availability = S::Installed;
    assert!(model.validate().is_err());
}

#[test]
fn conflicting_or_unsubstantiated_license_claims_fail_closed() {
    let mut model = chat();
    let environment = ctx(&model, R::LlamaCpp, H::Cpu);
    let mut request = task(T::Chat, M::Text, M::Text);
    request.license_allowlist = vec!["MIT".into()];
    assert_eq!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Incompatible
    );
    model.license = License::Known {
        spdx: "NOASSERTION".into(),
    };
    assert!(model.validate().is_err());
    assert_ne!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Recommended
    );
    model.license = License::Unknown;
    assert_eq!(
        match_model(&model, &request, &environment).verdict,
        MatchVerdict::Unknown
    );
}
