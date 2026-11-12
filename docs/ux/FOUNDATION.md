# Agent 06 — Desktop and responsive UI foundation

**Classification:** Implemented UI demo; real lifecycle enforcement and app adapters **not implemented**. Branch `agent/06-ui`, base `feat/rust-foundation`, assignment Issue #7, reference Draft PR #1.

## Architecture and explicit trust boundary

- `apps/tihulu-ai-web/`: dependency-free ESM responsive UI, CSS, EN/TR/DE strings and pure in-memory state reducer. It includes Overview, Model Market, My Models, TIHULU Apps, Devices, GPU Host, Settings, experimental AI Advisor, App Control Agent, Jan guide and simulated setup wizard.
- `apps/tihulu-ai-desktop/src-tauri/`: standalone Rust 2021 / Tauri 2 native host displaying those assets. Its `Cargo.toml` declares a nested workspace so the coordinator-owned root workspace is not edited. No privileged command is exposed. Tauri capability is scoped to core window functionality; CSP prohibits network connections.
- Future Raspberry Pi 5 web UI: same assets served only by a *future* explicitly started Rust web service with client authentication, TLS or trusted secured tunnel, strict LAN binding and opt-in. **No browser WebUSB requirement**; any native SDR/GPU operation must run through a Rust-owned, permissioned adapter and per-session lease.
- Current demo has **no network calls**, model downloads, background agents, Jan MCP bridge, device discovery, camera access, runtime inference or actual resource telemetry.

## Navigation and prototype states

The seven primary workspaces are Overview / Model Market / My Models / TIHULU Apps / Devices / GPU Host / Settings. Advisor, Agent and Jan Integration are additional directly reachable views. All model candidates, physical profiles, host routes and RAM/VRAM figures are synthetic. English is default; Turkish and German provide translated UI keys. Imported data from model registry/market/host protocol must not be presented as verified until suitable native checks exist.

The app/model and compute selectors are preview-only. Chat timeline and text entry produce a fixed, explicit **no inference connected** response; no message leaves the browser. Permission preview explicitly denies unconstrained shell/FS access. Jan guidance displays Settings → MCP Servers → Add MCP Server but never modifies Jan, launches an MCP bridge or reports success without a real handshake.

## Shutdown contract and required backend work

| User control | Owned resources the real backend must release | Explicit preservation |
| --- | --- | --- |
| **Stop AI** | This UI's Advisor/Agent tasks, client-specific local/remote model leases, temporary audio/chat buffers, AI workers | Other app clients, an independent host with valid other leases, unrelated devices |
| **App Quit** | All resources owned exclusively by this UI, including model RAM/VRAM contexts, child processes, SDR leases, sockets and UI host leases | Authorized independent TIHULU host with active clients; borrowed Jan/Ollama/readsb/tar1090 |
| **Global Hard Stop** | All UI-owned tasks plus owned RF/SDR and workers; escalate only tracked children after bounded timeout | All foreign and independently owned processes; no kill-by-name |
| **Quit All TIHULU-Owned Services** | Explicitly selected TIHULU-owned host services and jobs (after client warning/drain), and UI-owned resources | Every pre-existing external/borrowed service and its resources |

UI mock stages: **reject work → cancel → release → verify → simulated finish**. These are deterministic fixture transitions and **not** actual system operations. Independently enabled host with two mock authorized clients survives App Quit (L-12); Quit All displays explicit impact warning before simulated host release (L-13).

For real implementation, coordinator/transport agents must provide:
1. Typed owner IDs, recorded child process identities, cancel tokens, model reference counts, SDR lease handles and remote client lease IDs.
2. Rust stop API with distinct action scopes and auditable typed resource state; ref-count release on last active model client, bounded graceful timeout and owner-only escalation.
3. Verified closure of actual GPU/CPU/NPU model runtime sessions, child workers, USB handles and socket/stream buffers. Report *owner-attributable* RAM/VRAM memory and unknown/missing measurements, not global GPU memory totals.
4. Native window-close interception and crash recovery (supervised children/lease TTL). Browser tab unload is **not a guaranteed cleanup mechanism**; remote web sessions need server-side lease expiry.
5. Authenticated client revocation, warning/drain for independently owned host services, and preservation of unrelated services, other apps' SDR locks and model files on disk.

## Synthetic regression coverage and evidence

`apps/tihulu-ai-web/tests/state.test.mjs` exercises the pure UI state machine:

- **L-01** App Quit clears UI-owned mock model allocations without affecting external ownership.
- **L-02** Advisor closes while Agent and shared model context remain; last chat closes context.
- **L-03** UI lease releases but independent host and its other clients persist.
- **L-05** Global Hard Stop releases the UI-owned mock SDR lease, never borrowed readsb.
- **L-12** Independent server retains mock clients after App Quit and warning is included.
- **L-13** Quit All warns and releases host fixture, preserving external Jan/readsb.
- Additional idempotent repeated Stop AI, On-Demand/Background OFF defaults, and no-inference chat tests.

These tests **only validate mock state**. Node syntax, Tauri package check, actual close-on-OS, GPU/AI HAT+ 2 compatibility and native RF closure are separate test gates. No hardware validation, pairing test or installer build is claimed by these fixtures.

## Platform and acceptance notes

Target matrix: Linux x86-64 (Debian/Ubuntu/Pop!_OS), Linux aarch64 (Pi 5 8 GB and separately Pi 5 + Hailo-10H), Apple Silicon macOS, Windows x64 and ARM64. On Linux native Tauri requires appropriate WebKitGTK development libraries. ARM64 CI cannot prove the Pi AI HAT+ 2, HailoRT or GPU readiness. Browser form factors include desktop/tablet/mobile with horizontal navigation on narrow screens and keyboard-usable dialogs. Review with native screen readers, real hardware and remote authenticated clients is pending.

## Coordinator integration requests (do not edit shared files here)

1. Root Cargo workspace may add `apps/tihulu-ai-desktop/src-tauri` only after reviewing Tauri dependencies and platform CI; nested `[workspace]` currently keeps Agent 06 independent.
2. Shared Rust supervisor API must be designed with Agents 01–05. Only after it is verified should desktop/browser controls call backend commands or claim physical cleanup.
3. A secure Pi-hosted web transport, native device adapters and actual Jan bridge must precede real connection indicators, interactive downloads or external app actions.
4. No changes to `ui/index.html`, existing TIHULU app repos, main or production releases.

## Phase 2 typed interface

See [PHASE2_TYPED_BOUNDARY.md](PHASE2_TYPED_BOUNDARY.md) for the versioned Rust/JS DTOs, authoritative-owner mapping, service-versus-GUI closure behavior, protocol tests and evidence limitations. Service mode is simulated and defaults DISABLED. The V8 harness can validate committed JavaScript behavior but does **not** replace `node --test`, Tauri builds, external adapters or physical cleanup tests.
