# AI-10 — Synthetic model route + consent policy oracle (NOT real AI)

[AI-10 issue #38](https://github.com/Tihulu/Tihulu-AI-Tools/issues/38) · isolated from frozen [Phase 5 PR #32](https://github.com/Tihulu/Tihulu-AI-Tools/pull/32) and [Phase 6 PR #34](https://github.com/Tihulu/Tihulu-AI-Tools/pull/34) at `283edc8a1101f63e08f12c172d6b2d6688b9a5ca`.

This is an **independent CI-only policy oracle**, not a production runtime. It performs ZERO network calls, model scans, inference, model download/move, configuration changes, process termination, CUDA/Hailo/Metal work, disk writes or live authorization.

## Invariants captured by tests

1. Model discovery/connection does not constitute a user grant; bound approved exact model ID and capability are mandatory.
2. `chat` does not imply tool calls, structured output or an autonomous agent. A hypothetical agent must separately advertise both `agent` and `tool`.
3. CPU fallback for the app-owned model is opt-in, and requires a *reported* ready compatible CPU backend; these are fixture claims, **not actual engine proofs**.
4. GPU choice reports `gpu-requested` only, not actual VRAM use. External Jan/Ollama own their CPU/GPU routing; the fixture must not make a misleading local GPU promise.
5. Jan/Ollama are `borrowed-external`. A request Stop may cancel a caller-owned job, but never terminates the borrowed process or unloads someone else's model. App-owned last-client model release is a **request**, not a physical cleanup receipt.
6. Provider-local endpoint **syntax** is limited to literal loopback `127.0.0.1` and `::1` HTTP with a port and restricted paths; there is no DNS, network call, credential read, authorization handshake, server startup or security validation of a real endpoint.
7. The oracle advertises `start_provider=false` on all routing decisions: Background OFF.

**This independent fixture is not wired into Rust/Tauri production.** The current Phase 5 Rust core/provider tests remain authoritative for their exact SHA and narrower mock claims. Do not count these new tests as actual inference, real Jan/OpenCode integration, model compatibility or GPU validation.

## Sequence to close actual acceptance

- Independent review of typed model/provider identity and grants with Registry/ResourceLedger and [Jan SDK #6](https://github.com/Tihulu/Tihulu-AI-Tools/issues/6). Coordinate agent/MCP capabilities with [AGENTBRIDGE #36](https://github.com/Tihulu/Tihulu-AI-Tools/issues/36).
- Isolated owned CPU backend with synthetic model fixture and real denial tests, then separately authorized actual CPU inference from approved model and verified response/model ID.
- After safe native installer QA and owner permission, real Jan/Ollama loopback detection, authenticated model request, cancellation/timeout, CPU fallback, borrowed process preservation. Never modify Jan/OpenCode settings automatically.
- Dedicated GPU Host/mTLS/CUDA/Metal/Hailo physical acceptance with client isolation, actual VRAM/RAM release receipts, no inference or hardware claims from green mocks.
- Independent security, privacy, provenance, license, cross-OS and rollback QA. **Release HOLD.**

No owner machine, legacy apps, SDR services, installed providers, user models or map files changed. Additive isolated Draft PR only, no merges.
