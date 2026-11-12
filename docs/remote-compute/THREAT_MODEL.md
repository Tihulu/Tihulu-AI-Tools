# Security and privacy threat model — Remote Compute

**Scope:** typed contract/mock only. No crypto or live listener in this PR; this document is not a penetration test or certification.

| Threat | Required guard | Current evidence |
| --- | --- | --- |
| Rogue LAN device / DNS/mDNS spoofing | Explicit paired host ID and pinned TLS identity; reject untrusted server cert | Mock rejects wrong host/server pin; TLS pending |
| Client impersonation / compromised app | mTLS-bound client identity + server-issued per-app grant | Mock pin/grant checks; cryptographic credential issuance pending |
| Public endpoint exposure | OFF by default, loopback local admin, explicit host service/allowlisted LAN/VPN, firewall and TLS | No network listener exists |
| Request replay or cross-app cancellation | Owner-scoped lease+request IDs, authenticated cancellation, dedup and TTL | Mock regression covers idempotence and denied cross-client action |
| Remote filesystem path injection | Host-controlled opaque model IDs; separate verified staging digest | No raw path accepted by model ID parser or wire API |
| Unapproved Pi model transfer | User approval, license/runtime checks, hash, secure staging, retention limits | Staging state gate mock only; actual transfer missing |
| Denial of service/VRAM exhaustion | Per-app lease/job/payload/model budget, bounded admission/queue, request deadlines | Mock quotas and bounded history; global physical budget pending |
| Leaked prompts/voice/transcripts | Scoped tasks, explicit network destination, minimal encrypted transfer, redacted audits, short retention | Only byte count and metadata simulated; no payload transfer |
| Race after Quit/disconnect | Cancel owned jobs, reject new jobs, heartbeat TTL, bounded join/force only owned workers | Logical mock state tests; physical worker confirmation missing |
| Unauthorized host shutdown | Host owner gate; client Quit affects only its leases | Admin-only method on host object; no client command for host kill |
| Accidental Jan/Ollama/readsb termination | Track runtime ownership and explicit worker handles; never kill by process name | BorrowedExternal mock preserves external model |
| Secrets leaked through apps | OS keyring/secure store, never tokens in aipath.txt, filenames or audit | No secrets persisted by this crate |
| Remote text prompts trigger app actions | Separate locally verified app-tool permission broker | No shell, filesystem command, MCP or tool execution implemented |

### Pairing, certificate rotation and revocation

Future implementation must generate a cryptographically random short-lived, single-use pairing invitation, show a host certificate fingerprint directly on the host UI and require the operator to verify it out of band. The host must attest a specific client certificate/device and grant separately per app/model/task. Rotation requires confirmation bound to the old trusted identity or secure re-pairing; revoked grants close their affected leases. Anti-replay and audit persistence require a protected store, clock skew handling and denial-of-service controls. IP address, VPN membership, mDNS advertisement and mere possession of a host ID are insufficient identity.

### Connectivity and privacy

Direct PC↔Pi Ethernet and LAN still require authenticated TLS. An approved VPN is an *additional network boundary*, not a replacement for application authentication. Remote mode sends selected data/prompts to the named PC; UI should show destination, model placement, inputs leaving the Pi and active per-app grants. No hidden cloud fallback, unprompted microphone/file capture, or filesystem synchronization. Mobile browser acts as client/control UI only; no automatic phone GPU server.

### Resource ownership / cleanup

Physical GPU allocations are owned by verified runtime contexts, not by inferred process names. Host-owned resources may be unloaded only after the last authorized lease is gone and pending tasks are confirmed drained; borrowed models/providers and separately running host clients survive. Mock counters express expected owner-attributable VRAM deltas, **not actual NVML/CUDA/OS readings**. Hardware proof must include before/after backend handles, process ownership, lease counters and selected allocation metrics on RTX Pi/Hailo and relevant OSes.
