# Model Registry and Discovery — Workstream 02

**Status:** Experimental Rust implementation in the owned
crates/tihulu-ai-registry path. No integration with other apps, no live API
calls and no hardware acceptance. The coordinator owns the root workspace.

## Consent and adapters

| Adapter | Input supplied after approval | Reads | Never does |
| --- | --- | --- | --- |
| Jan Desktop | Authorized **current data root** | Only llamacpp/models and mlx/models trees | Guess a path; inspect chat history, secrets, config or launch Jan |
| Hugging Face | Explicit cache root | Model filenames, sizes, in-scope snapshot symlink metadata | Download, parse weights, traverse external symlinks |
| Custom folder | Chosen directory + owner label | Bounded model candidate metadata | Copy, move, delete or rewrite files |
| Ollama local API | Authorized loopback URL + already-fetched model list | In-memory sanitized IDs | Connect to Ollama or scan its private blob store |
| Jan/local OpenAI-compatible API | Authorized loopback URL + already-fetched model list | In-memory sanitized IDs | Network probe, configuration write, host/process kill |

No implicit scan of ~/ or other OS default locations is performed. Selecting
an app name must be paired with its **user-provided** authorized directory or
local endpoint. The caller is responsible for user permission, endpoint
authentication and correct evidence that a model list came from a real
authorized request. The crate neither uses credentials nor connects to hosts.

A caller may construct the default shared library path by calling
default_model_library(home), returning ~/TIHULU AI Tools/Models (Windows:
the user's chosen home equivalent). No directory is created; any other
approved directory can be used.

## Status and identity contract

The registry keeps the original source path, canonical target at scan time,
source adapter and *external file owner*. A scanned GGUF, ONNX, Safetensors,
TFLite, HEF or whisper GGML filename is **Discovered file** only. An artifact
would need a separately implemented trusted parser to become **Validated
model**; no such upgrade is performed here. A **Declared endpoint** is not
reachable by assumption. Only a trusted API client's successful listing can
supply **Available endpoint** status. **Verified runtime compatibility** is a
separate evidence field, always unchecked in this prototype.

Reference IDs are stable for identical source provenance and path, not
SHA-256 content digests. Only exact provenance matches are deduplicated;
same filenames in different places and different snapshot aliases survive.
No payload hash is computed (and no fake SHA-256 is claimed). Re-scanning
a missing model may mark the old reference missing without removing it.
A future explicit file-hash permission and trusted verifier can provide
cross-path content dedup and format validation.

A model from Jan may have an external on-disk owner while being served by an
independently created **AI Tools-owned** inference context. These owner
identities must never be collapsed.

## Phase 2 ResourceLedger handoff

The coordinator-owned core **ResourceLedger in Draft PR #12** is the ONE
authority for loaded model/runtime reference counts, owner phases,
last-lease unload requests and cleanup acknowledgements.

The previous Phase 1 production **LeaseRegistry has been removed**:
if kept alongside core, its independent counters could create duplicate
unload decisions. The Registry now stores private typed ModelId provenance
and a non-authoritative TaskLeaseReceipt only. The ResourceLedgerPort
interface forwards registration, attach, individual task release, app
stop and completion acknowledgement to the authoritative core ledger.
An opt-in production delegation implementation now exists behind the
core-ledger feature, available only on an integration branch containing the
coordinator PR #12 core API. The foundation base lacks those symbols.

- ModelId::Artifact / ModelId::Endpoint: typed, complete, stable private
  source reference, NOT a runtime ID or content hash.
- ModelOwner: Jan/Ollama/HF/TIHULU on-disk provenance, separate from the
  authority over a loaded runtime.
- RuntimeInstanceId: the backend runtime context label.
- LedgerOwnerRef / LedgerResourceRef: exact proposed core OwnerId /
  ResourceId numeric bridge types; only core may allocate/reuse policies.
- ContextAuthority: one-to-one core Exclusive, SharedManaged or
  BorrowedExternal provenance.
- ConsumerTask: app and task references for attribution; all actual
  lease reference counts remain in core.
- LedgerCleanupRequest: forwarded only from core, never invented in
  this registry. Owned backend must execute actual cleanup and acknowledge.

**Exact phase-2 mapping, conflict explanation, tests and remaining blockers:
see [LEDGER_HANDOFF.md](LEDGER_HANDOFF.md).**

Synthetic model-task tests cover L-03 (two apps share), L-04 (last
owned lease), L-07 (borrowed Jan/Ollama), L-15 (separate runtime
instances) and duplicate app Stop. These do not prove GPU/NPU/SDR
cleanup, core PR #12 cross-branch integration or real provider behavior.

## Security limitations / integration gates

- Traversal is capped by directory depth, total entries and model candidates.
  Jan restricts traversal to its two model directories. Root approval must
  canonicalize to precisely the selected source root.
- The scanner never opens model payloads. It skips all ordinary symlinks and
  allows a Hugging Face *file* symlink only if its canonical target remains
  inside the approved cache root. Dangling/external links are ignored.
- On a concurrently maliciously modified filesystem, ordinary canonicalize
  and metadata have a time-of-check/time-of-use race; production hardening
  requires platform-specific handle-relative/openat techniques and mount
  boundary validation. Never claim this prototype is race-proof.
- Network-mounted directories are **not auto-detected**; the UI must disclose
  and explicitly authorize such roots. Production code must apply mount and
  file-system controls before trusting untrusted directories.
- Loopback API observation permits only literal http://localhost,
  http://127.0.0.1 or http://[::1] with an explicit port; it performs
  **zero** I/O. Remote endpoints belong to the separate authenticated
  transport workstream.
- API model IDs and source metadata are untrusted; never pass them as commands,
  executable paths, secrets or user permissions.
- Keep private absolute paths in the user's own registry, never in repository
  commits or telemetry. No local persistence is implemented yet.
- Rust 1.82+ standalone tests should be run on Linux x86/ARM, macOS ARM64,
  Windows x86/ARM; synthetic CI cannot prove Pi 5, Hailo-10H, CUDA or Metal
  runtime compatibility or actual GPU memory release.
- EN default; future UI labels and consent language must also support TR/DE.
- For integration, coordinator must remove the crate-local workspace stanza
  and add the crate to the root workspace, then agree shared core ownership
  and verification contracts. Do not modify the shared root Cargo.toml here.

No download, move, deletion, runtime load or foreign application mutation is
in scope. Draft PR remains review-only.
