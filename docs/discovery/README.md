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

## Runtime lifecycle handoff

The pure LeaseRegistry records:
- RuntimeOwner::AiTools: an existing tracked TIHULU-owned loaded runtime.
- RuntimeOwner::BorrowedExternal: a pre-existing Jan/Ollama/other app runtime.
- Per-app and per-task leases, with separate owners across shared models.
- ReleaseAction::KeepLoaded while another client holds a lease.
- ReleaseAction::UnloadOwned after the last AI Tools-owned lease; the
  backend must cancel requests, unload the model, free RAM/VRAM and then
  acknowledge the unload. No keep-warm by default.
- ReleaseAction::DisconnectBorrowedClient on last borrowed-client release:
  disconnect the client only. Do not unload or kill the external provider.

**Important:** The registry generates expected lifecycle actions, not process
control. It does not allocate models, prove successful unload, reclaim VRAM
or supervise dead clients. Runtime/host workers must implement bounded
shutdown, owner-handle identity checks, cancellation and stale lease expiry.
All external Jan/Ollama services and model files remain untouched.

Mock acceptance coverage: L-02 (advisor/agent), L-03 (two apps), L-04 (last
host lease), L-07 (borrowed Jan), L-11 (relaunch) and L-15 (separate
models). No L-01/L-08/L-09/L-10/L-14 hardware or process behavior is claimed.

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
