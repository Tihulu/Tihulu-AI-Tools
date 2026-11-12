# Phase 2 mock integration sandbox — snapshot inventory

**Status:** Coordinator-owned experimental assembly only; **not merged**, **not released**, **not a working AI/GPU/Jan app**.

Purpose: bring the six isolated agents' reviewed source trees and the coordinator's pure resource ledger onto **one disposable branch** to run multi-OS `cargo fmt --all --check`, `cargo test --workspace --all-targets`, and Clippy on the actual modules together. This is a *snapshot copy*, **not merging** any agent Draft PR, foundation, main or other TIHULU app repo.

| Source | Frozen commit |
| --- | --- |
| `coord/resource-lifecycle-core` | `014c3f396bc467751e582652abf9001ca1bc709e` |
| `agent/01-platform` | `f191a58cb3a93e2ce637c5053c901542a7d7ec2d` |
| `agent/02-discovery` | `d70e7ef1a1abfe0edf4d7969741e6496f4feffd7` |
| `agent/03-market` | `63d4db9bdc6c10ddf806b5d0e0298eea672eb77a` |
| `agent/04-compute` | `b74e87d1652c22744854abfa710f9989bd72e2b2` |
| `agent/05-integrations` | `a517522630ed47ce26bab791f50f28d8d1105fcd` |
| `agent/06-ui` | `86378e4c79afa57a12d410e3977c1e12b80021eb` |

## Actual integration changes

1. Copy each owner's files only under their assigned paths into this coordinator-owned **sandbox branch**.
2. Copy `tihulu-ai-core` resource ownership module from coordinator Draft PR #12.
3. Modify only sandbox manifests: add four Rust crates to root `Cargo.toml`, remove their nested `[workspace]` marker (required for shared Rust workspace membership).
4. Keep the Tauri 2 native shell outside root workspace pending desktop dependency and platform readiness. Keep the browser mock and Python platform scripts as development artifacts; neither is connected to a live Rust API.
5. Run existing five-target `Rust Core — Multi-OS` GitHub Actions on this Draft PR. A green result would only prove the combined source tree compiles and tests, not that modules are wired end-to-end.

## Known constraints

- Registry's new `ResourceLedgerPort` is only an interface; core runtime instance and actual host/registry bridge are not yet implemented.
- GPU Host security is typed/mock evidence, **not real TLS pairing or real inference**.
- Jan MCP is permissioned/mock-only, not connected to actual Jan Desktop.
- Tauri/web UI displays mock data; UI close is **not** proven to release real device/model resources.
- ResourceLedger produces cleanup requests, but no CUDA/Hailo/SDR backend adapter has acknowledged actual release.
- Some agent sources may still fail rustfmt or test/lint; report exact CI errors and give them back to owners, do not mask them.
- Never install or change device/software, model files, Jan configs, remote hosts or other application repositories.

All production merges/releases and hardware acceptance require explicit separate review.
