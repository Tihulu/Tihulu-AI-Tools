# Model Market and compatibility engine — Workstream 03

**Status:** Implemented as an independent, deterministic Rust foundation on `agent/03-market`. **Not** a production model installer, compatibility benchmark, inference runner, GPU host, UI, or deployed adapter. Architectural contracts for integrating with Agent 02's registry and the coordinator's core are **proposed**. Internet/AI advisor is **experimental**.

## Scope and running the crate

- Code: `crates/tihulu-ai-market/` (no production I/O on import).
- Tests: in-crate unit tests and `tests/compatibility.rs` with synthetic fixtures.
- Rust 2021, MSRV 1.82, dependency on `serde` and `serde_json` **only**.
- Build and test (when a Rust toolchain is available):

```sh
cargo fmt --manifest-path crates/tihulu-ai-market/Cargo.toml --check
cargo test --manifest-path crates/tihulu-ai-market/Cargo.toml --all-targets
cargo clippy --manifest-path crates/tihulu-ai-market/Cargo.toml --all-targets -- -D warnings
```

The owned crate has a local `[workspace]` table so it can be tested without changing the coordinator-owned root `Cargo.toml`. Before integration, the coordinator should add `crates/tihulu-ai-market` as a root member **and remove the nested `[workspace]` entry** in a coordinated change. Root-workspace CI currently does **not** exercise this isolated crate. This PR must not claim CI coverage until the standalone test command actually runs.

## What is implemented

1. A strict, versioned **in-memory JSON manifest** with distinct inventory provenance and runtime verification records. Unknown JSON fields are rejected. An example is in `crates/tihulu-ai-market/tests/fixtures/installed-chat.json`; all fixtures are synthetic and do **not** identify actual model installations.
2. Deterministic checking of app task and input/output modality, runtime + file format + accelerator target, RAM/GPU VRAM/Hailo NPU capacity, requested EN/TR/DE languages, **directional** translation, quantization, license, offline constraints, paired-host authorization and app-adapter readiness.
3. Separate `status` (installed, external, downloadable, candidate, incompatible, unknown, unverified) and `verdict` (recommended, candidate, incompatible, unknown). A model's file presence or description never establishes task readiness.
4. Stable, side-effect-free ranking that **reuses verified installed models first, then verified externally managed models**; candidates/downloads are not recommended as already installed.
5. An opt-in AI advisor **proposal validator**: citation fields, confidence (0–100), request task, model ID and Internet-consent checks. The same deterministic Rust gate always runs; an LLM's confidence does not make an incompatible model usable.
6. A **mock preview lease/cleanup planner** for L-01, L-04, L-11, L-16. It emits cleanup obligations for app-owned contexts/buffers and owned pending download jobs, but never for borrowed external services.

## Non-goals and trust boundaries

- **No** real model download, install, copy, move, delete, file scan, background helper, benchmark, OS process kill, inference, GPU/Hailo initialization, Jan/Ollama data access, paired remote connection or HTTP call is implemented.
- `ModelManifest` is **untrusted catalog data**. Its `verified_runtimes` claims cannot make a model recommended without an independent `TrustedRuntimeProbe` supplied by a **trusted** runtime/registry adapter. The runtime probe itself is not fabricated by this crate. No API accepts "the model said it works" as execution authority.
- Manifest `id` must be bound by the registry to source provenance and artifact digest or versioned endpoint identity. Source references/paths are managed by Agent 02, never invented or relocated here. Future proof records must bind exact artifact content/version and hardware/runtime builds, with freshness/revocation semantics.
- A `Recommended` matching result is a **preflight metadata decision**, not permission to write another application's config or launch a model. Successful native inference, integration adapter authorization and explicit approval remain external gates.
- The compatibility matrix is deliberately conservative. Unknown format support (for example generic safetensors) stays `Unknown`; HEF + HailoRT is distinct from GGUF + llama.cpp / NVIDIA CUDA. A Hailo device is not a GPU, and an ARM64 CI runner does not demonstrate Hailo readiness.
- A paired GPU host is a distinct execution target requiring explicit authorization and served endpoint metadata. No public server endpoint, pairing or network discovery is included.
- License allowlisting is policy, **not legal advice**. Unknown licenses do not qualify as known unrestricted licenses.

## Availability vs. recommendation

| Inventory availability | Verification / compatibility | Result |
| --- | --- | --- |
| `installed` | trusted runtime probe + integrity + app adapter + full checks | `recommended`, status `installed` |
| `external` (e.g., managed by Jan) | same verified readiness; never move/kill external service | `recommended`, status `external` |
| `downloadable` | static match; not installed locally | `candidate`, status `downloadable` (or `unverified` when proof absent) |
| `candidate` | static match only | `candidate` |
| any | explicit impossible format/target, missing task, budget overflow, license disallowed | `incompatible` |
| any | missing runtime/memory/hardware/language/license evidence or unverified app adapter | `unknown` |
| installed or external | metadata claims compatible but no trustworthy tested runtime proof | `candidate`, status `unverified` |

Never label a model compatible across all TIHULU apps from its filename. An ASR model isn't a translation model, a chat LLM isn't automatically an embedding model, and tool calling requires separate capability declarations and permission controls.

## Hardware targets

| Requested target | Minimum deterministic constraints | Physical verification still required |
| --- | --- | --- |
| CPU (Linux/Windows) | known supported file format + runtime, RAM budget and adapter | actual CPU/runtime performance |
| CUDA | CUDA-capable declared backend, target-specific probe, distinct VRAM budget | compatible NVIDIA driver/CUDA/backend build |
| Apple Silicon | MLX or declared supported backend; **unified** memory budget rather than invented separate VRAM | native Metal/MLX runtime and usable memory |
| Raspberry Pi CPU | declared ARM64-compatible runtime, memory and app adapter | real Pi 5 CPU/thermals/speed |
| Hailo AI HAT+ 2 | Hailo HEF, HailoRT, Hailo NPU budget and explicit probe | Pi 5 Hailo-10H assets, HailoRT/driver and PCIe |
| Paired GPU host | versioned authorized endpoint, actual host budget, independent trusted probe | encrypted pairing, endpoint claims, lease/cancel/unload support |

EN is the default **UI language**, with TR and DE targets. This crate emits stable enums and English diagnostics only; localization belongs to Agent 06/UI. Model language support is separate from UI localization.

## Ownership / stop and test limitations

The mandatory policy is **On-Demand / background OFF / no keep-warm**. `MarketPreviewSupervisor` is a synthetic state tracker: it plans cancel-download, free owned RAM/VRAM/NPU buffers, and unload owned model contexts only after the last session. It refuses new sessions on close and does not touch `BorrowedExternal`. Real cleanup requires executor-held handles, bounded cancellation/join, a last-lease check across *all* consumers, and native resource measurements. See [Lifecycle test plan](LIFECYCLE_TEST_PLAN.md).

**Acceptance evidence:** the tests are source fixtures only; they neither download model weights nor operate GPU/NPU/SDR. Actual runtime/hardware verification and cross-OS standalone crate CI are outstanding until executed and reported. See [advisor and manifest contract](CONTRACTS.md).
