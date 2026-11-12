# Native shutdown / ownership mapping

**Status:** CONFIRMED behavioral requirement in [v0.9 lifecycle contract](../RESOURCE_LIFECYCLE.md); OS-specific implementation below is **PROPOSED**, with a bounded developer-only dummy-child test in `scripts/`. There is no production runtime supervisor or GPU model worker in the foundation.

## Shared invariant and ordering

Record an immutable app/session owner token and native handle **at spawn/allocation time**. Registration must be atomic with task start: once `STOPPING`, deny new workers and leases. On Quit, Hard Stop, OS-close or recoverable signal: **reject new tasks → cancel per-owner work/remote leases → drain with deadline → close owned model contexts, GPU/CPU/NPU buffers, audio/SDR/USB/network handles → terminate/reap non-cooperative owned workers → verify and report**. Repeat stop is idempotent. Never infer ownership from a process name, port, device type or installed model path.

The app can cancel a borrowed Jan/Ollama request, **not** kill or unload their independently managed process/model. The same holds for pre-existing readsb/tar1090 and other apps' SDR handles. A shared TIHULU host retains models while another authorized client lease exists; last lease causes unload unless explicitly independent residency policy was chosen. An independent GPU-host service must be separate from a UI window and **opt-in**. No model file is deleted because a session ended.

## POSIX Linux / Debian / Ubuntu / Pop!_OS

- Capture `SIGINT`/`SIGTERM` in the Rust application, using the same bounded cleanup pipeline as menu Quit; signal handlers should only notify an async/supervisor-safe shutdown channel (no unsafe blocking in a raw signal handler). `SIGKILL`/power loss cannot execute normal in-process cleanup.
- For an exclusively owned helper, start an isolated POSIX session/process group and retain the returned `Child` handle and group ID. Gracefully request cancellation / `SIGTERM` to **that recorded group only**; after bounded wait, escalate to `SIGKILL` on the same verified owned group, and reap the child. Use platform-appropriate identity checks against PID/group reuse and never signal a group containing borrowed helpers.
- In a multithreaded parent, avoid arbitrary `pre_exec` operations; OS-native spawn and explicit session/group setup require review. On parent crash, process groups alone do **not** guarantee automatic child cleanup: an explicit supervisor strategy (e.g. Linux parent-death signal when safely applicable, cgroup scoped to owned workers, or a manager heartbeat) needs design and testing.
- No global systemd unit in default On-Demand mode. Optional independent host uses explicitly approved user-scope unit; Quit of client releases its lease but does not stop a host serving other clients.

## macOS Apple Silicon

- Wire application menu `Quit`, Cmd+Q and last-window close (if configured to exit app) into one native app lifecycle/termination coordinator. Handle system termination requests and application shutdown events with an appropriate bounded async deferral; do not assume a webview `beforeunload` callback is sufficient.
- For owned local children use retained child handles and isolated POSIX groups with bounded graceful/force sequence. App Nap, suspended clients and abrupt Force Quit cannot be guaranteed to run a destructor; owner/host lease expiry or supervisor recovery is required.
- `.app` lifecycle, code signing, notarization, Metal/Core ML release and actual window close semantics require a real Tauri app and native macOS validation; a CLI matrix job is not that test. No launch agent automatically.

## Windows x64 / ARM64

- Production worker spawning should create a **per-app-session Windows Job Object** with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` where applicable, an explicit owner mapping and retained Job/Process handles. Assign only **newly created owned** workers; do not attach Jan/Ollama, arbitrary PIDs or processes borrowed from another app.
- Validate Job assignment, nested/pre-existing job constraints, child inheritance/breakaway restrictions and race-free suspended process creation. A worker that cannot be safely assigned must fail to start or use a separately reviewed safe supervision path; never silently claim child-tree containment.
- On Exit, first send cooperative cancellation through owned IPC; wait boundedly; terminate only the owned Job Object on timeout; close/reap handles. `TerminateJobObject` is last resort and must never affect another service or client. Console Ctrl+C, Ctrl+Break, GUI close, Windows session ending and service-hosted modes have different delivery/timeout behavior; all must have explicit mappings.
- Job close can reclaim owned child processes after crashes when configured, but GPU driver release and orphaned remote leases remain separate observable tests. The included Python smoke harness on Windows terminates **only its direct dummy child**, and does **not** prove descendant Job Object behavior.

## Raspberry Pi 5 and AI HAT+ 2

- Base Pi OS 64-bit native lifecycle is Linux ARM64 with the same owned-group rules. Do not assume the `ubuntu-24.04-arm` runner has Pi hardware, Hailo PCIe, kernel modules, memory capacity or USB/SDR behavior.
- Hailo-10H work requires separately supported HailoRT runtime/artifact and explicit backend-specific model/context unload plus closed accelerator handles; verify attributable NPU allocation, not system-wide zero memory. Do not attempt to force-reset PCIe or kill another app's Hailo runtime.
- Prefer CLI/headless on demand for memory-constrained Pi. If a headless API or LAN UI is later opted in, require auth + lease expiry on client disconnect, and independent-service policy if server mode remains active. Never autostart via systemd by default.

## Test/acceptance evidence

| Regression | Current agent test or design | Missing proof |
| --- | --- | --- |
| L-01 | Python dummy worker is cancelled, waited/reaped; mock owner resource order | Real AI model unload, RAM/VRAM release |
| L-08 | Injected non-cooperative fake child validates bounded force escalation and no borrowed signaling | Real child trees on all OS, Windows Job Object |
| L-10 | OS crash/Force Quit recovery documented | Kill-on-job-close, parent-death and host TTL |
| L-11 | Idempotent Stop and refusal of post-stop starts in mock | Real relaunch with hardware lease |
| L-12 | Borrowed/independent host mock left untouched | Two live clients + host |
| L-14 | Portable Python synthetic smoke tests proposed for five native CI environments | Real GPU/NPU/SDR and installer acceptance |

`python3 -m unittest discover -s scripts/tests -v` exercises fake ownership/stop transitions; `python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke` explicitly spawns **only a Python sleep child**, stops/reaps it and reports native scope. Tests never start Jan/Ollama/readsb/tar1090, SDR, Hailo or GPU inference. Tests passing is evidence of the **harness**, not proof the eventual Rust app obeys the invariant.
