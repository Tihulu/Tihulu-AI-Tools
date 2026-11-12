# Model Market resource lifecycle — L-01, L-04, L-11, L-16

**Status:** Synthetic in-memory cancellation and reference-counting tests implemented; real teardown executor and physical hardware verification **not** implemented.

## Mandatory behavior for future workers

1. Default is **On-Demand / Background OFF / no keep-warm**. Opening catalog or evaluating compatibility must not start inference, reserve GPU/NPU memory or spin a background worker.
2. Preview/test/import/download tasks are tied to an owned, cancellable session. Acquire an explicit ownership handle before allocating model context, CPU/GPU/NPU buffers, child process, downloaded bytes or network worker.
3. On task finish, Cancel, Stop AI, Hard Stop or true Quit: reject new/late work; signal cancellation; await each **owned** worker with a bounded timeout; release task buffers and model contexts. The last authorized model lease triggers unloading. Never delete on-disk weights merely to release RAM.
4. Preserve independent/borrowed Jan, Ollama, readsb, tar1090 and unrelated shared GPU-host consumers. **Never** locate processes by name and kill them; an external PID is not an ownership token.
5. If a download is interrupted, explicitly keep it in an incomplete/queued state under a documented resume policy and release the network/task worker. A partial artifact is never registered as `installed`. No downloader exists in this PR.
6. On reload/relaunch, allocate a fresh session. No late callbacks can reacquire a lease after the previous controller closes. Keep a clear distinction between a host's independent service lifetime and the lifetime of this app's model lease.

## Implemented mock test matrix

| Scenario | Unit test | Verified synthetic result | Not yet proven |
| --- | --- | --- | --- |
| L-01 Exit with AI Tools-owned preview | `l01_quit_releases_owned_buffers_and_context_without_external_kills` | Cleanup plan includes owned model unload + RAM/VRAM buffer release, no borrowed Jan action; second Quit idempotent | Actual OS worker stop and freed VRAM |
| L-04 Last shared host-client model lease | `l04_model_unloads_only_after_last_owning_client` | Shared owned model not unloaded on first client release, unloaded on last | Real host reference accounting, host process preservation |
| L-11 Relaunch after Quit | `l11_relaunch_uses_fresh_sessions_without_late_restart` | Closed controller rejects new leases and a new controller starts with empty state | No stray OS workers or queued timers |
| L-16 Quit during model download | `l16_download_worker_must_cancel_and_never_turn_into_installed` plus `no_download_is_ever_presented_as_an_installed_model` | Cancel action for owned mock job; no state promotion to Installed | HTTP cancellation, resumable partial files, integrity checks |

Additional tests cover duplicate Quit, borrowed service safety, CPU vs CUDA vs Apple unified memory vs Pi CPU vs Hailo NPU, paired endpoint permission, deterministic unknown/incompatible handling and untrusted AI suggestions.

## Boundary of the mock

`MarketPreviewSupervisor` stores resource **descriptions**, not actual OS handles. `close_session` and `close_all` return `CleanupAction` plans: an executor must *carry out and await* them, including cleanup after panic/error and forced termination when safe. This crate deliberately cannot claim actual model unloads, free VRAM, closed sockets or stopped processes.

Before integration, add RAII ownership of real handles, cancellation tokens, timeout accounting, restart-race guards, and coordination with the shared runtime lease service. Hardware acceptance must measure before/after process counts and resource allocation on actual Pop!_OS/NVIDIA CUDA, macOS Apple Silicon, Windows x64/ARM64, Pi 5 CPU and Pi 5 + AI HAT+ 2/Hailo. Standard ARM64 CI cannot certify Hailo.

**CI:** execute standalone `cargo test --manifest-path crates/tihulu-ai-market/Cargo.toml --all-targets` when a Rust toolchain/runner is available. A successful root workspace workflow alone does not establish this agent crate passed.
