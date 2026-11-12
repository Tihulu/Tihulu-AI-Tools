# Phase 2 — Independent agent test evidence

**Purpose:** Prove the six agents' **own committed code** compiles and passes its mock/unit tests before integration into the coordinator-owned root Rust workspace. These tests do **not** execute real AI models, pair a GPU, install software, scan user files or touch Jan/SDR hardware.

## CI design

The workflow at [`.github/workflows/phase2-agent-validation.yml`](../../.github/workflows/phase2-agent-validation.yml) is triggered by pushes to the `coord/phase2-agent-ci` branch (or manually). Each matrix/job checks out an existing agent branch on the official repository and prints `git rev-parse HEAD` so that the tested commit is explicit.

| Job | Checked-out agent branch | Test command |
| --- | --- | --- |
| Registry (02) | `agent/02-discovery` | `cargo fmt/test/clippy --manifest-path crates/tihulu-ai-registry/Cargo.toml` |
| Market (03) | `agent/03-market` | `cargo fmt/test/clippy --manifest-path crates/tihulu-ai-market/Cargo.toml` |
| GPU Transport (04) | `agent/04-compute` | `cargo fmt/test/clippy --manifest-path crates/tihulu-ai-transport/Cargo.toml` |
| Jan Integration (05) | `agent/05-integrations` | `cargo fmt/test/clippy --manifest-path crates/tihulu-ai-integrations/Cargo.toml` |
| Platform (01) | `agent/01-platform` | Python syntax, unittest and owned dummy child smoke |
| UI (06) | `agent/06-ui` | `npm --prefix apps/tihulu-ai-web run check` and `npm --prefix apps/tihulu-ai-web test` |

**First pass uses Ubuntu 24.04 x64** to identify source/format/compiler errors quickly without multiplying CI jobs. Cross-platform matrix expansion after passing and after measuring runner limitations. Root workspace's existing 5-platform CI and coordinator PR #12 each passed separately and must not be confused with these standalone tests.

## Constraints

- No PR or source code is merged. No agent branch is modified from this workflow.
- `persist-credentials: false`; the workflow only has read-only repository permission.
- Cargo cache/build output remains in short-lived GitHub-hosted runners. No model weights or credentials are downloaded; dependency resolution for `serde` and other standard Rust crates may use the normal crates.io registry.
- Passing these jobs proves syntactic/functional mock tests, **not** actual GPU allocation release, TLS pairing, Jan MCP setup, Tauri packaging, Hailo runtime or SDR device access.
- Agent branches may advance while this CI workflow runs; compare logged commit IDs to updated PR heads before approving integration.
- On CI failure, report the failing agent job and exact log to the owner. The coordinator must **not** amend an agent's owned source to conceal the problem.

## Integration gate after this workflow

1. Each standalone branch passes fmt, unit tests and Clippy on Ubuntu.
2. Agree on one authoritative loaded-model lease/unload decision (core vs Registry vs host), preserving foreign/borrowed Jan and SDR.
3. Create a separate coordinator-only assembly branch when approved; copy/reconcile reviewed agent changes and update the root Cargo workspace there.
4. Test common types, end-to-end mock flow, five-platform native builds, and future real hardware tasks separately.
