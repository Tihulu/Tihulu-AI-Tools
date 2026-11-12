# AI-08 — Phase 5 independent QA / acceptance gates

**Decision: HOLD (mock source only; no integrated real acceptance).**
All QA changes live in `qa/phase5/`, `docs/qa/`, and this QA-owned workflow.
No agent crate, coordinator production source, common export, existing workflow,
application installation, real provider, model weights or host service is modified.

## Pinned review inputs

- PR #17 `coord/phase2-integration-sandbox` HEAD:
  `df60a2baebc91ee6db0744d0ed3c7e96c16c4000`.
- PR #19 `coord/phase4-native-inventory` HEAD at QA fork:
  `5d69b6cc989746fbae694dd2ef709119dbbf1293`.
- Issue #15: one ResourceLedger authority, borrowed-process preservation,
  per-agent CI evidence, and cross-app integration not yet accepted.
- Issue #18: independently installed apps, distinct executable versus model
  paths, per-app and legacy path support, consent-scoped read-only inventory.
- Prior, not QA: PR #19 Rust Core multi-OS and Linux Tauri native smoke
  succeeded at that SHA (runs 43163727070 and 43163721237).
  Their successful status is not combined Phase 5 acceptance.

## Test matrix / evidence to collect

| ID | Area | Independent checks | Acceptance limitation |
| --- | --- | --- | --- |
| QA-01 | Rust and Tauri build | Root workspace tests; QA crate test/clippy/fmt; standalone Tauri test/clippy/fmt on CI Linux x64, Windows x64, macOS ARM64 | No GUI launch, installers or native hardware |
| QA-02 | Discovery permission | QA synthetic denied grants and wrong-root consent tests | Does not certify any real app permissions |
| QA-03 | Unapproved filesystem access | Jan scan limited to approved model subtree; synthetic external symlink escape on Unix | No syscall/OS sandbox audit or macOS security-scoped bookmark test |
| QA-04 | No automatic model moves/deletions | Assert no destination created, bytes preserved, no unapproved plan | Actual app adapter copy/move/rollback deliberately unimplemented |
| QA-05 | Jan/Ollama ownership | Synthetic borrowed Jan/Ollama/readsb/tar1090 callback counters remain zero on Hard Stop | No real process PID/OS supervision acceptance |
| QA-06 | GPU host isolation | Mock pairing denial, cross-client lease denial, one-client quit preservation, last owned mock VRAM cleanup | No real mTLS, CUDA, measured VRAM or coordinator host ledger deployment |
| QA-07 | Standalone app model roots | Six stable app IDs produce independent suggestions; install path left unchanged | No signed installed-app locator or persistent per-app manifest |
| QA-08 | Mock vs real evidence | Browser absence, forged native statuses, no false cleanup status; extra regression for records without approved scan | A response marked 'linked' still cannot mean installed/runnable |
| QA-09 | UI/native contract | Strict typed UI request denials, read-only Tauri probe and demo evidence checks | No live UI-to-Rust model discovery/stop/inference contract |
| QA-10 | Platform CI | QA-owned workflow on Ubuntu 24.04 x64, Windows 2025 x64, macOS 15 ARM64 | Linux ARM64, Windows ARM64, Pi/Hailo, installed GUI and signing are separate gates |

## Reproduce (all commands are non-installing and model/host-safe)

```sh
cargo fmt --manifest-path qa/phase5/Cargo.toml --all -- --check
cargo test --manifest-path qa/phase5/Cargo.toml --all-targets
cargo clippy --manifest-path qa/phase5/Cargo.toml --all-targets -- -D warnings
cargo test --workspace --all-targets
node --test qa/phase5/js/native-safety.test.mjs
npm --prefix apps/tihulu-ai-web test
cargo fmt --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml --all -- --check
cargo test --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml --all-targets
cargo clippy --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
```

Use `.github/workflows/phase5-independent-qa.yml` for actual OS runner evidence.
Tauri on Linux requires native WebKitGTK/system libraries provided in that QA
workflow. Workflow success must be verified against **the exact QA head SHA**.
If the QA test on nonzero `registry_records` with zero approvals fails, record
as a native handshake consistency gap; do not edit production or weaken the
test within this PR. No test outcome is claimed before its run actually finishes.

## Re-evaluate new Draft PRs without false combined green

