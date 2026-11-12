# Phase 2 — typed UI ⇄ Rust adapter boundary

**Status:** Experimental, mock-only contract; implementation does **not** connect real Tauri commands, inference, Jan, GPU, NPU, SDR or persistent services.

**Coordination:** [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15), Agent 06 [Issue #7](https://github.com/Tihulu/Tihulu-AI-Tools/issues/7), existing [Draft PR #14](https://github.com/Tihulu/Tihulu-AI-Tools/pull/14).

## Owned source

- `apps/tihulu-ai-web/ui-contract.mjs`: versioned JSON request/response envelope, JSDoc types, strict request-shape checks and an in-memory mocked adapter.
- `apps/tihulu-ai-desktop/src-tauri/src/ui_contract.rs`: matching Rust/Serde DTOs and typed tests. **Not registered as Tauri commands.**
- `apps/tihulu-ai-web/state.mjs`: in-memory fixtures, synthetic ownership, explicit GUI versus independent host lifecycles.
- `apps/tihulu-ai-web/app.mjs`: uses the mocked UI adapter for Overview/Model Market, model compatibility, GPU Host, Jan, chat and stop controls.
- `apps/tihulu-ai-web/tests/`: lifecycle, typed UI contract, localization and responsive source checks.

## Proposed JSON envelope v1

Requests use `{ "schema_version":1, "request_id":"ui-demo-01", "kind":"read_gpu_host", "payload":{} }`. The Rust equivalent is `UiRequest { schema_version, request_id, command: UiCommand }`, with a serde-discriminated `kind` and `payload`. Response uses `{ "schema_version":1, "source":"demo_fixture", "request_id":"ui-demo-01", "kind":"read_gpu_host", "data":{...} }`.

Every response from `createMockUiAdapter` has `source:"demo_fixture"`. The eventual backend may use `backend_verified` **only** with evidence from actual authoritative adapters; UI mock state can never upgrade its own provenance. Model availability `installed` or `external` **does not** imply `verified` compatibility, and a hardware profile selector never proves GPU/NPU readiness.

| Command kind | Payload | Mock behavior | Real integration gate |
| --- | --- | --- | --- |
| `read_overview` | `{}` | GUI phase, synthetic owned resources and totals | Rust ledger/adapter-reported snapshots, never UI memory arithmetic |
| `read_models` | `{}` | Candidate/external model examples | Registry authoritative stable model IDs, source provenance |
| `read_compatibility` | `{model_id, task_id, host_id}` | Always `unverified` | Market and runtime probe with trusted task/host identifiers |
| `read_gpu_host` | `{}` | Separate `gui_phase`, `service_mode`, `service_phase` and other clients | Agent 04 encrypted transport, authorized per-client lease data |
| `read_chat` / `send_chat` / `close_chat` | Typed `agent`, bounded `message` | Adds an explicit no-inference mock message, no tool calls | Signed-in session, model/runtime authorization, tool permission gate |
| `read_jan` / `set_jan_step` | `{}` / `{step:-1..3}` | Wizard-only progress; `verified_handshake:false` | Version-checked Jan MCP bridge, per-tool consent and real handshake |
| `preview_stop` | `{action}` | Owned/retained preview and warnings | Authenticated server-owned effect/clients inventory |
| `execute_stop` | `{action,confirmed:true}` | Changes synthetic state; `outcome:"simulated_only"` | Coordinator's single ledger, backend teardown and acknowledgement |
| `set_service_mode` | `{mode,confirmed:true}` | Changes independent-host demo fixture only | Explicit service installer/host permission, authenticated listener, client drain |

Shutdown actions are exact `stop-ai`, `app-quit`, `hard-stop`, `quit-all`. No generic shell, OS PID, arbitrary path, provider token, device ID or unrestricted RPC endpoint exists in this UI contract. `confirmed:true` prevents accidental demo action; it **does not** grant backend authorization. A real Rust handler must authenticate the calling owner/session, validate scope and permissions, and independently check every resource target.

### Authoritative owner mapping

Coordinator PR #12 proposes `OwnerId(u64)`, `ResourceId(u64)`, `ResourceLedger`, `Provenance`, `CleanupRequest` and adapter acknowledgement. UI `owner_id` and `resource_id` are **display identifiers**, not trusted authority handles. An eventual backend maps a GUI session or TIHULU app task to an authenticated ledger owner; **only the ledger** may decide when the last authorized lease causes a cleanup request. UI mock must never instantiate a second real unload authority, synthesize PID-based kills or assume physical deallocation from a state transition.

Host client lease ownership must remain distinct from independent host process ownership. A UI closure stops only its owned tasks, leases and workers; a deliberately enabled independent TIHULU host stays alive for other authorized clients. The host is **disabled by default** (On-Demand, Background OFF, no keep-warm). `Quit All` is a separate client-warning-and-drain operation requiring explicit approval; external Jan/Ollama/readsb/tar1090 remain untouched. After GUI closure the mock adapter refuses new chat, Jan or service mode mutations.

For real runtime shutdown, the UI must display **request → cancelling → draining → releasing → verifying → stopped or failed** based on backend acknowledgements, and never mark simulated RAM/VRAM allocation as physically freed. A browser tab close cannot guarantee cleanup; native Tauri close hooks and a server-side lease TTL/heartbeat are still needed. Physical GPU/NPU/SDR release tests remain outside Agent 06 scope.

## Validation and limitations

Owned test files:
- `tests/state.test.mjs`: L-01, L-02, L-03, L-05, L-12, L-13; defaults, idempotence, closed GUI.
- `tests/ui-contract.test.mjs`: schema/version/approval rejection, model compatibility unverified, independent host, Jan/chats, closing policy.
- `tests/i18n-responsive.test.mjs`: every EN/TR/DE translation key, the four distinct shutdown controls, independent service controls, modal translations, semantic navigation, CSS breakpoints, focus and overflow.

Reproducible **Node** command (run against a checked-out branch when available):
```sh
cd apps/tihulu-ai-web
npm run check
npm test
```
And standalone **Rust/Tauri** command (requires Rust, Tauri 2 toolchain and per-OS libraries):
```sh
cargo test --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml
cargo check --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml
```

In the current authoring environment, Node 22.16 is installed but a GitHub working checkout is not mountable and Internet DNS access is unavailable; `cargo` / `rustc` are absent. Source fetched directly from GitHub has been exercised by an **in-process V8 test harness** executing the committed JavaScript test bodies, language checks and a stubbed DOM render; this is **not** the `node --test` CLI or a real headless-browser viewport test. Do not represent that evidence as native CI or device acceptance. The coordinator must run actual Node and Rust commands in isolated CI before integration.

## Outstanding blockers

1. No Rust lifecycle supervisor connected: must use one authoritative resource ledger and backend ownership acknowledgements.
2. No Tauri command bridge registered: prevent real effectful control until ownership/policy gate review.
3. No live Jan MCP, GPU Host, registry, compatibility probe, language model or SDR adapter.
4. No authenticated Pi browser service or host-client lease expiry; no WebUSB is needed.
5. Real Node CLI, Tauri/Rust builds and native device/OS accessibility/breakpoint validation remain to be executed by CI/hardware operators.

No changes to root Cargo, coordinator core, root CI, frozen `ui/index.html`, other workstreams, other TIHULU repos, main or releases.
