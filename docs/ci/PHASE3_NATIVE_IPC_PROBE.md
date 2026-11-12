# Phase 3 — Minimal Tauri↔Web read-only IPC contract

**Integration sandbox only. Not an AI inference backend.**

The native Tauri 2 host registers exactly one new read-only command, `ui_backend_status`, returning a fixed `schema_version:1` snapshot:
`source:demo_fixture`, `transport:tauri_ipc`, `ui_bridge_ready:true`, with `backend_verified:false`, `hardware_active:false`, `cleanup_verified:false`.

The web UI uses `app.withGlobalTauri: true` only in the native shell to query `window.__TAURI__.core.invoke` through a **read-only** `probeNativeBridge` function. In an ordinary browser, this command does not exist and the UI stays in web-preview mode. A malformed, forged or failed response is rejected and is **never** treated as successful GPU, Jan, model, SDR or RAM/VRAM cleanup.

All existing model, chat, Shutdown, Quit and independent GPU host controls are still mock-only; this patch registers **no** privileged native command or execution adapter.

Native Tauri build, packaging, OS dependencies and actual integrated runtime are **not** accepted by the root Rust CI, which explicitly excludes this standalone Tauri crate. Node JS mock tests validate the contract but cannot prove a real native WebView handshake.
