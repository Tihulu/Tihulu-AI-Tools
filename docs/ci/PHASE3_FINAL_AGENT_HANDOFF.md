# Final agent handoff snapshot — coordinator QA gate

This is a **non-production** integration test branch. These are the final agent branches reported as complete by the project owner; snapshot sources are pinned at the exact SHA shown here. The original Draft PRs are not merged.

| Agent | Source commit |
| --- | --- |
| `agent/01-platform` | `350222c632ec8e9ce32a62a1b8295348c51d974a` |
| `agent/02-discovery` | `3386df01999d4969068ef064f4d43f74bd49c8f1` |
| `agent/03-market` | `4a3478c448d13f3298f825dba4411e0afcbc15fd` |
| `agent/04-compute` | `d397b50e39e086e0d631f1b9bd0f75b5a32e84f2` |
| `agent/05-integrations` | `a0ab494c9daad0895bb625bef2a51c5f0f2fb76e` |
| `agent/06-ui` | `5e8b5935ce637f326f10c85b85eff0e4f81fc012` |

Only in this branch, the four standalone Rust crates are enrolled in the root workspace by removing nested workspace markers from their copied Cargo manifests. The source Tauri shell is excluded from the root workspace by existing integration policy.

CI must verify `cargo fmt --all -- --check`, `cargo test --workspace --all-targets`, and `cargo clippy --workspace --all-targets -- -D warnings` on every configured operating system. No mock tests assert physical CUDA/NPU/SDR teardown, real TLS or live Jan MCP verification. No application or hardware deployment authorized.

Proceed to typed Rust interoperability and UI adapter wiring **only after** investigating any remaining build/lint failures; all external/borrowed services and user-owned model files must remain untouched.
