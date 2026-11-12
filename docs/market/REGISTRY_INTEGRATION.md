# Phase 3: typed Market ↔ Registry matching and stabilization

**Status:** Implemented as a read-only, deterministic integration contract in `crates/tihulu-ai-market/src/registry_contract.rs`; no production Registry adapter or real hardware probe exists. Phase 2 coordination: [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15). Agent-owned Draft PR #11 remains unmerged.

## Authoritative identity and data flow

```text
Authorized Registry/Core adapter
    ├─ ModelRecord.id: ModelId::{Artifact, Endpoint}
    │                   → private adapter map → RegistryModelId(opaque local handle)
    │                      [never a hash, path serialization, or runtime ID]
    ├─ ModelRecord.owner → RegistryOwner
    ├─ ModelRecord.reference → RegistryPresence + format hint
    ├─ ModelRecord.artifact_validation → ArtifactEvidence
    ├─ ModelRecord.runtime_verification → RegistryRuntimeEvidence
    └─ separately verified catalog ↔ Registry provenance → CatalogIdentityBinding
                  │
Market versioned catalog ── ModelManifest(id, format, task/language/license/memory)
                  │
Approved application adapter ── ApplicationTask(ApplicationId, TaskRequest,
                                                adapter_verified)
                  │
Authorized hardware/runtime observer ── TrustedHardwareProbe(
       RuntimeTarget(HostId, Runtime, HardwareTarget), budgets, readiness, host authorization)
                  │
                  ▼
             match_registry_model  [PURE / READ-ONLY]
             recommend_registered_models [pure task-level ranking]
                  │
                  └─ RegistryMatchDecision {
                       inventory_status, observed_presence, compatibility verdict + reasons,
                       registry_model_id, catalog_model_id, application_id,
                       target, can_propose_assignment, read_only: true
                     }
```

The coordinator/Agent 02 **must establish** a catalog-to-Registry provenance link before calling the match interface. `CatalogIdentityBinding` requires a non-empty evidence identifier and the same catalog ID. Agent 02's **current** `ModelId` is a typed `Artifact` or `Endpoint` provenance enum that may include sensitive private paths or endpoints. A local trusted adapter must map that exact enum value to an opaque `RegistryModelId` handle and retain the reverse mapping privately. Market's string wrapper **is not** the Registry enum, a cryptographic digest, an ownership capability, or a runtime identity. Do not stringify/log/transport `ModelId` paths. Matching filenames, unverified hashes or model names is insufficient evidence of equivalence.

## Exact mapping proposed for Agent 02's current Registry snapshot

| Agent 02 type / state | Agent 03 type / state | Required caveat |
| --- | --- | --- |
| `ModelRecord.id: ModelId::{Artifact,Endpoint}` | `RegistryModelId::new(opaque_local_handle)` | Private adapter maintains one-to-one typed `ModelId`↔handle mapping; **never** serialize source/canonical paths, endpoints, or credentials into the handle |
| `ModelOwner::JanDesktop/Ollama/HuggingFace/UserNamed/AiTools` | `RegistryOwner::JanDesktop/Ollama/HuggingFace/UserNamed/AiTools` | Preserve foreign owner; never change owner just to match catalog |
| `ModelReference::File` with `FileFormat` | `RegistryPresence::FileObserved { format_hint }` | File extension is **only a hint**; if missing, `Missing` |
| `ModelReference::Endpoint` + `EndpointState::Declared` | `RegistryPresence::EndpointDeclared` | Not proof endpoint is reachable or runnable |
| `EndpointState::ListedByAuthorizedClient` | `RegistryPresence::EndpointListed` | Model listed ≠ successful inference |
| `ArtifactValidation::Unchecked` | `ArtifactEvidence::Unchecked` | No trusted integrity claim |
| `ArtifactValidation::Validated {validator, evidence}` | `ArtifactEvidence::Validated {evidence_id}` | Only after an independently authorized validator; source itself cannot self-certify |
| `RuntimeVerification::Unchecked` | `RegistryRuntimeEvidence::Unchecked` | Must not upgrade catalog `verified_runtimes` claims |
| `RuntimeVerification::Incompatible {reason}` | `RegistryRuntimeEvidence::Incompatible {reason}` | Hard failure for selected target |
| `RuntimeVerification::Verified {runtime, host, evidence}` | `RegistryRuntimeEvidence::Verified {runtime, hardware, host_id, evidence_id}` | **Requires separate typed runtime and hardware mapping and authenticated host identity**. Arbitrary strings are not sufficient to issue a trusted probe |

**No direct dependency on Agent 02's crate is introduced in this Draft PR.** Agent 02 currently uses open-ended strings for `runtime` and `host`, whereas Market's `Runtime`, `HardwareTarget`, and `HostId` are typed. A coordinator-reviewed adapter must explicitly map versions and unsupported values to `Unchecked`, not guess from OS/filename. The current Registry does **not** persist a `CatalogIdentityBinding` or a trusted hardware probe; this is an integration blocker, not an implementation claim.

## Status / verification invariants

