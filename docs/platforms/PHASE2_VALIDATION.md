# Phase 2: standalone Rust validation and owned shutdown evidence

**Status:** CONFIRMED scope/requirements from [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15) and [Resource Lifecycle v0.9](../RESOURCE_LIFECYCLE.md); tests and platform commands are **proposed opt-in validation tooling**. An executed mock is **not** hardware acceptance. Branch `agent/01-platform`, Draft PR #8, Issue #2. Last review: 2026-11-12.

## Five standalone manifests (verified from current agent branch manifests)

| Name in runner | Separate agent branch | Manifest |
| --- | --- | --- |
| `registry` | `agent/02-discovery` | `crates/tihulu-ai-registry/Cargo.toml` |
| `market` | `agent/03-market` | `crates/tihulu-ai-market/Cargo.toml` |
| `transport` | `agent/04-compute` | `crates/tihulu-ai-transport/Cargo.toml` |
| `integrations` | `agent/05-integrations` | `crates/tihulu-ai-integrations/Cargo.toml` |
| `desktop` | `agent/06-ui` | `apps/tihulu-ai-desktop/src-tauri/Cargo.toml` |

All five currently declare **their own** `[workspace]` section; the foundation root `Cargo.toml` lists only the core and CLI. Thus `cargo test --workspace` at the root cannot substitute for each manifest-path job. Tauri desktop requires native GUI libraries and its adjacent web assets (the draft `frontendDist` is `../../tihulu-ai-web`); the demo's bundle is disabled.

## Safe, opt-in commands

Use a prepared, disposable local checkout/worktree (or coordinator integration checkout) on the **matching agent branch**. The runner **never** fetches, checks out, merges, copies or edits another branch. If a manifest is missing, it reports `NOT PRESENT` and `--execute` fails with exit code **3** before executing any Cargo command. It does not misreport that all five crates passed.

### Linux, macOS and Raspberry Pi OS (Python 3.11+)

```sh
# From the Agent 01 checkout, safe planning only:
python3 scripts/validate_standalone_crates.py

# Execute Registry-only native fmt/test/clippy from an ALREADY PREPARED
# disposable checkout of the Agent 02 branch (repeat --only as required):
python3 scripts/validate_standalone_crates.py \
  --repo-root /path/to/agent-02-checkout --only registry --execute

# Coordinator-prepared integration checkout containing ALL five manifests:
python3 scripts/validate_standalone_crates.py \
  --repo-root /path/to/integration-checkout --execute

# Optional, explicit dependency fetch when a reviewed CI job permits network:
python3 scripts/validate_standalone_crates.py \
  --repo-root /path/to/agent-03-checkout --only market --execute --allow-network

# Python synthetic checks and actual OWNED Python dummy worker smoke:
python3 -m compileall -q scripts
python3 -m unittest discover -s scripts/tests -v
python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke

# Read-only prerequisite inventory, no sudo/apt and no Hailo hardware access:
python3 scripts/platform_dependency_preflight.py --target linux-x64 --check-host
python3 scripts/platform_dependency_preflight.py --target linux-arm64 --check-host
python3 scripts/platform_dependency_preflight.py --target macos-arm64 --check-host
python3 scripts/platform_dependency_preflight.py --target pi5-arm64 --check-host
python3 scripts/platform_dependency_preflight.py --target pi5-hailo10h --check-host
```

Only use a `--target` matching the *actual native host* for `--check-host`; mismatched OS/architecture returns exit 3. A Pi-labelled ARM64 probe reports only architecture/tool prerequisites, **never** confirms that a Pi or Hailo exists.

### Windows PowerShell (Python 3.11+)

```powershell
py -3 scripts\validate_standalone_crates.py --repo-root "C:\path\to\agent-05-checkout" --only integrations --execute
py -3 scripts\validate_standalone_crates.py --repo-root "C:\path\to\agent-06-checkout" --only desktop --execute
py -3 -m unittest discover -s scripts\tests -v
py -3 scripts\platform_lifecycle_smoke.py --owned-child-smoke
py -3 scripts\platform_dependency_preflight.py --target windows-x64 --check-host
# On a NATIVE Windows ARM64 Python/Rust/MSVC environment only:
py -3 scripts\platform_dependency_preflight.py --target windows-arm64 --check-host
```

`--execute` runs `cargo fmt --check`, `cargo test --all-targets --offline` and `cargo clippy --all-targets --offline -- -D warnings` **per individual manifest**, stopping at first nonzero exit. Default is planning only. `--allow-network` intentionally removes Cargo offline restriction but does not download models, run installers or contact providers. Cargo builds may create local `target/` and `Cargo.lock` in the disposable checkout; do not run against a shared working tree. The Rust toolchain, dependencies, and native libraries must already be provisioned by a separately authorized runner administrator. The command timeout is 900 seconds; after a timeout inspect lingering Cargo child processes (particularly Windows, where the test runner is **not** a Job Object supervisor).

The script uses native host compilation; do not claim native ARM64 based on cross-compilation or x64 emulation. `cargo fmt` success is formatting only. Each crate's actual unit tests and lint must complete. For Windows ARM64, verify the *native* `aarch64-pc-windows-msvc` toolchain rather than assuming x64 emulation suffices.

## Resource lifecycle regression mapping

| Case | Agent 01 synthetic evidence | Pending native acceptance |
| --- | --- | --- |
| L-01 | Owned Python dummy process stops and is reaped; no model loaded | Worker plus runtime/model allocation and attributed RAM/VRAM release |
| L-08 | Fake stubborn child escalates to retained owned handle only | True POSIX descendant group, Windows Job Object with spawned grandchildren |
| L-10 | Termination/timeout policy documented; bounded dummy smoke | Crash/Force Quit orphan containment across actual OS runtime |
| L-11 | Idempotent and simultaneous Stop; no new work after Stop | Real relaunch and lease reference reclamation |
| L-12 | Untracked **test dummy** survives stopping the owned one; borrowed provider labels untouched | Independent shared host and other-client lease survive UI Quit |
| L-14 | Python tests can run on hosted runner OS matrix **once coordinator adds CI** | Physical Pi, Hailo, RTX, SDR, macOS Quit and Windows Job Objects |

The untracked worker in tests is another **test-created Python process**, not Jan/Ollama/readsb/tar1090. The suite never connects to external services. No model inference, downloads, SDR scan or accelerator probe is performed.

## Recording evidence

For each native run record: exact checkout branch + immutable commit; host OS/version and `uname -m` / `rustc -vV` (or Windows equivalent); Python/Rust/Cargo versions; manifest path; `cargo fmt`, `cargo test`, `cargo clippy` individual exit codes/logs; whether dependencies were already cached, whether network was opted in; Tauri native dependencies; screenshot/log of Job Object/child outcome if tested. Mark any missing/bypassed item **NOT RUN**, never PASS.

For hardware-only gates, separately record specific app owner/session IDs, model/host leases, before/after attributable CPU/GPU/NPU memory, runtime unload completion, USB/SDR handle release, independent clients unchanged, driver/firmware versions and exception/timeout results. Aggregate system VRAM/RAM is **not** required to become zero. The coordinator alone may approve integration and real-machine acceptance.
