# Phase 4 — Native Rust module inventory handshake (experimental)

**Scope:** coordinator-only stacked Draft PR, based on [integration PR #17](https://github.com/Tihulu/Tihulu-AI-Tools/pull/17). No application is installed, no data is scanned, and no real inference or cleanup is performed.

The Tauri 2 native binary now depends directly on five actual Rust crates:
`tihulu-ai-core`, `tihulu-ai-registry`, `tihulu-ai-market`, `tihulu-ai-transport`, `tihulu-ai-integrations`.

At application startup the native `NativeBackend` initializes a local **empty in-memory** ModelRegistry and ResourceLedger. The read-only command `ui_native_module_status` reads registry length and an owner-free ledger, and type-checks the Market, GPU Host and Jan contracts. The command accepts **no caller-supplied paths, commands or mutations**. It explicitly reports zero approved source scans and false inference, Jan, GPU, backend/cleanup verification.

The web app only calls this command after a valid `ui_backend_status` Tauri IPC probe; in browsers it never invokes Tauri. The probe validates the exact response schema and displays a narrowly worded **modules linked, not running** signal, while retaining the prominent demo/No Live Connections banner. Invalid data, IPC errors and attempted forged verification fail closed.

Tests include two native Rust invariant tests plus three browser-safe Node protocol tests. The separate Tauri Ubuntu CI **must** compile this standalone crate and exercise its Rust tests/Clippy; root combined CI tests the JavaScript and five portable Rust crates but does not imply Tauri startup works. This bridge does not implement application discovery, model file inventory, runtime-compatible model matching, authenticated pairing or physical GPU/RAM release.

Next acceptance stage: explicit grant-gated existing-source discovery, real provenance-aware snapshots, separate user-authorized runtime/stop adapters. Never equate successfully linked contracts with hardware ready, a model installed or a safe resource cleanup.
