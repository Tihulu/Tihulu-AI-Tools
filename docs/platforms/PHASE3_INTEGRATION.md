# Phase 3 — coordinator integrated workspace validation

**Status:** Test harness implemented under Agent 01 scope; OS/hardware acceptance remains pending. Updated 2026-11-12. Workstream [Issue #2](https://github.com/Tihulu/Tihulu-AI-Tools/issues/2), coordinator [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15), sandbox [Draft PR #17](https://github.com/Tihulu/Tihulu-AI-Tools/pull/17). No installer, real device access, model loading, external service mutation, merges or releases.

## Validated integration layout, not inferred runtime success

Coordinator PR #17 on `coord/phase2-integration-sandbox` contains a **six-package root workspace**: `tihulu-ai-core`, `tihulu-ai-cli`, `tihulu-ai-registry`, `tihulu-ai-market`, `tihulu-ai-transport`, and `tihulu-ai-integrations`. The Tauri 2 desktop manifest exists but is expressly excluded in root `Cargo.toml`. These root integration edits remain **coordinator-owned**.

Use the Agent 01 script `scripts/validate_standalone_crates.py --integrated` only on a **coordinator-prepared disposable checkout** containing the six root manifests. It checks root TOML membership, the existence of all six manifests, and explicit Tauri exclusion before allowing Cargo. It cannot validate the correct source revision, genuine Cargo execution without `--execute`, or physical resource release. The script neither merges branches nor fetches source.

`--integrated --execute` runs three commands, stopping on first failure:

```text
cargo fmt --all -- --check
cargo test --workspace --all-targets --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
```

`--allow-network` removes Cargo's `--offline` restriction only when explicitly selected; CI may use it for **approved build dependencies**, not models. Rust builds may write a local `Cargo.lock` and `target/`; always use a disposable checkout. A missing root member gives fail-closed exit code 2, a missing present-but-unchecked-out package manifest exits 3, missing Cargo exits 2, and timeout exits 124. **No result from this helper is proof of GPU/SDR/Jan runtime acceptance.**

For Tauri 2 (excluded from root), separately run:

```sh
python3 scripts/validate_standalone_crates.py --only desktop --execute
```

Only do so on a checkout with `apps/tihulu-ai-desktop/src-tauri/Cargo.toml` plus its web frontend and native prerequisites installed and approved. Neither the Tauri build nor package-signing/bundling has been tested by Agent 01.

## Native Linux x64 / ARM64 and Pi OS acceptance commands

In coordinator-supplied **native** `ubuntu-24.04` or `ubuntu-24.04-arm` runner (Python 3.11+ and Rust already provisioned):

```sh
python3 --version
rustc -vV
cargo --version
python3 -m compileall -q scripts
python3 -m unittest discover -s scripts/tests -v
python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke
python3 scripts/platform_dependency_preflight.py --target linux-x64 --check-host   # x64 runner ONLY
python3 scripts/platform_dependency_preflight.py --target linux-arm64 --check-host # ARM64 runner ONLY
python3 scripts/validate_standalone_crates.py --integrated          # read-only plan
python3 scripts/validate_standalone_crates.py --integrated --execute # offline default
```

For approved CI network dependency provisioning, append `--allow-network` to the final command. Any failed prerequisite check must be reported separately; missing **Tauri** GUI development libraries need not block **headless/core** Rust workspace validation. Physical Pi 5 is **not** equivalent to an Ubuntu ARM64 runner.

For an actual Pi OS 64-bit device (separate future *authorized* hardware session), use `--target pi5-arm64` or `--target pi5-hailo10h` for **read-only tools** preflight. These flags cannot identify Hailo-10H or validate its model/runtime. No accelerator, USB, GPU, NPU or SDR access is performed by the scripts.

## macOS Apple Silicon runner

In a native `macos-15` Apple Silicon runner, with reviewed Python 3.11+ and Rust installed:

```sh
python3 --version
rustc -vV
cargo --version
python3 -m unittest discover -s scripts/tests -v
python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke
python3 scripts/platform_dependency_preflight.py --target macos-arm64 --check-host
python3 scripts/validate_standalone_crates.py --integrated --execute
```

Run `python3 -m compileall -q scripts` in CI as well. Tauri depends on Xcode/Command Line Tools, WKWebView and appropriate signing/entitlement configuration for real packages. Root workspace CI does **not** test Cmd+Q, native App Quit, Force Quit orphan cleanup or Metal memory release.

## Windows x64 / Windows ARM64 runner (PowerShell)

In native `windows-2025` or `windows-11-arm` runner, using Python 3.11+ in PATH:

```powershell
python --version
rustc -vV
cargo --version
python -m compileall -q scripts
python -m unittest discover -s scripts/tests -v
python scripts/platform_lifecycle_smoke.py --owned-child-smoke
python scripts/platform_dependency_preflight.py --target windows-x64 --check-host  # x64 runner ONLY
python scripts/platform_dependency_preflight.py --target windows-arm64 --check-host # ARM64 runner ONLY
python scripts/validate_standalone_crates.py --integrated --execute
```

Invoke only the matching `--target` line. Native MSVC toolchain, Windows SDK and Edge WebView2 runtime are Tauri prerequisites; Windows ARM64 requires a genuine `aarch64-pc-windows-msvc` host/toolchain, not merely x64 emulation. Windows Python dummy termination validates a **direct retained child handle** only. It does **not** use Job Objects and cannot prove descendant cleanup or GPU driver release.

## Process ownership regression coverage

- Synthetic Python supervisor tests: no tasks after Stop; duplicate/concurrent Stop idempotence; stop after an injected timeout retains the failed child handle for retry; POSIX SIGTERM and escalation SIGKILL address only the **recorded isolated owned group**.
- The opt-in real dummy-worker smoke runs **only a Python sleep process**. A second intentionally untracked test-created process is checked as alive after owned-worker Stop, then cleaned up by its own test fixture. Mock borrowed labels `Jan`, `Ollama`, `readsb` and `tar1090` are never treated as PIDs or process names to terminate. **No actual external service was accessed.**
- Linux/macOS crash handling requires explicit parent-death/supervisor strategy beyond process groups. Windows Job Object children/descendant containment must be validated in a real native Rust/Tauri supervisor. No production Rust owner-aware process supervisor is implemented by these scripts.
- True app Quit/Hard Stop acceptance requires authoritative owner IDs and runtime resource ledgers, backend acknowledgements of model unload, per-client lease accounting, and observable app-owned RAM/VRAM/NPU/SDR release. Existing user-managed Jan/Ollama/readsb/tar1090, third-party GPU host sessions and model files must remain untouched. No system-wide GPU memory-zero assertion is valid.

## CI / evidence boundary and current status

- Agent 01 Linux x86_64 Python 3.13.5 locally: `python3 -m compileall -q scripts` **exit 0**; `python3 -m unittest discover -s scripts/tests -v` **33 tests, exit 0**; `python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke` **exit 0**; `python3 scripts/validate_standalone_crates.py --integrated` **exit 2** (correct root-workspace-membership rejection because Agent 01 checkout lacks other crates); `python3 scripts/validate_standalone_crates.py --integrated --execute` **exit 2, no Cargo invoked**; Linux dependency preflight `--check-host` **exit 4** (no cargo/rustc; Linux GUI dev libs missing). The integrated-Cargo invocation is intentionally **NOT RUN** locally; no Rust toolchain was available.
- PR #17 [GitHub Actions run #43155781257](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43155781257) finished **failure** on five native hosted OS jobs: formatter, tests and Clippy **outcomes failed**. Errors reported in integrated Rust sources (e.g. Transport E0505 and Integrations E0308/E0277/E0061), not fixed by Agent 01 because they belong to other agents. This does not establish a passing workspace.
- Existing CI workflow does not automatically include Python scripts on script-only PR changes. Proposed coordinator-only changes to CI triggers/steps are in [CI_PROPOSAL.md](CI_PROPOSAL.md); **Agent 01 did not edit shared CI**.

### Physical hardware still untested

Pop!_OS desktop RTX/CUDA, Pi 5 Raspberry Pi OS 64-bit, Pi 5 AI HAT+ 2 Hailo-10H/HailoRT and actual NPU model disposal, compatible SDR/USB release, real macOS/Tauri Quit and Windows Job Object process trees, signed/notarized installers, systemd/launchd/Windows Service policies, and all on-device model memory release remain **NOT TESTED**. Background OFF / On-Demand / no model keep-warm remains the default. Separate user approval is required for future hardware sessions and installation.
