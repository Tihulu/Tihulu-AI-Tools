# Responsive UI foundation (EN/TR/DE)

**Status: synthetic offline-first demo; no live backend.** This is the shared browser UI for desktop and a future authenticated Pi/native Rust web service. The root `ui/index.html` prototype is untouched.

## Preview / test

```sh
cd apps/tihulu-ai-web
npm test
npm run check
python3 -m http.server --bind 127.0.0.1 8000
```

Open `http://127.0.0.1:8000`. The server command is optional, local-only, and must be started manually. With `python3 -m http.server`, there are no API endpoints or authentication; use it **only for local development**. Never expose this demo server to the LAN. Future Pi/phone access requires a separate permissioned and encrypted Rust server.

No npm dependencies or network fetches are required. JavaScript modules, English-default Turkish/German translations, keyboard focus, accessible navigation and small-screen layout are implemented without a WebUSB dependency. All chats, model cards, RAM/VRAM counters, Jan wizard stages and shutdown actions operate **only on in-memory mock fixtures**; refresh resets the simulator. No model files, system settings or third-party processes are read or written.

### Demo interaction safety

1. Open Overview → select Stop AI, App Quit, Global Hard Stop, or Quit All TIHULU-Owned Services.
2. Inspect which mock resources are affected/preserved; Quit All warns about other authorized host clients.
3. Confirm **Simulate only**; observe generated mock lifecycle events and the resource table.
4. Select Reset demo to restore the synthetic fixtures.

Demo interactions do not invoke OS shutdown, release real memory, launch AI inference, pair GPU hosts, download models, connect Jan, or request permissions from an actual client. See `docs/ux/FOUNDATION.md` for the real backend contract and hardware acceptance gaps.

`npm test` runs deterministic L-01, L-02, L-03, L-05, L-12 and L-13 presentation-level regression fixtures. They are not evidence that OS workers or SDR leases release on physical devices.

## Phase 2 typed adapter (still mock-only)

`ui-contract.mjs` provides versioned, validated mock protocol requests for Overview, candidate models, unverified compatibility, independent GPU-host status, chat, Jan wizard and four distinct shutdown actions. All responses have `source:"demo_fixture"`, not real backend evidence. Independent host mode defaults to **disabled** and requires a confirmation modal even in the mock. GUI Quit is separate from a deliberately enabled independent host; after mock GUI closure no new chat, Jan or host-mode mutations are accepted.

Run `npm run check && npm test` in a **local checkout** with Node 22+. Responsive test coverage is CSS/source and stubbed DOM, not measured mobile browser screenshots. See `docs/ux/PHASE2_TYPED_BOUNDARY.md` for exact UI/Rust DTO mapping and future integration prerequisites.
