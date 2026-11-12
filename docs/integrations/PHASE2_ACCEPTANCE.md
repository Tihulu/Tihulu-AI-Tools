# Phase 2 — integration acceptance and evidence ledger

**Scope:** Agent 05 only, mock ports and synthetic resources. Tests in
`crates/tihulu-ai-integrations/tests/typed_phase2.rs` and existing
`tests/mock_integration.rs`. EN-default / TR / DE future UI direction.

## Scenarios to execute

| Scenario | Mock acceptance |
| --- | --- |
| P2-01 read-only discovery | Jan client with a verified per-app permission sees only adapter-advertised app tool; Registry reports candidate versus runtime-verified status without using a host or loading a model |
| P2-02 authorization | Action denied with no one-shot approval, wrong client, or reused approval. Approved exact task/model/host/job gets one scoped mock host request |
| P2-03 explicit Stop/Hard Stop | Owner session rejects future work, cancels own pending lease and releases it; repeat Stop has no duplicated unload |
| P2-04 shared host L-03/L-04 | Two distinct app sessions hold a model on an independent mock host; first exit keeps other active, last exit produces exactly one synthetic host-owned unload decision |
| P2-05 borrowed L-06/L-07 | Jan, Ollama, readsb borrowed handles receive zero cancel/force calls; borrowed host runtime client lease closes without external process/model unload |
| P2-06 contradictory/missing model | Missing/incompatible Registry reference or unauthorized host route cannot dispatch a job even with an approved MCP action |
| P2-07 host binding | A host route identifying a different host/task/Registry-reference tuple is refused; host model identity is distinct from Registry reference |
| P2-08 token/path rejection | Identifier constructors deny path traversal and URL-like strings; proposed aipath rejects token/JWT/command and unknown fields |
| P2-09 cancel before dispatch | STOP or cancelled token before invocation rejects action, host stays idle, and no new lease is admitted |
| P2-10 independent lifecycle L-05/L-08/L-10 | Existing mock tests cover owned SDR release, forced close of owned worker only, unresolved remote lease diagnostics and borrowed-service preservation |

## Test commands to run on Rust 1.82+ (no dependencies)

```sh
cargo --version
rustc --version
cargo fmt --manifest-path crates/tihulu-ai-integrations/Cargo.toml --check
cargo test --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets
cargo clippy --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets -- -D warnings
```

This crate is independent of root workspace, using its local `[workspace]`
stanza. Root CI green does not mean the above commands ran. No shared CI
configuration was changed in Phase 2. The coordinator's Issue #15 gate
requires executable results for this standalone manifest and future
cross-platform CI validation, not text-only test files.

**Execution evidence:** Record exact executed commands, exit codes, CI
job URL and environment before labeling a gate passed. Code review/source
fixtures alone are NOT executed tests. If Rust is unavailable, explicitly
report `cargo: command not found` / `rustc: command not found`, preserve
tests unverified and ask the coordinator's standalone-crate test runner
to validate the Draft PR. A retry requires actual output.

**Physical acceptance still open:** Jan Desktop live MCP handshake,
real authenticated GPU host, Pi 5 CPU / Pi 5 Hailo-10H, RTX desktop VRAM,
Windows native worker cleanup, macOS Apple Silicon, Linux SDR locking,
USB release and borrowed system services. Synthetic zero active leases
is not physical memory/VRAM measurement and cannot establish that a real
model was unloaded.
