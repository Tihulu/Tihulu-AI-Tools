# Workstream 05 resource ownership and mock acceptance matrix

## Invariants

- The immutable owner session must be tracked before a worker, SDR handle,
  or remote lease is created. A borrowed external process is a reference,
  NOT an owned child.
- Stop/Exit/Hard Stop atomically switch the selected session to Stopping,
  reject late work/registrations, cancel owned tasks, request resource
  release, and force-close only tracked owned handles after a grace window.
- The callback interface is strictly nonblocking. This mock enforces
  a bounded wait only if adapter callbacks honor that contract; the
  production OS supervisor needs process-group/Job Object identity,
  watchdogs, cancellation, authentication and forced-shutdown validation.
- Every unresolved owned resource is reported rather than falsely
  marking the session Stopped. Repeated cleanup attempts are safe.
- Borrowed Jan, Ollama, readsb and tar1090 receive ZERO stop/force
  callbacks, even during Hard Stop. Shared remote GPU host processes
  are not killed; only this client's authenticated lease is released.
- Owned SDR streams/USB handles must be released on app Stop/Exit/Hard
  Stop; another app's receiver and pre-existing shared hardware remain.
- RAM/VRAM release requires real backend unload/closure evidence;
  neither a Rust mock nor a generic drop implies GPU deallocation.
  Model files and saved radio recordings are preserved.

## Deterministic tests

- L-05: owned SDR handle cancelled and released; unrelated owner intact.
- L-06: borrowed readsb/tar1090 receive no shutdown calls.
- L-07: borrowed Jan/Ollama receive no shutdown calls.
- L-08: unresponsive owned child receives a force-close request after a
  simulated grace timeout, never by process name.
- L-10: orphaned remote lease remains reported as unresolved until a
  simulated expiry; late registrations are rejected; a fresh session
  may start independently.
- Tool exposure: missing/unavailable adapters hide tools; client/app/
  capability grants are required and revoked explicitly.
- Tool action: exact-request, one-use owner approval, wrong request or
  client rejected, no shell/file access API.
- Wizard: guided manual fallback, explicit consent, read-only probe gate,
  rollback only an owned entry and reject unknown foreign entry.
- GUI independence: proposed aipath refs resolve against mock availability
  and legacy fallback; unknown fields including tokens/commands rejected.

## Unverified / required later

No live Jan installation or CLI is probed. No actual MCP JSON-RPC framing,
network transport, model scanning, device, SatDump, decoder, GPU host or
Hailo/RTX/Metal/Windows process group is invoked. There is no real
callback implementation for SDR or process workers. Tests prove mocked
authorization/ownership state only. Further work requires coordinator
reconciliation of core ownership types and a real per-app adapter plus
explicit integration approval. The host also must isolate one MCP client's
resources from other clients, not identify them by app name alone.

Rust validation status must be recorded separately in Issue #6 and PR:
do not interpret root workspace CI success as isolated crate testing.
