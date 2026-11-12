# Platform prerequisites and installer acceptance — read-only inventory

**Status:** PROPOSED platform/package checklist, not installed or verified. No working installer or native package is built in Agent 01. Use `python3 scripts/platform_dependency_preflight.py --target TARGET` for a plan; add `--check-host` only on the matching native host for **read-only tools and pkg-config** checks. Never run this script as root or as an installer.

| Host | Development prerequisites for Tauri 2 | Packaging & runtime gate | Pending native acceptance |
| --- | --- | --- | --- |
| Ubuntu/Debian/Pop!_OS x64, Linux ARM64 | Rust 1.82+ per manifest; C/C++ build tools, `pkg-config`, WebKit2GTK **4.1** development package, GTK3, OpenSSL, librsvg, optional Ayatana AppIndicator | Distro-specific .deb/AppImage test, compatible GLIBC and graphical session; headless Rust CLI has no WebKitGTK requirement | Actual Ubuntu/Debian/Pop!_OS versions; GNOME/KDE/COSMIC/Wayland behavior; driver/SDR release |
| macOS Apple Silicon ARM64 | Native `aarch64-apple-darwin` Rust, Xcode or Command Line Tools, macOS SDK, WKWebView | Native `.app` Quit, entitlements, signature, notarization and Gatekeeper verification; no automatic launch agent | Actual M-series hardware, model/Metal unload, Cmd+Q/last window/Force Quit |
| Windows x64 | `x86_64-pc-windows-msvc` Rust, MSVC Desktop C++ Build Tools, Windows SDK and Edge WebView2 runtime | User-scope installer, MSIX/NSIS choice; MSI may need VBSCRIPT feature; no startup service | Real `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` owned-descendant check, GPU driver, UAC/rollback |
| Windows ARM64 | Native `aarch64-pc-windows-msvc`, ARM64-capable MSVC link tools and Windows SDK, WebView2 | Native ARM64 artifact/runtime; emulated x64 process does not count | Native ARM64 runner and native process tree, per-dependency ARM64 compatibility |
| Raspberry Pi 5 ARM64 | **Raspberry Pi OS 64-bit**, native Rust ARM64/CPU; CLI/headless path preferred; only if desktop app supported, Linux WebKitGTK and GUI | Verify actual Pi OS release, firmware/CPU/memory, package ABI, storage/USB/SDR leases; no implicit systemd service | On-device CPU, storage, network, process groups and resource limits |
| Raspberry Pi 5 + AI HAT+ 2 | Pi 5 + **Hailo-10H**; current official software guide requires Raspberry Pi OS **Trixie 64-bit**, kernel device driver/firmware, HailoRT and vendor-supported artifact | **Separately approved/manual** `hailo-h10-all`/HailoRT provisioning, package conflicts, reboot/driver acceptance; not part of this PR | Physical Hailo-10H identify, real model session, last lease unload, attributable NPU memory and crash recovery |

### Linux package candidates (information, not an install command)

The current official [Tauri 2 prerequisites](https://tauri.app/start/prerequisites/) list `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libxdo-dev`, `libssl-dev`, `libayatana-appindicator3-dev` and `librsvg2-dev` for Debian-family builds (package availability varies by distribution/version). The Python preflight only checks `cargo`, `rustc`, `cc`, `pkg-config` and the `webkit2gtk-4.1`, `gtk+-3.0`, `librsvg-2.0`, `openssl` pkg-config modules. **This is intentionally incomplete**: passing preflight is not installer or Tauri acceptance.

On Linux ARM64 GitHub Actions runners, the WebKitGTK build may work while Raspberry Pi OS deployment still fails because Ubuntu ARM64 is not Pi hardware. A headless Pi web service must be explicitly authorized and authenticated; default Background OFF / On-Demand.

### Hailo-10H separation

The official [Raspberry Pi AI software guide](https://www.raspberrypi.com/documentation/computers/ai.html) distinguishes `hailo-h10-all` for AI HAT+ 2 from `hailo-all` for Hailo-8/8L variants; they must **not** be treated as interchangeable or co-installed. A CPU-capable Linux ARM64 binary or `hailortcli` on PATH does not demonstrate the Hailo-10H's compatibility with a specific runtime/model. The **read-only preflight does not call `hailortcli`**, PCIe drivers, SDR, CUDA or model runtimes. Any device identify/firmware probe requires an explicitly authorized physical acceptance session; no driver/config update by these scripts.

### Clean removal and resource retention gates

Default install/uninstall does not install or disable systemd/launchd/Windows Services, create login auto-start, touch model paths, kill processes by name, remove user recordings or change external Jan/Ollama/readsb/tar1090. Stop only app-owned child handles/model/SDR leases by the runtime owner ledger; preserve independently configured GPU hosts with active authorized clients. Partial installer rollback must restore only TIHULU-owned binaries/manifest. Last model lease means unload **on runtime**, not delete on disk. See [installer safety](INSTALLER_SAFETY.md), [native lifecycle](LIFECYCLE.md), and [phase 2 validation](PHASE2_VALIDATION.md).
