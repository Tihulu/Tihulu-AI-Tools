# Agent 05 — Phase 3 Jan MCP test repair and evidence

**Status:** Source repair committed for review, **Rust tests not executed in this authoring environment**. This work stays within `crates/tihulu-ai-integrations/**` and `docs/integrations/**` on the existing `agent/05-integrations` branch and Draft PR #10. Integration reference is coordinator sandbox Draft PR #17, tracking Issue #15.

## Failure reproduced from real upstream CI logs

The coordinator's [multi-OS integration run 43155781257](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43155781257) on its **frozen Agent 05 snapshot** (`a517522630ed47ce26bab791f50f28d8d1105fcd`) showed:
- `tests/mock_integration.rs:396`: `assert_eq!(report.state, &mut lifecycle, "stopped")` compares `SessionState` to `&mut LifecycleSupervisor` (E0308/E0277).
- `tests/mock_integration.rs:414`: `Some(&mut lifecycle, "active")` passes two arguments to `Option::Some` (E0061/E0308/E0277).
- `cargo fmt --all -- --check` found multiple formatting differences in Agent 05 source and tests.
- Separately, `tihulu-ai-transport` failed with E0505 and unused-doc-comment Clippy errors; **these belong to Agent 04**, not to Agent 05. No cross-agent files were touched.

## Repair

The formerly malformed assertions now use the actual lifecycle enum:
```rust
assert_eq!(report.state, SessionState::Stopping);
assert_eq!(lifecycle.state("fresh-session"), Some(SessionState::Running));
```

The first mock retains an unresolved owned remote lease, so shutdown must remain `Stopping` until the mocked host TTL signals release. After release and retry, the report becomes `Stopped`; the separately opened fresh session is `Running`.

Applied all **54 Agent 05 rustfmt hunks** published by the coordinator's Linux x86-64 CI log across six owned Rust source/test files, except that the one hunk preserving the erroneous `Some(&mut lifecycle, "active")` expression was replaced with the typed correction above. A static comparison confirmed **53/53 unchanged formatter outputs** remain in the repaired tree. This is **source-level matching, not an executed rustfmt pass on the repaired commit**.

## Existing regression coverage retained (21 mocked tests, none run here)

- `mock_integration.rs` (10 tests): registered/available app-only MCP tool exposure, per-client permissions, one-shot exact-action grant, declined/revoked actions, Jan Desktop setup preview, owner-specific SDR Hard Stop, borrowed Jan/Ollama/readsb/tar1090 preservation (L-05/06/07), owner-only bounded force-close (L-08), unresolved/retried lease and fresh sessions (L-10), and GUI-independent token-free `aipath.txt` references.
- `typed_phase2.rs` (11 tests): redacted read-only Registry model/compatibility query, single-use session-bound approval, cancellation and STOP before dispatch, STOP/Exit keeping other host clients, last host-model lease unload **in a mock host only**, borrowed Jan/Ollama/readsb survival, borrowed host-runtime no-unload, invalid/contradictory Registry/GPU Host refs, duplicate job ID protection, forbidden cross-app/cross-session routes, and no credentials in `aipath.txt`.

All mocks are intentionally inert: no Jan installation, MCP server, real Jan configuration, external host, GPU/NPU/SDR, downloaded models or actual worker process was modified.

## Actual authoring-environment commands and results

Commands attempted on the current repair:

```sh
cargo fmt --manifest-path crates/tihulu-ai-integrations/Cargo.toml --check
# bash: cargo: command not found ; exit 127

cargo test --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets
# bash: cargo: command not found ; exit 127

cargo clippy --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets -- -D warnings
# bash: cargo: command not found ; exit 127
```

Also `cargo`, `rustc`, `rustfmt` and `rustup` are unavailable in the authoring container. **There is no successful cargo test/fmt/clippy run for the repaired Agent 05 head.**

## Coordinator acceptance still required

1. Pull the updated **Agent 05 branch head** into the disposable integration sandbox (#17) via coordinator-owned changes; its existing CI run refers to an older frozen snapshot and cannot verify this repair.
2. Run `cargo fmt --all -- --check`, `cargo test --workspace --all-targets` and `cargo clippy --workspace --all-targets -- -D warnings` on the refreshed sandbox, and independently run the standalone Agent 05 crate against its own manifest.
3. Have Agent 04 resolve its independent Transport E0505 and Clippy warnings; no Agent 05 code should attempt to patch another workstream.
4. Validate real Jan Desktop MCP schema, authenticated app/Registry/GPU Host bridge, process supervision, and owner-attributable GPU/NPU/SDR cleanup only in separately authorized real-system trials. Mock tests and model lease counters are **not hardware acceptance**.

**No merge, release, root workspace edit, shared CI change or live Jan modification was performed.**