For each of AI-01 Platform, AI-02 Registry, AI-03 Market, AI-04 GPU Host,
AI-05 Jan and AI-06 UI, plus AI-07 App Paths: capture the source Draft PR
number, exact head SHA, diff and CI jobs; compare its ancestry with pinned
PR #19. Review only owner-scoped files and tests. Rerun this QA suite against
an **explicit newly assembled temporary integration ref** after coordinator
approval; never infer that individually green branches compose successfully.
No cherry-pick, merge, deploy, install, app scan or migration is authorized by
this report. This QA branch cannot automatically verify source changes that
are not on its ancestry.

## Blocking acceptance work

1. Signed/trusted installed-app adapters with bounded native realpath and
   symlink checks; explicit on-disk consent and external SSD handling.
2. Real model inventory/runtime verification, not linked module types.
3. Native UI request/response integration with provenance, explicit grants,
   owner-scoped Stop, cleanup failure reporting and accurate mock/real labels.
4. Coordinator ResourceLedger must own every live host model/resource lease;
   no duplicate unload authority, no borrowed-provider termination.
5. Per-client authenticated GPU transport and real backend release receipts
   (RAM/VRAM) in an approved hardware test environment.
6. Jan/Ollama real authorization and independent-process preservation.
7. Windows/macOS install/startup/uninstall, Linux desktop packaging, Windows
   ARM64/Linux ARM64, Pi and app standalone tests after source and CI acceptance.
8. Proof of an exact integrated SHA after Phase 5 agents are assembled.

**No GO, merge, release, real installation, model migration or hardware approval.**


## Independent Phase 5 Draft reviews and actual three-OS QA run

Reviewed current Phase 5 agent Drafts, each **independently** descended
from PR #19 and **not** combined with the others:

| Draft | Reviewed head | Independent CI at review | Scope |
| --- | --- | --- | --- |
| #20 Registry discovery | `7c3030d09c30a841ed42813b6427d7e7d7f7b589` | Rust Core multi-OS success (run 43167579541) | Six Registry-owned files, bounded synthetic consent scope |
| #22 Market compatibility | `d015a806966d97e340f807b6b5ef418b17d0a492` | Rust Core multi-OS success (run 43167409284) | Eight Market-owned files, read-only Registry bridge |
| #23 GPU Host | `b5223371259235285bb5c88d18905f765a9c0e5c` | Rust Core multi-OS success (run 43167546657) | Eight Transport-owned files, synthetic ledger receipts |
| #24 Jan/Ollama | `d7a0120a49affc130f951325a19aaad8c0c14962` | Rust Core run 43167628325 in progress at review | Four Integrations-owned files, opt-in provider mock |

**BLOCKER QA-CROSS-01 — source-contract coexistence.** PR #20 adds a new
`ModelOwner::TihuluApp(String)` variant to Registry. PR #22's
`crates/tihulu-ai-market/src/inventory_bridge.rs` has an exhaustive
`match owner: &ModelOwner` without a `TihuluApp` arm. The combined source
would produce a Rust non-exhaustive pattern error. Market's typed
`RegistryOwner` mapping must also account for this independent-app owner.
This is **source review evidence**, not a claim of an executed combined build.
Neither individual green CI run can authorize a merge.

**Executed QA workflow at head
`51b5d436fcdeff9b6ad85d4c082883a00f0e4ba5`:**
[Actions run 43167482748](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43167482748).

- QA Rust `fmt`, `test --all-targets`, `clippy -D warnings`;
  root `cargo test --workspace --all-targets`, and existing UI regression
  tests **passed on ubuntu-24.04, windows-2025 and macos-15**.
- QA JS negative tests on each OS: **5 pass, 1 expected regression failure**,
  `qa/phase5/js/native-safety.test.mjs:49`. The native status validator
  returns `native_contracts_linked` for a forged `registry_records=1`
  despite `approved_source_scans=0`; expect
  `invalid_native_modules`. The CI aggregates this as a failure,
  and the test was not suppressed or weakened.
- Native Tauri `cargo fmt/test/clippy` **PASS** on Ubuntu 24.04 x64 and
  macOS 15 ARM64 (source/native tests only, no installed startup).
- **BLOCKER QA-WIN-01:** Windows 2025 native Tauri `cargo test`
  **FAIL** in `tauri-build` because
  `apps/tihulu-ai-desktop/src-tauri/icons/icon.ico` does not exist
  (Windows resource generation); subsequent native Clippy skipped.
  QA did not alter production icons or packaging.
- No real model scans/provider connections, filesystem migrations, CUDA,
  measured VRAM, Tauri GUI launch, signed installers or hardware acceptance.

**Acceptance remains HOLD**: coordinator/owner must address both real
CI failures and independent branch contract mismatch; then assemble an exact
combined SHA and rerun this suite. Tests/CI must remain blocking.
