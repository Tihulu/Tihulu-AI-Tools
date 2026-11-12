# Model Market contracts — v1 (proposed integration)

**Engineering state:** Manifest serialization and pure validation are implemented in the isolated Rust crate. Cross-crate registry ingestion and AI research remain proposed / experimental.

## Manifest v1

A strict JSON record corresponding to `ModelManifest` in `src/manifest.rs`. There are no implicit defaults for missing security-relevant properties. See [the synthetic fixture](../../crates/tihulu-ai-market/tests/fixtures/installed-chat.json). Key fields:

| Field | Meaning |
| --- | --- |
| `schema_version` | Exactly `1` in this crate; fail closed on future incompatible versions |
| `id`, `display_name` | Stable registry identity and visible label, not arbitrary filename inference |
| `source`, `availability` | TIHULU-owned, externally owned (e.g. Jan), or market-only; installed/external/downloadable/candidate/unknown/unverified |
| `format` | GGUF, Whisper GGML, ONNX, MLX, CTranslate2, HEF, served endpoint, safetensors, unknown |
| `runtimes`, `hardware_targets` | Explicit declared support; no implicit GPU/ARM64 compatibility |
| `tasks`, `input`, `output` | Task **and** modality contract |
| `languages`, `translation_pairs` | EN/TR/DE and any other explicit BCP-47 tags; directional pairs for translation |
| `quantization`, `memory` | Exact quantization, target RAM MiB, GPU VRAM MiB, Hailo NPU MiB (optional = unknown) |
| `license` | Known SPDX ID, unknown, or restricted |
| `integrity_verified`, `verified_runtimes` | Inventory claims, **not independently trusted by themselves** |
| `supports_tool_calling`, `internet_required` | Extra hard task/policy constraints |

Input may come from a versioned offline catalog or a user-authorized registry snapshot; importing JSON does not scan any path or allow model execution. Never store API keys, chat histories, local private paths or secrets in public catalog examples. Preserve external model ownership and reference it in place.

### Verification provenance boundary

The compatibility engine requires an independent `CompatibilityContext.trusted_probes` entry with matching model ID + runtime + hardware + evidence ID. It also requires `integrity_verified`, adapter readiness, runtime readiness, hardware readiness, and budget checks. **This crate does not create trusted probes.** Only a separately authorized trusted registry/runtime adapter can issue one after testing the exact version. A model card, path extension, endpoint name, or LLM advisor cannot issue its own trusted probe. In the first PR, test probes are marked `synthetic-fixture-probe`.

Future coordination must specify authenticated probe issuance, model artifact content digests, endpoint version identifiers, revocation, test expiration, and actual observed hardware resources. Until then, `Recommended` remains a static preflight verdict, never an executable grant.

## Compatibility and ranking semantics

1. Reject manifest contradictions and explicit hard mismatches with deterministic explanations. Known disallowed licenses or absent task capabilities are incompatible, not "slow".
2. If evidence is incomplete (hardware/runtime readiness, available RAM, a language or translation direction, untrusted catalog claims), show `unknown` or `unverified`; **never** upgrade missing proof.
3. An installed model gets `recommended` only when the matching verified runtime and app adapter exist; externally owned installed models get the same status while retaining their external ownership.
4. Sort by verdict then by availability: verified installed > verified external > static candidate; incompatible choices remain last. Stable model-ID sorting resolves ties.
5. Downloadable models remain candidates until the separate authorized downloader/integrity/registry workflow succeeds. Partial downloads and failed previews must not be registered as installed.

**Safety note:** Format mapping here is conservative. HEF/HailoRT never implies compatibility with CUDA, llama.cpp or GGUF. `safetensors` alone does not prove the engine or task. API endpoints require explicit format `endpoint`; they are not local model files.

## Experimental Internet / AI matching advisor

The advisor accepts only a proposal payload:

`AdvisorProposal { model_id, task, explanation, confidence_pct, citations, internet_used }`

- Network **OFF by default**. If `internet_used = true`, an explicit `AdvisorPolicy { internet_opt_in: true }` must be passed for that request. No HTTP client is provided in this crate.
- Each claim needs a non-empty cited title, `https://` URL and supported claim description. The crate validates citation *shape*, not content authenticity. Future research adapters must use source-backed versioned model cards/docs and distinguish benchmark from promotional claims.
- `confidence_pct` must be 0–100. Display it as *advisor confidence*, not validated runtime probability. Confident guesses never override deterministic checks.
- `validate_advisor_proposal` re-evaluates the requested model and task using Rust. It always returns `executable = false`; any later import, model test, assignment, settings write or download must request independent explicit user approval and run fresh validation.
- Only user-approved, minimal model/task metadata should be sent to an optional remote advisor; **never** share private files, absolute paths, tokens or local prompts without an additional scope-specific grant.
- No background Internet polling, automatic recommendations from unverified web claims, autonomous app changes or automated downloading.

## Coordinator integration proposals (not edits in this PR)

- Agent 02 registry provides normalized `ModelManifest` plus source ownership and content identity without moving stored models.
- Coordinator-owned core provides a trusted, versioned hardware/runtime readiness provider and permission checks. Evidence is bound to exact artifact+runtime+target and passed independently of imported manifests.
- Agent 04 paired GPU host provides authenticated endpoint information and truly client-scoped model leases; a remotely served API is not the same as local CUDA.
- Agent 05 provides verified app/task adapters; "recommended" alone cannot write `aipath.txt` or call Jan.
- Agent 06 renders localized EN (default), TR and DE statuses; incompatible/unknown/unverified clearly differentiated, "Already installed" before "Find/download", and cancel/Quit progress.
- Coordinator adds `crates/tihulu-ai-market` to root workspace only after review. Remove this crate's temporary local `[workspace]` marker in the same coordinated edit.
