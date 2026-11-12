# Phase 2 — typed Jan MCP / TIHULU app / Registry / GPU Host boundary

**Status:** Proposed integration interfaces, implemented only with offline Rust mock
ports in the owned `tihulu-ai-integrations` crate. No live Jan bridge,
registry/market/host endpoint, SDR or AI adapter has been connected.
Issue #15 is the coordinator contract; Issue #6 and Draft PR #10 remain
Agent 05 ownership. No other TIHULU application repository is touched.

## Authority and call flow

1. **Jan Desktop chat** is an untrusted MCP *client*, not a model/file/host
   owner. The future STDIO/secured-HTTP MCP frontend authenticates its client
   identity and decodes only defined tool arguments. No arbitrary shell, open
   filesystem API, generic HTTP proxy or direct USB/SDR control is exposed.
   An optional Jan Agent plugin remains a separate package/approval.
2. **IntegrationBridge** exposes a tool only if a *real, available, explicitly
   registered* application adapter advertises it and Rust has a client/app/tool
   permission grant. Different apps share one MCP tool name with an allowlist
   of app IDs rather than duplicate names. There is no global auto-approval.
3. **PermissionGate** accepts owner grants from the **trusted local host UI /
   backend only**, not from Jan, the model or MCP arguments. An action requires
   a matching **single-use exact-request approval**, bound to the authenticated client's current app session, on top of its scope grant.
   An approval for another client, task, model, host, job or app cannot be
   replayed; failures do not imply user consent. Revocation drops pending
   approvals.
4. **TypedAppAdapter** is a dependency-injected contract, not a discovered
   real app. It calls `RegistryReader` for app-visible metadata and
   `HostGateway` for independently authenticated app/model/task/host route
   authorization. Its mock supports read-only `find_model_for_task`,
   `list_models`, `get_app_status` and one approval-gated
   `request_media_task`. Other advertised capability names remain disabled
   until actual app adapters exist.
5. **RegistryReader** returns only references visible to this application;
   Registry's `ref-v1-...` identity and source owner are separate from a
   loaded runtime and remote host model. Candidate, missing, incompatible,
   and runtime-verified statuses are different. Read-only discovery returns bounded, redacted model reference IDs,
   owner labels and verification statuses; it never loads weights, moves
   files, scans foreign paths or guarantees inference.
6. **HostGateway** receives an app-scoped request and returns a trusted route
   with a distinct host-owned `HostModelId`; a host may use a different
   model file and registry from a Pi client. This route must be *derived* from
   verified pairing / mTLS peer and independent app credentials, per-app
   grant, task/model/runtime compatibility and quotas. No MCP-supplied
   `HostRoute` or fabricated “verified” flags are trusted.
7. Before host execution, the adapter prepares an inert owner-specific
   cancel/release handle and registers it against the running app session.
   Only then may it dispatch the host request. Stop/Exit/Hard Stop rejects
   new tasks and cancels/releases **this app's** remote lease and owned
   child/SDR handles. It never orders host process termination.

## Proposed shared-type mapping (not a wire format)

| Integration typed API | Neighbor's current type | Mapping and authority |
| --- | --- | --- |
| `AppId`, `ClientId`, `OwnerSessionId` | Transport `AppId`/`ClientId`, coordinator `OwnerId` | Validated opaque caller/app/session identities. Host ingress binds to authenticated client and app credentials, not LLM arguments; coordinator maps trusted session to `OwnerId` |
| `RegistryModelId` | Registry `ModelRecord.id: String` (e.g. `ref-v1-...`) | Index for a source/provenance record. No digest claim, direct path or runtime-load authority |
| `RegistryOwner` | Registry `ModelOwner` | Distinguishes Jan/Ollama/external file owner from the runtime/host that might load it |
| `RegistryVerification` | Registry `ArtifactValidation` and `RuntimeVerification` | Proposed lossy user-facing projection, not a substitute for original evidence; preserve validation source when implementing real port |
| `TaskKind` | Transport `TaskKind`; Market task mapping | Same variants Chat/Embeddings/Transcription/Translation/Vision; centralize canonical enum after coordinator review |
| `HostId`, `HostModelId`, `JobId` | Transport `HostId`, `ModelId`, `RequestId` | Typed conversion after verified binding; host model ID is **not** the Registry reference ID or Pi path. Job/request replay belongs to Host |
| `HostRuntimeOwner` | Transport `RuntimeOwnership` | `HostOwned` can close last lease through host ledger; `BorrowedExternal` never requests Jan/Ollama runtime unload |
| `HostRoute` | Transport host/app grant + paired client + verified peer evidence | Trusted host-generated decision; not serializable MCP input; no session token in `aipath.txt` |
| `LifecycleSupervisor` app session | Core `OwnerId`, `ResourceId`, `Provenance`, `ResourceLedger` | Future authoritative core ledger **must own** resource identity and last-lease decision. Agent 05 owns only the client handle, cancellation propagation and bounded observation |
| `ResourceKind::RemoteModelLease` | Core `ResourceKind::RemoteInferenceLease` and Host `LeaseId` | Adapter closes this client's authorized remote lease. Host/coordinator resolves shared model last lease once, never two unload authorities |

**Next integration gate:** Coordinator should select one canonical owner resource ledger
(PR #12), adapt Registry and Host events to it without duplicate unload
decisions, and replace these bridging prototypes with imported shared types.
The unmodified independent agent crates currently use different Rust
nominal types and namespaces; this table is a **proposal**, not proof
they compile together. The SessionId in the mock is trusted host-side,
not an identity that an MCP caller can change.

## Specific safety rules

- A model found on Jan's disk remains Jan-owned; even if a separate
  TIHULU-owned runtime uses a granted reference, Jan cannot be stopped by
  any TIHULU app Quit. Jan as model-file owner, Jan as inference API
  server and Jan as MCP client are three different roles.
- Pre-existing Ollama, Jan, readsb and tar1090 are `BorrowedExternal`.
  The app's own request streams/leases can be closed, but no broad
  `pkill`, service reset, shared-host kill or RF device lease theft.
- Hard Stop closes tracked, app-owned SDR and AI handles, stops admission
  first, allows a bounded graceful period and reports unclosed resources
  rather than falsely claiming release. Real OS process supervision and
  actual GPU/SDR teardown remain separate validation.
- Independent GUI-close-safe startup uses only stable model ID, paired
  host ID and legacy fallback via the pure proposed `aipath.txt` parser.
  Never put credentials, authentication tokens, URLs, shell strings or
  private API connection values in `aipath.txt`; use approved OS vault /
  pairing registry for secrets and host-specific addressing.
- A future Jan setup wizard may use verified, supported Jan Desktop CLI
  or guide manual configuration with owner approval, but Phase 2 does
  not call Jan or edit Jan configurations. Keep Jan Agent plugin distinct.
- Before production, remove caller access to trusted grant/approval APIs
  at the MCP ingress and verify unforgeable client/session identity.
  A Rust struct is a useful boundary but does not itself authenticate a
  request, implement encryption, or defend against a malicious adapter.

## Remaining blockers

Production requires versioned JSON-RPC/MCP tool schema, authenticated
transport, signed/verified app adapter registration, registry persistence
and evidence projection, host pairing/lease integration, process/SDR handle
supervision, robust async cancellation/timeout and crash recovery, OS
secret vault, actual Jan compatibility matrix and real hardware evidence.
No RAM/VRAM release claims are made from this mock.
