# Agent 01 — platform, build and distribution plan

**Status:** PROPOSED implementation architecture, not released installers or hardware acceptance. **Scope:** `docs/platforms/**` and `scripts/**` on `agent/01-platform`. See [resource contract](../RESOURCE_LIFECYCLE.md), [workstreams](../AGENT_WORKSTREAMS.md), Issue #2 and foundation Draft PR #1.

## Target and evidence matrix

| Target | Rust target triple | Foundation runner | Planned deliverable | Not verified by runner |
| --- | --- | --- | --- | --- |
| Debian / Ubuntu / Pop!_OS x64 | `x86_64-unknown-linux-gnu` | `ubuntu-24.04` | Native CLI first; proposed Tauri 2 .deb/AppImage after desktop app exists | Pop!_OS, Debian versions, GPU/CUDA, desktop integration |
| Debian / Ubuntu ARM64 | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` | Native CLI; .deb per distro/ABI | Raspberry Pi OS and actual ARM hardware |
| Raspberry Pi 5 64-bit | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` **CPU only** | Pi OS 64-bit native CLI/headless local UI, optional desktop only if tested | Pi firmware, CPU/memory, PCIe, SDR/USB and storage |
| Pi 5 + AI HAT+ 2 / Hailo-10H | `aarch64-unknown-linux-gnu` | `ubuntu-24.04-arm` **no Hailo** | Same host binary; optional separately verified HailoRT adapter/artifacts | HailoRT, Hailo-10H model compatibility, actual NPU memory release |
| macOS Apple Silicon | `aarch64-apple-darwin` | `macos-15` | CLI first; proposed signed/notarized Tauri .app/.dmg | Gatekeeper signing/notarization and Metal/ANE runtime |
| Windows x64 | `x86_64-pc-windows-msvc` | `windows-2025` | CLI first; proposed signed MSIX or NSIS after app exists | Installer UAC, GPU drivers and owned descendant teardown |
| Windows ARM64 | `aarch64-pc-windows-msvc` | `windows-11-arm` | Native ARM64 CLI, native MSIX/NSIS only after validated | x64 emulation assumptions, native engine availability |

**Confirmed baseline evidence:** foundation documentation records successful GitHub Actions run [#7](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43148925278), 5/5 Rust-core matrix jobs, including format/test/clippy. This is evidence of the foundation workspace only. It does not validate this branch's smoke scripts, Tauri, packaging, real VRAM/RAM teardown or any named hardware.

## Current vs planned

- **Current (confirmed):** Rust workspace and consent-gated read-only CLI; no native desktop bundle, installer, daemon, inference engine or Hailo plugin.
- **Proposed:** Rust-first manager shared across app consumers; Tauri v2 desktop on tested platforms. Linux Tauri 2 commonly needs WebKitGTK/GTK and related dev libraries at build time; packaging must determine actual distro-specific dependencies using the future desktop app's pinned Tauri toolchain, not a generic runner build.
- **Proposed Pi fallback:** headless native Rust API plus responsive browser UI on an explicitly chosen loopback/LAN binding and authenticated pairing, when authorized and implemented. No implicit network listener, WebUSB requirement, startup daemon or publicly exposed port. Web UI is **not** presently implemented or validated.
- **Proposed artifact policy:** one binary/bundle per OS/ISA; no universal claim that Hailo, Metal, CUDA and CPU runtimes are interchangeable. Include immutable release tag, commit, target triple, dependency inventory, SHA-256 manifest, provenance and OS signature when supported.
- **Localization target:** UI English default; Turkish and German; locale files/app UI are Agent 06's responsibility.

## Safe development build procedure

From the repository root, with Rust toolchain installed, run `python3 scripts/verify_rust_workspace.py --execute` (or `py -3 scripts/verify_rust_workspace.py --execute` on Windows). The script defaults to **Cargo offline mode** and does not install tools. If dependencies are not cached, it exits with an error rather than accessing the network; `--allow-network` is a separate explicit opt-in.

The proposed CI job should invoke `python3 -m unittest discover -s scripts/tests -v` on each available OS. Optionally run `python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke` to spawn and immediately clean up **only** a short-lived Python dummy child. See [CI proposal](CI_PROPOSAL.md).

## Packaging gates

1. **Build:** lock dependency versions, reproducible target matrix and toolchain, Rust fmt/test/clippy; separate Tauri matrix when an actual app exists.
2. **Package:** per-architecture ABI checks; Linux libc/distro requirements, Windows native vs emulated dependencies, macOS minimum OS/signing; Pi OS 64-bit test and headless fallback.
3. **Install:** consented per-user install by default; no privileged daemon, auto-start, firewall rule, model download, external app modification or automatic GPU host.
4. **Quit:** owned workers/models/USB leases cleaned by future runtime; native synthetic tests and then observable real hardware tests required.
5. **Uninstall/rollback:** stop only owned running components with explicit consent; preserve models, user data and all borrowed services; test version rollback (see [installer protocol](INSTALLER_SAFETY.md)).
6. **Release:** independent approval after native installer, signing, hardware and security acceptance; this PR must **not** ship any release.

## Coordinator-owned follow-ups (not modified here)

The coordinator may add `scripts/**` tests to the existing Rust workflow's trigger and jobs. The current `.github/workflows/rust-core.yml` filters PR paths to Rust workspace files and therefore a docs/scripts-only PR **does not automatically run that workflow**. The coordinator must review any future root Cargo, Tauri, bundle, CI or core supervisor edits.

## Phase 2 — executable validation workstream

- [Standalone five-crate command matrix, test evidence and blockers](PHASE2_VALIDATION.md).
- [Native Tauri/package prerequisites and Pi Hailo boundaries](INSTALLER_DEPENDENCIES.md).
- The optional Python runner `scripts/validate_standalone_crates.py` operates on explicitly supplied disposable agent/integration checkouts; it never merges or installs code. `scripts/platform_dependency_preflight.py` is read-only.
- See [CI proposal](CI_PROPOSAL.md) for coordinator-owned workflow changes only. Agent 01 does not edit shared `.github/workflows/**`.