- `inventory_status = installed` or `external` means **the catalog declares a local/reference-in-place artifact**. It does not mean that the file was parsed, its license was approved, a runtime was successfully started, or a task adapter exists.
- Catalog `verified_runtimes` and `integrity_verified` are **untrusted claims** without separate Registry evidence. The matching function disregards manifest-only runtime test claims; absent Registry evidence yields a Candidate/Unknown verdict, never Recommended.
- Unknown license, missing artifact, unlisted endpoint, mismatched host/runtime, missing adapter, or missing provenance cannot yield a Recommended result. Contradictory format, owner and impossible hardware targets are Incompatible.
- Hailo HEF/HailoRT and NVIDIA CUDA/GGUF are different format/runtime/accelerator triples. Pi CPU does not imply Hailo capability. A paired GPU endpoint is a distinct, authenticated host and runtime target; a local CUDA probe cannot validate it.
- A synthetic `TrustedHardwareProbe` is a plain value for deterministic tests and **not an authentication mechanism**. In production only a trusted, separately authorized Core/host adapter can issue or attest it, with versioned source evidence and freshness. A caller cannot treat arbitrary JSON/LLM output as trusted.
- A Recommended static preflight can set `can_propose_assignment` but still returns `read_only = true`. It is **never** permission to download, modify `aipath.txt`, bind a foreign runtime, open sockets or launch inference. Assignment requires independent approval and runtime-specific checks.

## Lifetime and single ownership authority

Market creates **no leases** and never unloads models. Model identities here describe catalog and Registry *references*, not runtime sessions. Coordinator-owned core must provide one authoritative resource ledger for app/session, runtime instance, model context, and host-client leases. Agent 02 has removed its duplicate runtime `LeaseRegistry` in Phase 2 and proposes a `ResourceLedgerPort` to coordinator core. Agent 04's remote host must likewise forward active-owner release decisions to the one authoritative ledger; Market must not become another decision-maker.

The separate `MarketPreviewSupervisor` in Phase 1 is a **synthetic cleanup-plan fixture**, not a production lifetime/lease authority. In integration, its mock `UnloadOwnedModel` action must never run in parallel with the authoritative ledger or initiate process-name kills. On-Demand/Background OFF/no keep-warm remain the default. Borrowed Jan/Ollama and externally managed SDR services are preserved. Physical RAM/VRAM/NPU/SDR release requires future real adapters, bounded shutdown and measurements.

## Synthetic Phase 2 fixtures

Fixture JSON under `tests/fixtures` is **fabricated test metadata, not real downloaded weights or verified hardware evidence**. The new `tests/registry_contract.rs` exercises:
- Jan Desktop-owned GGUF, unchecked/checked artifact and runtime evidence, provenance and owner mismatches
- Media/Text ASR and directional TR→DE translation, including wrong task/direction
- CUDA GGUF VRAM budget, Raspberry Pi 5 CPU only, Hailo HEF/HailoRT/NPU budget and cross-target contradiction
- Paired GPU endpoint authorization, presence, host identity and distinct remote budget
- Wrong format, missing reference, unknown license, and non-executable cited online suggestions
- Unverified app adapter, host runtime readiness, blank IDs

Run only against the owned crate:
```sh
cargo fmt --manifest-path crates/tihulu-ai-market/Cargo.toml --check
cargo test --manifest-path crates/tihulu-ai-market/Cargo.toml --all-targets
cargo clippy --manifest-path crates/tihulu-ai-market/Cargo.toml --all-targets -- -D warnings
```

The integration **sandbox** in Draft PR #17 includes a frozen Market source snapshot and temporarily removes the nested `[workspace]` marker only in that sandbox. Agent 03 retains standalone compatibility on its branch. Coordinator must resnapshot the reviewed Agent 03 changes to test them in root-workspace CI; old sandbox results do not validate the new SHA. A pass on root CI or a check of JSON syntax is not proof these tests compiled. Real Jan API, Pi CPU, Hailo, NVIDIA, Windows/macOS and remote host acceptance remain pending.

## Phase 3 mock app-task recommendation flow

`recommend_registered_models(application_task, candidates)` accepts an authorized `ApplicationTask` plus a bounded caller-provided list of `RegistryCandidate { manifest, registry, target, probe }` snapshots, delegates **each** decision to `match_registry_model`, and returns a deterministic sorted list. Verified installed models precede verified externally owned references; unverified, unknown-license, impossible-format/hardware and market-only entries never become executable recommendations. It never downloads, runs tests, creates leases, writes `aipath.txt` or touches Jan.

The matching API returns `read_only = true` for every decision. Even `can_propose_assignment = true` is **only** a preflight suggestion. The user must separately approve any model binding, and a trusted Registry/Core adapter must recheck artifact, runtime, license and host evidence before side effects.

See `crates/tihulu-ai-market/tests/recommendation.rs` for a synthetic Survival Kit Chat → Registry-owned local GGUF / Jan-owned GGUF / insufficient-CUDA VRAM / Pi Hailo wrong modality / unknown-license / untrusted-market comparison, including input-order-independent ranking. No physical machines or inference engines participated.

### Safety hardening

Versioned manifest JSON now has a conservative 64 KiB input limit and bounded identifier/capability arrays. Unknown/NOASSERTION/NONE license placeholders must not be disguised as a known SPDX license; task-specific allowlists reject conflicting known licenses. **A declared SPDX string is still untrusted catalog metadata, not independent legal or license verification.** The real authorized adapter must validate provenance and license approval before actual assignment.

The Phase 3 contract intentionally has **no direct dependency** on sibling Agent 02's crate. Only coordinator-owned integration can implement private `ModelId` handle issuance and trusted runtime/hardware evidence. No manufactured catalog-only test record may become a loaded runtime lease.
