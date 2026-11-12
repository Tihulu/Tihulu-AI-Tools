# AI-06 PHASE5-UI — native inventory & standalone application workspace

**Status:** Isolated UI components and mock fixtures only, **no native Registry discovery accepted yet**. Designed against coordinator [Draft PR #19](https://github.com/Tihulu/Tihulu-AI-Tools/pull/19), branch `coord/phase4-native-inventory`; report to [Issue #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15). This work may not change `app.mjs`, `native-bridge.mjs`, Tauri commands, root manifests/CI, other agents' files or external TIHULU apps.

## Files and coordinator-only wiring

- `apps/tihulu-ai-web/native-inventory-model.mjs` — strict, cloneable `demo_fixture` model/app/provider/ownership snapshots; rejects forged verification, real filesystem paths and claims of actual scanning or cleanup; pure consent-preview reducer.
- `apps/tihulu-ai-web/native-inventory-i18n.mjs` — standalone English(default)/Turkish/German strings, no edits to existing translations.
- `apps/tihulu-ai-web/native-inventory-view.mjs` — `renderNativeInventoryWorkspace(snapshot, state)` and `mountNativeInventoryWorkspace(root, snapshot?)`; isolated controls and escaped HTML; accepts **mock schema only** and rejects/spoofed real responses.
- `apps/tihulu-ai-web/native-inventory-view.css` — scoped styles, responsive 950/640px breakpoints, table scrolling, focus outlines and reduced-motion rule.
- `apps/tihulu-ai-web/tests/native-inventory-*.test.mjs` — pure state, ownership, i18n, screen, mocked consent and safety tests.

No native IPC calls, browser file picker, network requests, model loading, file writes, releases, background tasks or process operations exist. **These modules are not imported by the current app** (coordinator will wire after review). To preview later without modifying original app, the coordinator can explicitly import the module and CSS into a test host and call `mountNativeInventoryWorkspace(hostElement)`. The controller returns a `dispose()` method to remove listeners. A fake sample UI must not be silently substituted for real discovery.

## User-visible panes

1. **Models** — origin (TIHULU app, Jan, Ollama, HF, user), format (GGML/GGUF/safetensors/HEF/ONNX), separate file owner and runtime verification; all sample model IDs are `demo:...` and paths are `demo://...`, **never real on-disk paths**. `installed` and `verified runnable` remain separate back-end states.
2. **Applications** — six standalone TIHULU app identities, *separate* installation/executable path, active model root, legacy model paths. Existing paths untouched; no app installation status is inferred. Future app-specific configuration changes need their own explicit approval/rollback gate.
3. **Folder consent** — user chooses source category, sees a review of bounded metadata-only access; disabled actual folder picker, disabled grant checkbox, disabled native approval control. The only enabled action **reviews a mock request**, never creates an access token or invokes an OS picker. Every live scope must be a specific selected/canonical path, not entire disks.
4. **Providers** — Jan, Ollama, GPU Host all `not_checked`. No fake connected state. Independent GPU host OFF by default; GUI Quit alone cannot stop it when later legitimately enabled.
5. **Ownership/cleanup** — GUI, independent host and borrowed external service owners shown separately. Lease counts are `null`/unknown, owned cleanup `not_executed`, foreign processes `not_applicable` (never terminated). No simulated RAM/VRAM/SDR release is presented as verified.

The workspace banner must remain prominent whenever source is `demo_fixture`. `native_inventory_status` on PR #19 currently proves only *Rust modules linked and empty in-memory Registry/Ledger*, **not** actual installed model discovery, Jan pairing, GPU connection or cleanup. Its actual command name is `ui_native_module_status`; do **not** reuse this as a model inventory API.

## Exact minimum backend contracts (NOT implemented)

The coordinator should implement separate, policy-reviewed **read-only** native commands or versioned service methods with real, provenance-labelled output. The names below are **proposed**, not existing endpoints:

| Proposed operation | Input authorization | Required result and invariants |
| --- | --- | --- |
| `inventory_list_models` | Authenticated GUI/app context and explicit approved source grants | `{schema_version,source,grant_scope_id,models:[{opaque_model_id,origin,format_hint,file_owner,artifact_validation,runtime_verification,scoped_display_location}]}`. Runtime proof must be from trusted probe; private canonical paths remain backend-only. `installed` does not imply runnable. |
| `inventory_choose_folder` | Native OS picker invoked by deliberate user gesture, scoped owner | `{pending_folder_alias,scope_digest,read_only,hidden_excluded,recursive_bounds,expires_at}` **without scanning**. The backend canonicalizes realpath, rejects symlink escape, root/secret/broad-disk selections and inaccessible folders. No arbitrary path from JS is treated as permission. |
| `inventory_review_grant` and `inventory_approve_grant` | Separate explicit consent after displaying exact folder, purposes, depth, owner, retention | Time-scoped opaque `grant_id` bound to OS user/session and canonical scope, revocable and audited; scan remains denied before approval. Revocation blocks future scans. Never scan all disks by default. |
| `inventory_list_apps` | Trusted, signed app identity/adapter and user-approved discovery | Each record has `app_id`, `installed_state`, `installation_path` (**distinct from** `active_model_root`), `active_model_root`, `legacy_model_roots`, source provenance, app version, verification and read scope. No running apps reconfigured. |
| `inventory_provider_status` | Consent-scoped Jan/Ollama status probe; separately authenticated GPU Host pairing | Each provider returns `not_checked|unavailable|verified_connected`, `proof_type`, `observed_at`, `external_owner`; IPC/model listing alone is not an authenticated inference connection. Never start or change Jan/Ollama. |
| `inventory_resource_status` | Coordinator's authoritative `ResourceLedger` owner mapping; host-scoped client auth | Per-owner `lease_count` or unknown, `cleanup_phase` and `verified_ack` only after backend adapter release. GUI close releases GUI-owned leases but preserves independent host and other authorized clients. Borrowed processes are never killed. |

**No effectful app-path operation** is enabled by this UI. Future `PreviewModelRootChange`, `ApplyApprovedModelRootChange`, `RollbackModelRootChange` must preserve existing app binaries and models, check owner/locks/shared references, preview a safe config-only repoint, back up app settings and verify rollback; copy/move/delete/installer changes remain separately approved and out of scope. See [Issue #18](https://github.com/Tihulu/Tihulu-AI-Tools/issues/18) and `docs/ci/INDEPENDENT_APP_PATHS.md`.

### Native success gate

A future authorized native adapter must carry evidence from real Registry records and permission grants. Its JSON must be independently validated against an accepted **new native-response schema**; today's `validateInventoryFixture` deliberately rejects any `backend_verified`, real OS path, installed-app or verified-cleanup claims. The coordinator should not modify this demo validator to make live results appear. Native errors and absence must render `unavailable`/unknown, never `connected`.

## Validation and remaining blockers

Run from an actual checkout of the new PR branch:

```sh
node --check apps/tihulu-ai-web/native-inventory-model.mjs
node --check apps/tihulu-ai-web/native-inventory-i18n.mjs
node --check apps/tihulu-ai-web/native-inventory-view.mjs
node --test apps/tihulu-ai-web/tests/native-inventory-model.test.mjs apps/tihulu-ai-web/tests/native-inventory-view.test.mjs
npm --prefix apps/tihulu-ai-web test
```

Tests are mocked only: responsive **source CSS** assertions and DOM-controller stubs, not real mobile browser screenshots, real Registry scans, GPU/NPU/SDR cleanup or live Jan/Ollama connections. Coordinator must separately test real installed source permissions, OS app paths and service leases, plus Tauri builds on supported OSes.

**CI gate:** no shared CI configuration was modified by Agent 06. Report actual new Draft PR CI runs and missing checks separately; green Rust core CI on the base is not proof these new UI modules are wired or physically functional.
