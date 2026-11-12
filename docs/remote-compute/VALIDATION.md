# Validation and acceptance ledger — Agent 04

## Evidence boundary

The branch contains dependency-free Rust typed protocol contracts, an offline in-memory host and an in-memory request/response transport, plus synthetic regression tests (including Phase 2 single-authority handoff fixtures). **No GPU host daemon, HTTPS listener, TLS cryptography, staging service, CUDA/NPU inference, model deletion or filesystem access is started.**

Run from repository root with Rust >= 1.82:

```bash
cargo test --manifest-path crates/tihulu-ai-transport/Cargo.toml --all-targets
cargo fmt --manifest-path crates/tihulu-ai-transport/Cargo.toml --check
cargo clippy --manifest-path crates/tihulu-ai-transport/Cargo.toml --all-targets -- -D warnings
```

The current authoring environment lacks `cargo`/`rustc` and the root workspace intentionally excludes this agent crate. **These tests were authored but cannot be reported as executed locally.** An automated CI check directly invoking this standalone manifest is a coordinator review step; do not confuse existing root-workspace CI green status with Agent 04 crate test success.

## Phase 2 evidence

**26 test functions are currently authored** in `crates/tihulu-ai-transport/tests/mock_lifecycle.rs` (15 baseline plus 11 Phase 2 additions), as counted from the committed source; this is a source inspection only, NOT an executed pass result.

Phase 2 scenarios include: two distinct Pi/app devices sharing an owned PC model, coordinator ledger-owned last-lease cleanup acknowledgement and failure/retry, ordered replay-safe release events, wrong resource/generation ACK denial, app-launch/task scoped cancellation, one Pi link outage without disrupting another, request retry and lease ID tombstones, TTL expiry and borrowed Jan runtime preservation, authority mapping conflicts, grant downgrade, drained host confirmation only after ledger ACK, and new numeric ResourceId/runtime generation after final release.

The `LedgerFixture` is intentionally **test-only**, not the code from coordinator Draft PR #12. `tihulu-ai-core` remains coordinator-owned and unmerged on the Phase 2 target; no dependency or actual cross-crate integration test exists yet. A future CI job must compile/run this standalone crate, link the reviewed core/Registry contracts, and record results.

## Regression coverage (synthetic)

| Requirement | Test coverage | Evidence type |
| --- | --- | --- |
| L-02 | Advisor Quit preserves Agent with shared model on same Pi | Mock Rust test |
| L-03 | Two apps/devices share PC model; first exits, second remains productive | Mock Rust test |
| L-04 | Last model lease unloads and simulated owned VRAM reaches zero after cancellation drain | Mock Rust test |
| L-09 | Mock link loss, missed heartbeat, TTL cleanup while other lease remains | Mock Rust test |
| L-10 | Simulated client crash closes its leases; later callback cannot resurrect cancelled job | Mock Rust test, no real OS crash |
| L-12 | AI Tools UI-owned session closes; independently configured host stays accepting | Mock Rust test |
| L-13 | Explicit host-owner drain blocks new work and waits for bounded cancellation | Mock Rust test |
| L-15 | Separate models in same host: unload only model without consumers | Mock Rust test |
| Authorization | Wrong pins, host ID, missing TLS, public route, cross-client cancellation | Mock Rust test; **no real encryption proven** |
| Integrity | Host-vs-Pi asset distinction, consent staging gate, reject path-like IDs | Mock Rust test |
| Quotas | Per-app concurrent jobs, payload cap, task grants, ID idempotence | Mock Rust test |
| External preservation | BorrowedExternal stays unowned/unloaded by TIHULU | Mock Rust test |

## Outstanding gates (no claims of passing)

- Rust unit tests must actually be compiled and executed on supported Linux x86_64/aarch64, macOS Apple Silicon and Windows x64/ARM64 runners; run `fmt` and `clippy` on the standalone manifest.
- Integration tests with a **real authenticated** rustls HTTP stack, certificate pin/revocation/rotation and short-lived pairing; verify no listener before explicit enable and network firewall behavior across direct Ethernet, LAN and VPN.
- Actual model source verification, staged-copy consent/hash/license/format and file protection; run zero-trust/fuzz/property tests against untrusted wire data.
- Real backend cancellation/termination and GPU VRAM attribution under NVIDIA RTX; ensure no leaked AI Tools-owned contexts when final lease ends. Test Pi 5 with and without Hailo AI HAT+ 2, while respecting platform/runtime differences.
- Verify shared host, Jan/Ollama, readsb/tar1090 and unrelated third-party processes remain unaffected. Test GUI Quit vs separately configured independent host and intentional Quit All behavior.
- Measure actual timing, memory and recovery under power loss, cable pull, VPN outage, process crashes, replay, response reordering, backpressure, abuse/rate limiting and concurrent simultaneous clients.
- Coordinator must enroll the agent crate into root workspace only after review. No other TIHULU repositories or shared root files may be modified by Agent 04.

**Status language:** typed protocol = implemented; deterministic regressions = authored, not yet execution-verified; live encrypted transport/GPU hardware = proposed and untested.
