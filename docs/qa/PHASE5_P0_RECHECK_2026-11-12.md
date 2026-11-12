# AI-08 — PHASE5-P0-RECHECK
**Date:** 2026-11-12  
**Repository:** `Tihulu/Tihulu-AI-Tools`  
**QA Draft:** #21, `agent/08-phase5-independent-qa` (base #19)
**Decision: COMBINED PHASE 5 = HOLD** — no combined source checkout/commit has been built or tested.

## Exact reviewed inputs

| Artifact | Exact reviewed head SHA | GitHub Actions CI |
| --- | --- | --- |
| #19 Phase 4 base | `5d69b6cc989746fbae694dd2ef709119dbbf1293` | Existing separate Rust + Ubuntu Tauri CI only |
| #20 Registry | `7c3030d09c30a841ed42813b6427d7e7d7f7b589` | [43167579541](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43167579541) — 7/7 success |
| #22 Market | `d015a806966d97e340f807b6b5ef418b17d0a492` | [43167409284](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43167409284) — 7/7 success |
| #23 GPU Host | `b5223371259235285bb5c88d18905f765a9c0e5c` | [43167546657](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43167546657) — 7/7 success |
| #24 Jan/Ollama | `d7a0120a49affc130f951325a19aaad8c0c14962` | [43167628325](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43167628325) — 7/7 success |
| #25 App Paths (corrected) | `10fcdc76ec5ac3ad8fa013971cdac7316890db3f` | [43168301470](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43168301470) — **7/7 success** |
| #26 UI | `3706d73a59dfedc8087a7a0e1c7b32e306eb7f8d` | No exact-head GitHub Actions run observed |
| #27 Platform | `c73b578970c9a05b12a6a1564f6d0963912f773c` | No exact-head GitHub Actions run observed |
| #21 QA before this report | `93b9272fef87e1d70d6a7ae141a719bd5caa4978` | [43168401733](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43168401733) — **failure**, see below |

GitHub's seven `Rust Core — Multi-OS` jobs include Linux x64/ARM64, Windows
x64/ARM64, macOS ARM64, Python synthetic lifecycle, and UI mock tests.
They **do not** compile and exercise an assembled tree containing all Phase 5 Drafts.

## Independent P0 gate decisions

### QA-CROSS-01 — HOLD

- #20 adds `ModelOwner::TihuluApp(String)` in
  `crates/tihulu-ai-registry/src/registry.rs`.
- #22 `crates/tihulu-ai-market/src/inventory_bridge.rs` still has a
  fully enumerated `registry_owner(&ModelOwner)` match **without** the
  new `TihuluApp` arm. Neither reviewed Draft head has advanced to a
  remediated compatible pair. A combined compile is expected to fail
  on an uncovered enum variant.
- Each PR is individually CI green (links above). **No combined
  source compile has been executed.** Market's owner, not QA, must align
  the owner DTO/mapping, followed by an explicit temporary integration
  ref and three-OS compile/test.

### QA-UI-01 — HOLD

- At exact QA head `93b9272fef87e1d70d6a7ae141a719bd5caa4978`,
  `apps/tihulu-ai-web/native-bridge.mjs` still accepts
  `registry_records: 1` when `approved_source_scans: 0` as
  `native_contracts_linked`, whereas the current Phase 4 read-only,
  fresh-registry handshake requires the injected inconsistent response
  to be rejected.
- The preserved regression
  `qa/phase5/js/native-safety.test.mjs`:
  `QA: an unapproved inventory count is not accepted as discovered models`
  fails (5 pass / 1 fail) on Linux, Windows, and macOS.
- [QA run 43168401733](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43168401733)
  executes all existing UI tests before the independent negative tests;
  failing result is **intentionally blocking**, not skipped or waived.
  Future designs allowing legitimately pre-registered records require a
  separately versioned, trusted provenance proof, not merely a raw count.
- UI/native contract owner must remediate in an independent Draft;
  original failure and all negative assertions must stay enabled.

### QA-WIN-01 — HOLD

