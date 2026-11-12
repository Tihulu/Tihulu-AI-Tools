# Phase 3 — Minimum backend adapter surface for integrated MVP

**Status:** Proposed interface / mock verified only; does not activate runtime integration.  
**Agent scope:** `agent/06-ui`, existing Draft PR #14 / Issue #7.  
**References:** coordinator [PR #17](https://github.com/Tihulu/Tihulu-AI-Tools/pull/17) (isolated integration snapshot) and [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15). PR #17 is an experimental snapshot, **not** permission to modify the shared Rust core/other workstreams or claim the UI backend exists.

## Contract and evidence

Shared UI DTO version is `schema_version: 1`, exactly the currently published JS `ui-contract.mjs` and Rust `src-tauri/src/ui_contract.rs` names. A native transport implementation is **not yet registered**. A future Tauri command can accept a versioned `UiRequest` and return `UiReply<T>`; an optionally enabled Raspberry Pi web host must use a separate authenticated, encrypted Rust endpoint, not a publicly exposed anonymous demo server. This is a proposed single dispatch surface, **not an implemented API URL or Tauri command**.

The envelope is `{ "schema_version": 1, "request_id": "ui-session:1", "kind": "read_overview", "payload": {} }`, with a reply `{ "schema_version": 1, "source": "backend_verified", "request_id": "ui-session:1", "kind": "read_overview", "data": { ... } }`. Until backend proof is attached, source **must** remain `demo_fixture` and all UI values visibly simulated. An adapter error is `unavailable/unknown`, **never** converted to a fake successful connection. The frontend never chooses `backend_verified` by editing a local state field.

The first integrated MVP needs the following **minimal operation contract**, not generic command execution:

| Request kind | Ownership-aware input | Required response / caller behavior | Backend dependency |
| --- | --- | --- | --- |
| `read_overview` | Authenticated GUI session | `OverviewSnapshot`: GUI phase, policy, resource snapshots, owner/provenance and measured-or-unknown RAM/VRAM; never whole-device VRAM as the app's | Coordinator's authoritative `ResourceLedger` + native supervisor |
| `read_models` | Consent to authorized inventory sources | `ModelsSnapshot`: opaque registry reference, source owner, availability, format hint; **installed ≠ runnable**; never expose private source paths | Registry Agent 02 read-only model reference |
| `read_compatibility` | `model_id, task_id, host_id` (opaque bounded identifiers) | `CompatibilitySnapshot`: `unverified` by default, or `verified`/**incompatible** only with trusted runtime/host probe and evidence | Market Agent 03 task/hardware/runtime matching |
| `read_gpu_host` | Authenticated host/client scope | `HostSnapshot`: separate `gui_phase`, `service_phase`, `service_mode`, authorized client count, connection, evidence; no unverified pairing claim | Compute Agent 04 attested transport and lease queries |
| `read_chat`, `send_chat`, `close_chat` | Advisor/Agent session; text within 500 UTF-8 bytes and approved task/model/tool scope | `ChatSnapshot` + explicit agent-owned cancel acknowledgments; tools read-only by default; no arbitrary shell/filesystem | Inference adapter + Agent 05 permissioned tools + ledger per-session lease |
| `read_jan` | Explicit consent for Jan metadata check | `JanSnapshot` with `registered`, `verified_handshake`, `connection` and required approval scope. **Connected** only after real authenticated MCP handshake/allowed tool listing | Agent 05 Jan MCP connector |
| `set_jan_step` | Local UI wizard step `-1..3` | Local wizard navigation only. Do not register/start/change Jan or claim handshake from step progress | **No backend endpoint required** for UI-only wizard |
| `preview_stop` | Requested `stop-ai`, `app-quit`, `hard-stop`, `quit-all` and calling session | Authoritative server plan of affected/preserved **owned** resources and authorized clients, warnings, stable short-lived plan ID; no effect on preview | Coordinator ledger + app/host owners |
| `execute_stop` | Explicit approval + server-bound short-lived plan, session authentication, idempotent request ID | Structured cleanup phases/results with back-end acknowledgement; never assert released from UI mock flags | Coordinator supervisor + agents' adapters |
| `set_service_mode` | Explicit owner approval, signed host identity, authorized client impact | Separate independent host lifecycle with opt-in persisted service policy; STOP must not accidentally terminate unrelated clients | Compute host service control; platform daemon safety |

`set_jan_step` is intentionally local, and there is **no file-write or generic Jan configuration command**. No WebUSB is needed for native hardware. No model download/install/move or real inference endpoint is part of this MVP interface until independently approved.

## Stable identity translation at the Rust boundary

- Registry Agent 02 uses `ModelRecord.id` / typed `ModelId` with potential private path information. The UI receives only an **opaque non-sensitive alias minted by the backend**; do not serialize raw `ModelId::Artifact` source/canonical paths.
- Compute Agent 04 defines `HostId`, `ClientId`, `AppId`, `AppSessionId`, `TaskId`, `ModelId`, `RuntimeId`, `LeaseId`, `RequestId`. Convert only authorized opaque aliases to UI string IDs; never conflate registered model identity with runtime/model-context ownership.
- Coordinator PR #12's `OwnerId`, `ResourceId`, `ResourceLedger`, `CleanupRequest` and `Provenance` remain **one authoritative owner/last-lease unload decision**. The browser has read-only display IDs; it must not independently execute cleanup or guess ownership from a process name.
- Host lease cleanup releases this GUI's client lease, never the independent server/other authorized clients. `quit-all` has a distinct, explicitly approved host-aware impact plan, client drain and external-service preservation.

Opaque request IDs and stable aliases should use bounded ASCII `[A-Za-z0-9][A-Za-z0-9._:-]*` (max 128 UTF-8 bytes; no `..`, no raw paths) for `model_id`, `task_id`, `host_id`. Rust and JS DTOs now validate matching limits. The request envelope `request_id` remains a distinct client idempotency token (1–64 allowed characters).

## Shutdown state reporting (required before any live control)

The server must bind a preview plan to an authenticated GUI or app-session owner, resource IDs, client leases, and a limited expiry; a browser's `confirmed: true` flag is **not** backend authorization. Stop phases: `stop_requested → cancelling → draining → releasing → verifying → stopped | cleanup_failed`. Physical/boundary cleanup is credited only after the responsible native adapter's acknowledgment; unknown RAM/VRAM goes to `null`/unverified rather than fabricated zero. Idempotent repeated STOP does not duplicate unload or kill borrowed Jan/Ollama/readsb/tar1090.

- **Stop AI**: this UI's Advisor/Agent tasks, streams and owned model references. Preserve its unrelated SDR and independent host.
- **App Quit**: all this GUI instance's owned model/worker/SDR/transport resources. Preserve independent service mode and its other authorized clients.
- **Global Hard Stop**: owned resources only with bounded owned-child escalation; never process-name killing.
- **Quit All TIHULU-Owned Services**: separate audited approval, client warnings, drain and native host-owner cleanup. Borrowed external processes remain untouched.

Closing the actual Tauri window is different from simulating Quit inside the browser. Before a live MVP, Tauri must install a native owned-task close supervisor with bounded cancellation/verification; browser tab closure additionally requires authenticated host-side lease TTL/heartbeat cleanup. **Neither exists in the current demo**.

## Required MVP integration handoff / testing

1. Coordinator gives the UI a *read-only, authorized* adapter that can answer `read_overview`, `read_models`, `read_compatibility`, `read_gpu_host` with genuine provenance and proper errors. If unavailable, remain in demo/disconnected mode.
2. Agent 05 exposes a verified Jan connection-status DTO, not a wizard-generated connected flag; chat/tools require separate explicit permissions and a real inference adapter.
3. Coordinator adds a real stop-preview/execute API only after per-owner resource ledger and host lease cleanup ACKs are tested. The default service remains On-Demand / Background OFF / no keep-warm.
4. Native UI tests: `cargo test --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml` + `cargo check --manifest-path apps/tihulu-ai-desktop/src-tauri/Cargo.toml` on OS-specific Tauri prerequisites.
5. Web UI tests (against **real branch checkout**): `npm --prefix apps/tihulu-ai-web run check`, `npm --prefix apps/tihulu-ai-web test`, then actual Chromium/Firefox/mobile/tablet viewport and keyboard/screen-reader review.
6. E2E mock ownership traces on integration sandbox: app/task → registry/market → host or local route → permissioned adapter → owner-specific STOP → last-lease acknowledgment, with two external clients; no physical inference or SDR claims.

**Agent 06 boundary:** only `apps/tihulu-ai-desktop/**`, `apps/tihulu-ai-web/**`, `docs/ux/**`. Root `Cargo.toml`, coordinator core, CI, archived `ui/index.html`, other agent code, releases and hardware settings remain untouched.
