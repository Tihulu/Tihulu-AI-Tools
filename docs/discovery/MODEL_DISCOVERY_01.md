# MODEL-DISCOVERY-01 — consent-scoped, read-only local inventory

**Status:** experimental native Rust implementation in an isolated Draft PR stacked on coordinator PR #19. No scans of the user's machines, installations, downloads, model execution, or app configuration changes were performed.

References: APP-PATH-01 Issue #18, Phase 2 coordinator Issue #15, sandbox PR #17, native bridge PR #19, and the ResourceLedger contract in PR #12.

## Consent and bounded scope

A trusted native consent UI must first show the **selected model root, canonical resolved root, source owner, and separate hidden-directory choice**. It then supplies affirmative permission to ApprovedModelScope::approve. This constructor is a scoped Rust policy gate, **not** an OS permission dialogue, authorization token, or proof that a UI asked the user.

No automatic home, disk, mount, application, file, network or runtime scan. Entire filesystem/drive roots and lexical parent traversal are rejected. A user-selected external SSD **subfolder** is supported. Each new, legacy, or externally configured path requires an independent grant; suggestions from app_paths.rs are never grants.

ApprovedModelScope::revoke invalidates all clones and prevents new metadata reads and successful final result publication after observed revocation. Partial results are discarded on revocation. The host UI must expire scopes on close and on app restart.

Default depth 6 / 4,000 entries / 250 models; hard caps depth 8 / 10,000 entries / 500 models. Each JSON/header read is limited to 64 KiB; each directory enumerates at most remaining budget + 1 entries. Partial reports explicitly set truncated. Hidden children are skipped unless separately approved. On Windows the hidden/system file attributes are also inspected.

## Provider model

| Provider | Explicitly approved path | Inspected only | Ownership |
| --- | --- | --- | --- |
| Jan Desktop | Current Jan data root | llamacpp/models and mlx/models only | Borrowed JanDesktop |
| Ollama | Models root | Bounded manifests JSON + explicitly referenced sha256 model blobs | Borrowed Ollama |
| Hugging Face | HF cache root | models--*/snapshots/* and in-scope snapshot-file aliases | Borrowed HuggingFace |
| TIHULU app | One app's active/legacy model folder | That app's selected tree only | TihuluApp(stable app ID) |
| Custom/SSD | One selected user directory | That directory only | UserNamed(owner) |

An Ollama model is reported only when a bounded manifest contains a model-layer descriptor with a literal validated SHA-256 digest and the corresponding blob exists under the approved root. The digest is **declared**, not verified cryptographically; other Ollama blobs are not searched as models. Malformed metadata or missing referenced files produce warnings and never become verified models.

Jan chat/history/config paths are never entered. HF datasets, spaces, and lock trees are not traversed. Directory symlinks are skipped. In-scope HF snapshot-file aliases are allowed only if canonical targets remain inside the approval. Model-file content is not parsed except bounded GGUF/SafeTensors container headers; whitelisted sibling config.json fields may be read up to 64 KiB.

## Typed UI-facing read-only contract

- ModelSource: Jan, Ollama, HF, independently identified TIHULU app, or selected custom directory.
- ConsentDecision and ApprovedModelScope: user-granted read scope with revocation, selected and canonical roots.
- DiscoveryBounds, DiscoveryReadOnlyPort::inventory, DiscoveryInventory, and warning types.
- DiscoveredModel: typed ModelId, name, size, format hint, canonical location, provenance path, source, owner, evidence and bounded whitelisted untrusted metadata.
- FileEvidence::UnverifiedCandidate versus FileEvidence::FormatHeaderVerified. The latter proves only bounded GGUF magic/version or SafeTensors container header checks. It does NOT prove full weight integrity, license, task support, inference readiness or GPU/NPU compatibility.

The stable ModelId is a private source reference, never a ResourceLedger ResourceId, content digest or public-safe identifier. Private paths must not be sent to other apps, browser pages, telemetry or remote services without separate consent. No Tauri IPC command is implemented in this PR; PR #19's existing native module status remains status-only.

Runtime leases remain entirely under coordinator ResourceLedger. Discovery cannot load, unload, move or delete files, cannot kill Jan/Ollama, and cannot change independently installed TIHULU app roots or configs.

## Synthetic tests and limitations

Tests use disposable temporary directories only: permission denial and revocation, scope traversal, bounded enumeration, Jan-only roots, Ollama manifest/digest and malformed JSON, HF in-scope and out-of-scope symlinks, Jan symlink engine escape, hidden folder consent, independent TIHULU app roots, external SSD subfolder, distinct models with same filename, unreadable files, malformed config, verified GGUF/SafeTensors headers, and preservation of original files. No real user model folders or hardware are accessed.

Known production hardening remains: cross-platform std::fs canonicalization and metadata have time-of-check/time-of-use races under adversarial concurrent directory replacement. Production requires native handle-relative no-follow opening, mount and Windows reparse-point checks, proper native grants and independent cross-OS acceptance. Windows symlink creation may require privileges; any skipped test must be reported.

## Safe integration order

1. Review coordinator resource ownership and stacked sandbox PR #17.
2. Review native read-only Registry/Core bridge PR #19.
3. Review this NEW discovery Draft PR targeting PR #19's head branch, keeping PR #19 unchanged.
4. Run actual cargo fmt/test/clippy on this exact child SHA across available CI OS runners; correct failures only on this child branch.
5. After review, separately authorize native Tauri consent UI + OS-aware path grants. Reuse read-only contracts, but do not present simulated listings as real scans.
6. Later, authorize controlled real-OS scan tests and optional per-app integration; keep model moves, Jan config changes, downloads, runtime inference and GPU/NPU/SDR acceptance separately gated.

No merger, release, installation, actual model scan, external file changes, or other agent work is included.