- On QA head above and Phase 4 base, GitHub Contents returns 404 for
  `apps/tihulu-ai-desktop/src-tauri/icons/icon.ico`.
- Windows 2025 x64 native Tauri build in
  [QA run 43168401733](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43168401733)
  fails in `tauri-build` Windows resource generation:
  `icons/icon.ico not found`. Windows native Clippy not executed after
  the failed compile. Linux Ubuntu 24.04 and macOS 15 native
  `cargo fmt/test/clippy` **pass** in that same run.
- Desktop packaging owner must provide a valid project-owned icon
  without altering safety permissions, and repeat Windows native build
  and Clippy on an exact remediated SHA. Packaging/installation
  acceptance remains separate and unavailable.

### APP-PATH-WIN — GO for #25 *independently*, not combined

- Previous failing #25 SHA:
  `0e033b63be555c08262b11cdfe762c87654b2071`
  had Windows x64 and ARM64 `InvalidInstallRecord` fixture failures.
- Remediation SHA:
  `10fcdc76ec5ac3ad8fa013971cdac7316890db3f`.
  The intervening diff modifies only
  `crates/tihulu-ai-registry/tests/app_adapter_contract.rs`
  to use an OS-aware `native_path` test fixture, while preserving
  actual strict `Path::is_absolute` production checks.
- [Run 43168301470](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43168301470)
  is **complete/success**, with **7/7 successful** jobs, including both
  Windows architectures. Existing negative tests were not disabled.
- Does not prove actual installed-app identity, native filesystem
  access, per-app path migrations, SSD mounting, or coexistence with #20.

### JAN/OLLAMA borrowed-process ownership — GO for synthetic contracts, HOLD for real processes

At #24 `d7a0120a49affc130f951325a19aaad8c0c14962`:

- `crates/tihulu-ai-integrations/src/providers.rs::stop_session`
  removes only that caller's grants and tracked requests and delegates
  owner-scoped shutdown; there is **no provider-process kill API**.
- `tests/phase5_providers.rs::stop_and_quit_cancel_only_owned_requests_and_keep_borrowed_services_alive`
  asserts: cross-client cancellation denied; other app's Ollama
  stream survives first app Stop; borrowed Jan/Ollama/readsb
  sentinel stop counts stay zero; repeated Stop is idempotent.
- `crates/tihulu-ai-integrations/src/lifecycle.rs` requests
  `request_cancel` / `force_close_owned` **only** for
  `Ownership::AppOwned`.
- #23 `tests/phase5_gpu_host.rs::borrowed_external_provider_never_receives_a_cleanup_request`
  proves mock shared Jan leases emit no ResourceLedger cleanup ticket,
  and attempted synthetic unload is rejected.
- Independently, PR #21's Rust regression
  `qa_jan_ollama_and_foreign_services_are_never_killed_on_app_stop`
  verifies zero borrowed close callbacks, and its CI Rust test matrix
  passed on Linux/Windows/macOS.
- #23 and #24 each have separate 7/7-success cross-OS CI; this is a
  **mock-only contract GO**, not evidence that a real Jan/Ollama PID
  cannot be terminated by future unreviewed adapters. Real-process
  acceptance **HOLD**, no real providers were contacted.

### QA negative-test integrity — GO

- `qa/phase5/js/native-safety.test.mjs` blob SHA
  `713bbca80701b8c5ed46d32340f27cc65b82b013` is
  **identical** on earlier CI SHA `51b5d436fcdeff9b6ad85d4c082883a00f0e4ba5`
  and QA `93b9272fef87e1d70d6a7ae141a719bd5caa4978`.
  All six Node negative tests remain. Rust QA tests preserve the
  negative cases; sole Rust test-line change in those two QA revisions
  aligned the RF Geolocation synthetic app ID with AI-06/07.
- `.github/workflows/phase5-independent-qa.yml` blob SHA
  `9dcfbe4cc1a00ac66923b98ed1d9f98f86fb391f`
  is identical across those QA revisions. Failure aggregation remains
  mandatory. No production source or existing workflow modified.

## Latest exact-head CI details (QA #21)

[Run 43168401733 at `93b9272...`](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43168401733):

