# AI-01 PHASE5-PLATFORM — native packaging and lifecycle gates

**Status: proposed validation contract and safe test scripts, not a release or native/hardware acceptance.** November 12, 2026. Reference [Draft PR #19](https://github.com/Tihulu/Tihulu-AI-Tools/pull/19), base branch coord/phase4-native-inventory, and [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15). This PR changes only new scripts/tests and this document. No shared workflows, other agents' source, model files, installed apps or services.

## Native build / Tauri 2 readiness matrix

Phase 4 native Tauri IPC only links real Rust contracts **read-only**. It reports zero source scans, no loaded model/runtime, no Jan/GPU connection and no verified cleanup. Tauri 2 is excluded from the root workspace, builds independently from apps/tihulu-ai-desktop/src-tauri/Cargo.toml, and bundling is disabled in tauri.conf.json.

| Host | Native Rust triple | Native prerequisites | Unverified hardware / OS gates |
| --- | --- | --- | --- |
| Linux x64 (Debian, Ubuntu, Pop!_OS) | x86_64-unknown-linux-gnu | Rust/Cargo, Node, C/C++ build chain, pkg-config, WebKitGTK 4.1, GTK3, librsvg, OpenSSL; optionally libayatana-appindicator and libxdo | X11/Wayland/COSMIC window, native Quit, .deb/AppImage, GPU/SDR |
| Linux ARM64 | aarch64-unknown-linux-gnu | ARM64-native equivalents and ABI | Raspberry Pi OS and Hailo are NOT implied by Ubuntu ARM64 CI |
| Windows x64 | x86_64-pc-windows-msvc | Native MSVC C++ tools, Windows SDK, Edge WebView2, Rust, Node | Installer/UAC, signing, Job Object descendants |
| Windows ARM64 | aarch64-pc-windows-msvc | Native ARM64 MSVC/linker/SDK and WebView2 | x64 emulation is NOT native ARM64 validation |
| macOS Apple Silicon | aarch64-apple-darwin | Apple SDK, Xcode or Command Line Tools, WKWebView, Rust, Node | .app/.dmg, notarization, Gatekeeper, Cmd+Q, Metal cleanup |

Official references: [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) and [bundle configuration](https://v2.tauri.app/reference/config/). Hardware Pi 5 and AI HAT+ 2/Hailo-10H acceptance (Pi OS 64-bit, HailoRT, firmware and compatible model artifacts) remains a later separately authorized task. ARM64 build success does not demonstrate Hailo inference.

## Exact cross-platform validation commands

Python 3.11+ is required for tomllib. All new tools are read-only by default. The platform runner checks six expected root workspace manifests, separate Tauri manifest, disabled bundling, read-only frontend path and browser tests. It prints Cargo fmt/test/Clippy/build and Node test commands. Only the explicit --execute-build flag runs native builds; no installer, bundle, GUI startup or service is launched. Cargo defaults offline; --allow-network (only with --execute-build) permits build dependency retrieval, **not** model downloads. Use a disposable checkout because Cargo may create target directories/lockfiles.

### Linux x64, from repository root on native x64 runner

    python3 -m compileall -q scripts
    python3 -m unittest discover -s scripts/tests -p 'test_phase5_*.py' -v
    python3 scripts/phase5_platform_smoke.py --target linux-x64
    python3 scripts/phase5_platform_smoke.py --target linux-x64 --preflight
    # OPTIONAL: reviewed toolchain, disposable native checkout ONLY
    python3 scripts/phase5_platform_smoke.py --target linux-x64 --execute-build
    # Separate opt-in existing disposable dummy-worker smoke
    python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke

### Linux ARM64

Run the Linux commands with --target linux-arm64 on a **native ARM64** machine such as Ubuntu 24.04 ARM64. Report rustc -vV, architecture and ABI. The runner refuses executing native checks when OS/architecture mismatch. Do not report a Pi/Hailo or real hardware result.

### macOS ARM64 Apple Silicon

    python3 -m unittest discover -s scripts/tests -p 'test_phase5_*.py' -v
    python3 scripts/phase5_platform_smoke.py --target macos-arm64 --preflight
    python3 scripts/phase5_platform_smoke.py --target macos-arm64 --execute-build
    # Read-only inspect ONLY a previously created, authorized macOS artifact
    python3 scripts/phase5_platform_smoke.py --target macos-arm64 --inspect-artifact /approved/path/Tihulu.app

### Windows x64 / Windows ARM64 PowerShell

    py -3 -m unittest discover -s scripts/tests -p "test_phase5_*.py" -v
    py -3 scripts/phase5_platform_smoke.py --target windows-x64 --preflight
    py -3 scripts/phase5_platform_smoke.py --target windows-x64 --execute-build
    # Use --target windows-arm64 only on native Windows ARM64 host
    py -3 scripts/phase5_platform_smoke.py --target windows-x64 --inspect-artifact "C:\approved\Tihulu.msi"

## Packaging smoke without installation

The read-only --inspect-artifact path hashes/size-checks an **existing** .deb, .AppImage, .msi, .msix, .exe, .pkg, .dmg file or macOS .app directory. The .app inspection reads Info.plist and requires a non-symlink executable, bounded file tree and no symlinks; such checks may reject otherwise legitimate bundles for manual review. It NEVER executes a package, validates signature, notarization, package contents or successful installation. Report fields always indicate signature_verified=false, installer_executed=false, launch_tested=false, hardware_tested=false. Independent publisher signature and OS-specific installer acceptance are mandatory future steps.

## Native GUI startup: separate observational gate

Cargo tests/build are not proof a window launched or closed correctly. The new startup report validator only parses a supplied observer JSON; it **does not start the GUI**. In a separately authorized native test session, independently record immutable source SHA, OS/arch, approved binary hash, visible Demo window, both read-only IPC probe results, and GUI-owned process/child state before/after Quit. No service/network/model/SDR access should be activated. Capture logs/screenshots without secrets. Then run:

    python3 scripts/phase5_startup_report.py --report /approved/native-observation.json

Report schema must include schema_version=1, target platform, observer, true booleans for gui_window_observed, native_ipc_observed, external_services_untouched, native_process_exit_observed, and ui_native_module_status matching Phase 4 exactly: source native_in_memory_read_only; five linked modules; zero registry records/approved scans; gui_owner_registered/backend_verified/jan_connected/gpu_connected/cleanup_verified all false. Forged connected or cleanup claims are rejected. **Passing schema checks does not authenticate an observer or prove real startup.**

## User-level install/uninstall/rollback protocol (design only)

1. **Explicit consent and provenance** before any future install: verify OS/ISA, immutable commit/version, trusted publisher signature, artifact hash, dependencies and exact dry-run file writes; no hidden privilege elevation, network listener, system service, start-at-login, firewall rule or model download. On-Demand, Background OFF and no keep-warm are default.
2. **Per-user paths**, and distinct program binaries, settings, logs, user models and temporary files. Preserve independent TIHULU application directories, existing Jan/Ollama model stores and recordings. No auto-move/copy/import, privileged scan or broad deletion.
3. **Owned Quit before update/uninstall:** deny new tasks, release the GUI's model/host leases, close owned runtime contexts/streams/device handles, terminate/reap only tracked owned workers. If an independently opted-in GPU Host still serves others, keep it running. On incomplete cleanup report a leak; **never** use kill-by-process-name, generic service stop or external device reset.
4. **Uninstall/rollback:** remove only manifest-owned program files and shortcuts after verification; preserve model/user data by default. An opt-in deletion request requires separate confirmation. Reject unowned/symlink paths. Restore a version-pinned, signed previous binary and owned launcher atomically on failure. Do not roll back other applications or provider states.
5. Native Windows MSI/MSIX/NSIS, Linux .deb/AppImage and macOS .app/.dmg packaging, signing, user-level installer/uninstaller and interrupted rollback remain **NOT IMPLEMENTED/NOT TESTED** here.

## GPU Host lifetime distinct from GUI and owner-safe Stop

- A GPU Host may run independently **only if explicitly enabled**; it is not automatically started by GUI or at login. GUI Quit cancels GUI-owned jobs and host client leases, reaps only GUI-owned children and releases its model, sockets and SDR handles; does not stop a host serving authorized independent clients.
- Stop AI/app Hard Stop releases only that app's leases and associated model/runtime allocations. A shared model unloads on its **last** active authorized lease, once. A separately authorized idle host process may remain running while holding no default keep-warm model.
- Quit All TIHULU-Owned Services is a distinct user-confirmed operation with warning; never terminate pre-existing Jan, Ollama, readsb, tar1090 or other application services, including those sharing GPU/SDR. No broad PID/name/port kills.
- Native POSIX SIGINT/SIGTERM and isolated child process groups, Windows Job Objects with kill-on-close for owned descendants, and macOS Quit/Force Quit need real implementation and OS tests. Process groups alone do not solve crash orphans; use supervised lease expiry. Python tests operate only synthetic handles or a separate opt-in dummy worker.
- **RAM/VRAM verification** requires backend model unload acknowledgements, per-owner CPU/GPU/NPU allocation accounting, owned child termination and SDR handle release. Other programs' GPU use may remain. Never demand zero system-wide VRAM.

| Acceptance gate | In this PR | Still needs native/hardware testing |
| --- | --- | --- |
| Owned Stop/GUI Quit vs independent host (L-01,L-03,L-04,L-12) | Pure fake lease model, last lease once | Two actual apps and GPU host, model unload/VRAM |
| Borrowed Jan/Ollama/readsb/tar1090 (L-06,L-07) | Names are mock metadata only, no external handles touched | Real non-invasive before/after service observation |
| Crash/Stop (L-08,L-10,L-11,L-14) | Synthetic ownership tests, existing opt-in Python child test | OS Job Objects, force-exit/orphan recovery, actual process descendants |
| Packages | Safe no-op plans, hash-only metadata | Real native Tauri GUI, signed installers, per-user install/update/uninstall/rollback |

## Local evidence and remaining blockers

Agent 01 authoring environment: **Linux x86_64, Python 3.13.5**. Executed python3 -m compileall -q scripts (exit 0) and python3 -m unittest discover -s scripts/tests -p 'test_phase5_*.py' -q (**32/32 passed**, exit 0), using temporary fake fixture repositories, fake model memory counters and mocked build commands. No installer, service, model, real hardware, native Tauri window or Jan config was accessed. New committed Python/test files were byte-verified using Git blob SHAs of the locally executed content.

**cargo and rustc are absent** locally; no native Rust build, Tauri GUI startup, package or RAM/VRAM/SDR release has been executed. The upstream PR #19 Rust Core CI is separate evidence for portable Rust contracts and cannot be substituted for this Phase 5 platform suite. Coordinator must explicitly run approved native CI and Tauri readiness, while shared workflow modifications remain outside Agent 01 scope. Real Pi/Hailo, CUDA/Metal, Windows Job Objects, macOS GUI Quit and signed installers remain NOT TESTED.
