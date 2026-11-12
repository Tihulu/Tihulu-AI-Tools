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
