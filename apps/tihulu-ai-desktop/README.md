# TIHULU AI Tools — Tauri 2 desktop shell

**Status: experimental UI-only foundation.** No native model manager, AI inference, Jan MCP, GPU host, SDR access or cross-platform installer has been implemented.

The Rust/Tauri 2 shell hosts the shared static responsive UI from `../tihulu-ai-web`. It intentionally defines **zero application commands**, privileged Tauri plugins, hardware adapters, background workers and network listeners. Desktop hardware access will eventually use explicit Rust-owned adapters and ownership-aware leases; **WebUSB is not required**.

## Development

From `apps/tihulu-ai-desktop/src-tauri`:

```sh
cargo check
cargo test
```

These commands require the Rust toolchain and Tauri 2 OS-specific development dependencies (e.g. WebKitGTK and related packages on Linux). To launch the shell during development, use a Tauri 2 compatible CLI pointing at this crate/config and the adjacent web assets. `bundle.active=false` intentionally prevents this preliminary shell from presenting a distributable installer. On CI, these commands only verify build syntax and platform library prerequisites, not GPU/Hailo/RF hardware.

## Native boundary before production

- A Rust lifecycle supervisor (coordinator-owned core) must expose typed resource owner/lease handles and idempotent Stop/Exit/Hard Stop commands, with bounded cancellation, owner-only escalation and a measurable final verification report.
- A Tauri close-window hook will need to await bounded cleanup and display an incomplete/failed result if resources remain. Abrupt crashes need independent owned-child supervision and remote-lease TTL; JavaScript `beforeunload` is **not** resource cleanup.
- Independent host services may survive UI close only if explicitly enabled for other authorized clients; Quit All must warn them and ask before stopping TIHULU-owned services.
- External Jan/Ollama/readsb/tar1090 and all other borrowed services are never killed or unloaded by our UI. Model files are never deleted by Stop/Quit.
- Future privileged Tauri capabilities must be scoped to concrete, reviewed adapters, not general shell/filesystem execution.
- Desktop targets: Linux x86-64/aarch64, macOS Apple Silicon, Windows x64/ARM64; Raspberry Pi browser interface uses the same web assets and an **explicitly started, authenticated native Rust server** only after that backend is implemented.

The current four shutdown controls **simulate state transitions only**. A window close only closes this no-worker UI shell and does not claim to manage any real external services.