| Runner | Portable QA Rust, root workspace, existing UI mocks | Native Tauri | QA negative |
| --- | --- | --- | --- |
| Ubuntu 24.04 x64 | PASS | PASS (fmt/test/clippy) | **FAIL — intentionally blocking** |
| Windows 2025 x64 | PASS | **FAIL — icon.ico absent** | **FAIL — intentionally blocking** |
| macOS 15 ARM64 | PASS | PASS (fmt/test/clippy) | **FAIL — intentionally blocking** |

The independent QA negative-step `continue-on-error` permits
**subsequent evidence collection only**; the explicit final
`Enforce QA negative-test result` step fails the job. CI is **RED**.

## Remaining integration blockers

1. Fix #20/#22 `TihuluApp` exhaustive owner mapping in a producer-owned
   Market remediation PR; prove compile against **both** exact heads.
2. Fix native status provenance/schema check without weakening the original
   forged-inventory regression; repeat all three OS QA UI tests.
3. Supply valid Windows Tauri `icon.ico`, preserve restrictive
   permissions/capabilities, and repeat native Windows test + Clippy.
4. Integrate #25 path adapter only with Registry owner-reviewed exported
   interface and scoped consent; fake adapter contracts are not live discovery.
5. Wire #26 UI to native approved discovery/model paths and ownership
   snapshots with explicit mock/real provenance; #26 has no exact-head
   GitHub CI; #27 platform scripts have no exact-head CI.
6. Assemble **one immutable integration SHA** for #20, #22–#27 plus QA #21
   in an isolated coordinator-owned integration worktree/ref.
   No authoritative combined CI currently exists. Check versioned contracts,
   all imports/exports, tests, IPC data and native startup together.
7. After full mock integration passes, real authenticated Jan/Ollama adapters,
   host mTLS, actual owned resource cleanup receipts, application install
   smoke tests, and measured RAM/VRAM evidence remain separate explicitly
   authorized future acceptance gates.

## Recommended SAFE test order

1. **Freeze SHAs** of every agent head; capture provenance and diff
   against base #19. Do not modify the frozen base or main.
2. **Contract compile first**: build Registry #20 + Market #22 in a
   disposable integration worktree; fix typed owner mapping in its own
   PR if it fails. Run `cargo fmt --all -- --check`,
   `cargo test --workspace --all-targets`,
   `cargo clippy --workspace --all-targets -- -D warnings`.
3. **Scoped security mock tests**: combine App Paths #25 and Registry,
   run consent, traversal, symlink escape, borrowed model and migration
   non-effect checks; no host disk scan or actual data migration.
4. **Lifecycle mock tests**: combine GPU Host #23 + Jan/Ollama #24,
   exercise shared/borrowed leases, per-app Stop/Quit, late ACK and
   retry, stolen identities, multiple clients, and independent host.
5. **Native contract tests**: verify real-vs-mock IPC provenance with
   #26 UI; preserve and pass **every** QA-UI-01 negative case before
   declaring UI GO. All tests on synthetic fixtures.
6. **Build/packaging prerequisite check**: add valid Windows icon in
   production-owner PR, then Tauri `cargo test` and Clippy on Linux
   x64, Windows x64, macOS ARM64. Use #27 dry-run platform checks;
   no installer execution or native host startup.
7. **Full single-SHA cross-OS integration**: explicitly assemble all
   reviewed changes plus unmodified QA negatives in isolated CI;
   run full Rust workspace, Tauri, Node, Python and 3-OS matrices
   with fail-on-red. Reconfirm no unapproved operations or mock-to-real
   status promotion. Record exact SHA, job logs, and review sign-offs.
8. **Separate future real-environment gates only after authorization**:
   genuine app installation/startup, provider/host authentication,
   external SSD/permissions, physical cleanup verification, and
   Windows ARM64/Linux ARM64/Pi/Hailo checks. These are **not**
   authorized by the present QA task.

**No merge, release, install, model scan/move/delete, host connection,
hardware action or automatic CI acceptance.** Unverified combined code
must never be marked green based on isolated jobs.
