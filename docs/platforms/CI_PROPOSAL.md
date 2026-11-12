# Agent 01 — isolated CI proposal (not applied to shared workflow)

**Status:** PROPOSED for coordinator review. Do **not** edit `.github/workflows/rust-core.yml` from this branch.

## Existing baseline

Foundation `.github/workflows/rust-core.yml` uses `ubuntu-24.04`, `ubuntu-24.04-arm`, `windows-2025`, `windows-11-arm`, `macos-15` for `cargo fmt --all -- --check`, `cargo test --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings`. Foundation run #7 passed 5/5 jobs. **Its path filter excludes `docs/platforms/**` and `scripts/**`**, so docs/scripts-only Agent 01 PR commits do not automatically run that workflow.

## Proposed additional workflow (coordinator-owned)

- Trigger on PR to `feat/rust-foundation` when `scripts/**`, `docs/platforms/**` or relevant lifecycle/CI code changes; optionally manual dispatch.
- Read-only repository permissions, no credentials, no hardware USB, no service management, no model download, no installer, no publish or release.
- Matrix: same five OS images as foundation subject to runner availability. Setup Python 3.11+ through a pinned, reviewed action if absent. Run:
  1. `python -m compileall -q scripts` — syntax only.
  2. `python -m unittest discover -s scripts/tests -v` — mock ownership/idempotency/timeout/borrowed preservation.
  3. `python scripts/platform_lifecycle_smoke.py --owned-child-smoke` — bounded **owned dummy child** only. Windows verifies a direct handle only, not Job Objects/descendants.
- Retain command exit codes, os/ISA, test list, logs and complete warnings. Ensure cleanup in a `finally` block and bounded worker lifetime in case CI cancels tests. Disable auto-retry that could leave worker processes unnoticed.
- Optional separate Cargo validation invokes `python scripts/verify_rust_workspace.py --execute` with pre-populated cache. Network fetching needs a distinct explicit workflow policy; script defaults `--offline`.
- Distinguish **hosted-runner evidence** from native package, Raspberry Pi 5, AI HAT+ 2, CUDA RTX, Metal, Windows Job Objects, SDR and real memory-release trials. Track those hardware gates explicitly with signed test logs / before-after owned lease counts.

## Pending test gate

No CI check triggered for these proposed tests until the coordinator reviews and adds an allowed workflow. Run locally where Python is available, report that exact OS/interpreter and result; do not imply hosted CI passed.

## Phase 2 standalone Rust crate matrix proposal (Issue #15)

**Only coordinator may edit or add `.github/workflows/**`.** Root `cargo test --workspace` sees only the foundation core and CLI, not the five standalone manifests. For each of the five independent crates, perform `cargo fmt`, `cargo test --all-targets` and `cargo clippy --all-targets -- -D warnings` **with `--manifest-path`** on a review-pinned branch head or an integration checkout that contains all manifests. Test in native runners `ubuntu-24.04`, `ubuntu-24.04-arm`, `macos-15`, `windows-2025`, `windows-11-arm` when available; give each crate/OS an independent result, including `NOT RUN` when Tauri native dependencies are absent.

Proposed coordinator runner invocations from a reviewed checkout:

```sh
python3 scripts/validate_standalone_crates.py                 # read-only plan
python3 scripts/validate_standalone_crates.py --execute       # requires all five manifests
python3 -m unittest discover -s scripts/tests -v
python3 scripts/platform_lifecycle_smoke.py --owned-child-smoke
```

The runner supports `--repo-root PATH --only registry|market|transport|integrations|desktop --execute` for branch-specific checkouts and defaults Cargo to `--offline` (dependency download explicit with `--allow-network`). Don't use it as a source merge mechanism; no other repositories may be touched. Before a Tauri desktop job, provide OS-specific libraries as documented in [installer prerequisites](INSTALLER_DEPENDENCIES.md); do not infer Pi/Hailo/RTX/SDR or installer acceptance from a passed CI job. Python tests on all five runners are separate from Rust crate jobs. Include scripts/docs paths in future CI triggers if approved.

Physical lifecycle tests for L-01/L-08/L-10/L-11/L-12/L-14 still require managed workers, native Windows Job Objects and verified OSX/Linux/Pi shutdown behavior. The Python harness only manages its own disposable dummy processes. See [Phase 2 guide](PHASE2_VALIDATION.md).
