# Phase 5 Market inventory and compatibility bridge

**Status:** Implemented as a read-only Rust contract and deterministic synthetic tests on Agent 03's Phase 5 branch. Not a production inventory scanner, model validator, downloader, GPU benchmark, hardware certification, or deployment.

**Stacked base:** `coord/phase4-native-inventory` (Draft PR #19). Source ownership is limited to `crates/tihulu-ai-market/**`. Agent 02's Registry, Tauri UI, coordinator Core/ledger and shared workflows are **not modified**.

## Exact typed contract

The new `inventory_bridge` module depends on the existing Registry crate via the **Market crate's own Cargo manifest**, not through root-workspace edits. `InventoryInspection` receives an authorized `tihulu_ai_registry::registry::ModelRecord` and its selected, full typed `ModelId`. It checks that the selected ID matches the record *and* the reference's actual provenance (owner/source/canonical path or endpoint). The raw `ModelId`, which may embed private file paths and server URLs, is not serialized or returned to UI.

The caller separately supplies an adapter-issued opaque `RegistryModelId` handle and a validated catalog provenance binding, an `ApplicationTask`, a typed target `RuntimeTarget`, and a `TrustedHardwareProbe`. **An ordinary Rust struct does not authenticate its caller**: only a trusted coordinator-approved adapter may issue those fields or allow a later assignment. Unknown or invented host/runtime strings are not silently normalized.

`inspect_registered_model` produces:

- `InventoryReadiness.catalog_status`: availability from the versioned catalog (`installed`, `external`, `downloadable`, etc.), **never** a claim that a model process is running.
- `discovered`: a Registry file observation or an endpoint listed by an authorized client. A missing path or merely declared endpoint remains undiscovered.
- `artifact_verified`: Registry artifact-validation evidence with non-empty validator and evidence, not a filename guess. It is not necessarily neural architecture verification.
- `target_runtime_verified`: an exact Registry runtime token/host and separate trusted target probe with matching runtime and accelerator. No alternative aliases, guessed host IDs, or cross-GPU evidence.
- `runnable_preflight`: true only if the full deterministic matching and provenance/format/task/license/budget/runtime/adapter checks pass. This is **not a model process, execution permission, physically tested hardware, or performance guarantee**.
- `observed_format`: typed Registry `FileFormat` hint (GGUF, Whisper-GGML, ONNX, safetensors, TFLite, HEF) or listed endpoint. Extension-based hints **cannot prove valid model contents**.
- `declared_architecture`: optional catalog-provided `ModelArchitecture` classification. Existing v1 catalogs default to `unknown`; unknown non-endpoint architecture cannot produce a runnable preflight. Classification is **not inferred from filename**. Hailo compiled and served endpoint types cannot be mislabeled GGUF. Explicitly impossible architecture/task contracts are rejected.
- `declared_tasks`: catalog task claims, never automatically inferred from model format.
- `decision`: existing pure `RegistryMatchDecision` with `read_only = true`, no mutating actions, model download, inference launch, runtime lease or installer.

## Target decisions — synthetic examples

| Case | Deterministic result | What it does *not* prove |
| --- | --- | --- |
| Owned GGUF + CPU, trusted artifact/runtime/host probes, declared decoder | Runnable *preflight* | CPU performance or loaded weights |
| Jan-owned GGUF + CPU, consistent provenance | External source, reusable as reference; borrowed Jan remains untouched | Jan process ownership, server availability |
| GGUF + NVIDIA CUDA target | Eligible only for matching exact runtime/host evidence, compatible VRAM budget and adapter | NVIDIA driver/CUDA/backend actually loaded |
| ONNX + Apple Silicon | Eligible with unified RAM and separately trusted target probe | Actual Metal/MLX performance |
| GGUF + Raspberry Pi CPU | Pi CPU-specific target; not a Hailo permission | Raspberry Pi speed or thermals |
| HEF + HailoRT + Hailo target | Eligible with Hailo-specific NPU budget and exact probe; **not** CUDA/GGUF | AI HAT+ 2 installation or physical NPU inference |
| Listed, user-authorized paired GPU-host endpoint | Eligible only with host authorization, matching runtime and VRAM budget | Secure pairing, live host connection or actual remote inference |
| TFLite format hint and no declared supported backend | `unknown` (never silently executed through ONNX Runtime) | TFLite inference support |
| Incorrect owner/ID, unknown/corrupt JSON, mismatched format/host, unknown/restricted license | `unknown` or `incompatible`, no runnable preflight | Any physical action |
| Downloadable-only online recommendation | Catalog candidate only; no automatic download | File presence or runtime safety |

**Known gap:** Agent 02 Registry's `FileFormat` does not currently recognize MLX. Market does **not** silently classify a `.mlx` file as ONNX or GGUF; MLX remains a future separately authorized Registry format/validator integration task. For Apple Silicon, the synthetic bridge checks the declared ONNX+Apple target instead.

## Ownership and cleanup

Market has zero filesystem/network I/O and no model/GPU/NPU/SDR workers. It does not acquire or release resource leases. Only the coordinator-owned authoritative resource ledger and authorized backend may start/stop owned contexts; Jan, Ollama and other independent providers remain external and unaffected. **No keep-warm by default**. The earlier Market lifecycle supervisor is a mock cleanup-plan fixture and cannot be used as a production ledger.

## Offline fixtures and commands

The fixture `tests/fixtures/phase5-offline-decoder.json` explicitly declares an architecture. `tests/inventory_bridge.rs` also uses previous versioned offline fixtures with deterministic synthetic observations of real Registry **types**, without reading model files or touching external applications.

Execute in an integration checkout containing PR #19's Rust workspace:

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

For just Market:

```sh
cargo test -p tihulu-ai-market --all-targets
cargo clippy -p tihulu-ai-market --all-targets -- -D warnings
```

Physical GPU/Apple/Pi/Hailo/SDR behavior, Jan configuration, model downloads, cross-process lease cleanup and actual host authentication are **not** tested. Green synthetic CI is not hardware acceptance. Coordinator must separately approve any UI integration, persistent paths, model-validation adapter, native packaging or production release.

## QA-CROSS-01 — Registry #20 independent app ownership (integration patch)

**Pinned inputs:** Registry #20 `7c3030d09c30a841ed42813b6427d7e7d7f7b589`; Market #22 `d015a806966d97e340f807b6b5ef418b17d0a492`; independent QA #21 `93b9272fef87e1d70d6a7ae141a719bd5caa4978`. This isolated branch starts from Registry #20 and overlays **only** the eight Market #22 paths before applying the Market-owned cross-contract fix.

- Map `ModelOwner::TihuluApp(app_id)` to **`RegistryOwner::TihuluApp(app_id)`**, not `AiTools`, `UserNamed` or a generic `ExternalService`. The app ID is preserved in the typed read-only `InventoryCompatibilityReport.source_owner`.
- Catalog source uses explicit `ModelSource::TihuluApp { app_id }` with `availability: external`. Independent app models **cannot** claim `installed` (AI Tools owned). Owner IDs are validated as Registry-compatible lowercase slug identities and compared **exactly**, not case-folded or suffix-matched.
- Accept `TihuluApp` file provenance only from `SourceAdapter::TihuluAppDirectory` with a self-consistent typed Registry `ModelId`, owner, path and reference. A generic custom folder or endpoint must not impersonate the app directory adapter.
- A different consumer application cannot propose a binding to another independently owned application's files, **even when synthetic artifact and runtime evidence are marked verified**. The current typed matching contract carries **no cryptographically authorized cross-app borrowing grant**. Cross-app use therefore fails closed until coordinator defines a separately approved owner grant; a catalog-binding string, runtime probe or user-supplied ID alone is insufficient.
- Same-app (exact `application_id == app_id`) evaluation may reach `runnable_preflight` **only after** identity, source, format, task, license, artifact and target-runtime checks. This still does not execute/load models or grant filesystem access. `discovered` alone is never verified or runnable.

Deterministic regression tests in `tests/cross_market_ownership.rs` cover discovered-only external app model, synthetic verified same-app preflight, mismatched owner IDs, `UserNamed`/generic-external alias rejection, unauthorized cross-app assignment, wrong adapter, malformed owner ID, and forbidden `installed` ownership claim. These tests use no real model files, hardware or provider connections.
