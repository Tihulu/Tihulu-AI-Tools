# Phase 3 — Agent 06 evidence and blockers

**Branch:** `agent/06-ui` (continue [Draft PR #14](https://github.com/Tihulu/Tihulu-AI-Tools/pull/14)). **Reference sandbox:** [PR #17](https://github.com/Tihulu/Tihulu-AI-Tools/pull/17) — snapshot only; its green shared Rust checks do not imply Tauri/UI tests.

## Reproducible commands and actual outcomes (2026-11-12)

Environment: local isolated container, **not** a cloned GitHub working tree. `node --version` = `v22.16.0`; `npm --version` = `10.9.2`; `git --version` = `2.47.3`. `cargo` and `rustc` are unavailable. GitHub connector allows reading/writing authorized repo content, but the container's `git ls-remote https://github.com/Tihulu/Tihulu-AI-Tools.git refs/heads/agent/06-ui` fails: `Could not resolve host: github.com`.

Requested CLI attempts from `/tmp/agent06_checkout`:

| Command | Actual result | Meaning |
| --- | --- | --- |
| `npm --prefix apps/tihulu-ai-web run check` | **Exit 254**; `ENOENT ... apps/tihulu-ai-web/package.json` | No repository checkout was available; source check did **not run** |
| `npm --prefix apps/tihulu-ai-web test` | **Exit 254**; same `ENOENT` | Node test runner **did not run** |
| `command -v cargo` | No executable found | Rust/Tauri compilation/tests **not run** |
| `command -v rustc` | No executable found | No Rust compiler in the container |

No attempt has run an installed application, model, SDR, Jan instance, GPU host or hardware.

## Additional checks actually executed

A V8 in-process harness retrieved current `agent/06-ui` files through the GitHub connector, evaluated the committed pure JavaScript state/contract/test bodies and exercised a stubbed DOM render. **27/27 tests/checks passed**, 0 failed, plus **10 JS/JSON syntax parses passed**. Coverage: simulated lifecycle L-01/L-02/L-03/L-05/L-12/L-13; Stop AI vs App Quit vs Hard Stop vs Quit All; independent host preservation and explicit service mode; request schema rejection, unauthorized owner fields, path-shaped model IDs and malformed/oversized chat payloads; mock-only model/Jan/host/Advisor/Agent responses; EN/TR/DE key completeness; source-level responsive breakpoints/keyboard indicators; rendered mock screen navigation, My Models and Market model IDs and localized quit modal.

**This is not `npm test`, headless-browser responsive testing, Tauri/Rust CI or physical resource lifecycle evidence.** One existing asynchronous ESM import test was approximated by an equivalent stubbed-DOM render, not executed by Node's built-in test runner. The Rust DTOs and Serde serialization tests are uncompiled proposals until real Cargo CI executes.

## Coordinator/CI verification request

From a genuine checkout of `agent/06-ui`:

```sh
npm --prefix apps/tihulu-ai-web run check
npm --prefix apps/tihulu-ai-web test
cargo test --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml
cargo check --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml
```

Tauri compilation requires target-native system libraries and Rust toolchain; CI results cannot prove actual GPU/Hailo/SDR cleanup. Browser tests must use real desktop/tablet/mobile viewport sizes and accessibility tooling. Validate the Rust `UiRequest` flattened Serde enum round-trip and request payload bounds before a native dispatcher is registered. The root `Cargo.toml`, shared CI and coordinator core were intentionally untouched by Agent 06.

## Remaining MVP blockers

1. Coordinator's single authoritative `ResourceLedger` must emit authorized ownership/resource snapshots and completed cleanup ACKs (no browser-side cleanup authority).
2. Registry identity projection must hide path-bearing `ModelId`; Market must separately prove `verified` runtime readiness. GPU host status/leases require attested authenticated transport and other-client preservation.
3. Jan bridge only displays connected after a real MCP handshake and explicit permission. Chat inference/tools remain unavailable until separately approved.
4. Native Tauri window closure / crash supervision and Pi web-session TTL cleanup need backend implementation; the present mock `released` states do not free RAM/VRAM/SDR.
5. Actual Node and Rust test execution from a checkout, OS packaging, real browser responsive/accessibility and hardware validation remain outstanding.

See [PHASE3_BACKEND_ADAPTER.md](PHASE3_BACKEND_ADAPTER.md) for the minimal API surface. No merge/release or other repository changes.
