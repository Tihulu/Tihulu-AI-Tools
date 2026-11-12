# Workstream 05 — permissioned Jan MCP and independent app SDK

**Implementation status (2026-11-12): mock-only Rust contract, not a working MCP server.**
Owned solely by Agent 05. Refer to repository-level JAN_INTEGRATION.md,
JAN_SETUP_WIZARD.md, AIPATH_CONTRACT.md and RESOURCE_LIFECYCLE.md for product
requirements. All target UI flows default to English and plan Turkish and German.

## Architecture

Jan Desktop chat (MCP client) -> future supported STDIO/secure HTTP transport ->
TIHULU integration bridge -> Rust authorization -> verified app adapter ->
specific scoped capability.

The crate currently implements the **middle authorization/SDK layer only**:
- Fixed capability enum and typed arguments. There is NO run_command,
  arbitrary file read/write, unrestricted process execution, raw SDR tuning,
  or generic HTTP proxy tool. LLM output is never a grant.
- Explicit host registration for adapters; tool listing requires a registered,
  available adapter advertising that exact capability **and** a client/app/tool
  grant. There are no built-in live TIHULU app adapters.
- Read-only examples: app listing, model metadata discovery, task compatibility,
  approved app status, planner status and binding preview, but only when a
  future real adapter implements the operation. A tool name by itself does
  not mean the app supports it.
- State-changing examples: specific binding, specific media task and cancel
  owned job. These require both a scoped grant and an exact-request,
  single-use approval issued by **trusted UI/backend code** after the owner's
  confirmation. Approval cannot be requested through the MCP surface.
- Revocation removes a client's grants and pending action approvals. Current
  state is in-memory and must be persisted later in a protected backend store.

## Trust and integration boundaries

1. Jan Desktop is an MCP client. TIHULU is the app-side authorization boundary.
   Jan's language model, system prompt and tool arguments are untrusted.
2. Jan Agent plugin work is a **separate** optional package, distinct from
   Jan Desktop MCP registration. No plugin or CLI command is installed here.
3. Jan may be the **model-file owner**, a separately active **local API server**,
   or an **MCP client**; those roles do not confer process or file ownership.
4. A paired PC GPU host is another service with its own identities and
   per-client leases. A host reference never contains its secret or local
   model path and does not give a chat administrative access.
5. Future real adapters must validate app protocol/schema versions,
   installation provenance, scope and active job/lease identity. They must
   redact secrets, sensitive paths, transcripts and private raw logs from
   tool results by default. No adapter is inferred from a folder name.
6. A future MCP transport must expose only list_tools/invoke; it MUST NOT
   expose register_adapter, permissions_mut, owner_approved_action or any
   direct OS execution endpoint. In-process trusted host code is a separate
   trust boundary. Per-request approval tokens are never LLM-provided fields.
7. Launch on demand. No background daemon, public listening socket, model
   scan, downloads, Jan configuration write or hardware operation exists here.

## Availability and executable behavior

A bridge call checks the supervisor's actual session state (rather than trusting a caller-supplied status) and is rejected when the session is not Running, the
cancellation token is set, the adapter is missing/offline, the capability is
absent, the exact grant is absent or an action lacks one-use owner approval.
Adapter methods are synchronous test interfaces; production MCP tasks must
be hosted by a cancellable task supervisor with owned worker handles and
bounded shutdown. A blocking malicious adapter is not safely cancellable
by this mock crate alone.

Jan chat MCP tool availability is therefore **zero today**. Real Jan MCP
wire compatibility, capability schema/JSON serialization, STDIO/HTTP
framing, endpoint authentication, readiness and tool-call results still
require separate transport implementation, real app adapters, Jan version
tests and explicit user approval.

## Independent app startup / aipath.txt

The pure proposed startup parser accepts a narrow set of non-secret v1 and
v2-proposal identifiers: app ID, optional manager, legacy fallback,
reference to a managed/external stable model ID, and/or paired host ID.
It does not read aipath.txt, accept arbitrary files or resolve host
addresses; the schema is not yet a committed production parser standard.
No tokens, commands or key material are accepted. Resolution is delegated
to the app's separately verified local registry/runtime and securely paired
host registry, and falls back to legacy behavior if unavailable.
The AI Tools GUI is not a prerequisite. Jan-as-inference-server does require
that separate Jan service to be independently running.

Coordinator-owned follow-up: reconcile this prototype grammar with canonical
model registry/market and GPU-host ID schemas before enabling consumers;
integrate the crate into root workspace only after review (remove nested
workspace marker and add member to root Cargo.toml in a coordinator PR).
Do not change root files as part of Workstream 05.

## Commands and evidence

When Rust 1.82+ is available, run:
  cargo fmt --manifest-path crates/tihulu-ai-integrations/Cargo.toml --check
  cargo test --manifest-path crates/tihulu-ai-integrations/Cargo.toml
  cargo clippy --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets -- -D warnings

The root workspace intentionally does not include this crate yet. Existing
root CI may be green while this isolated crate remains untested. Physical
SDR, hardware resource release, CUDA/Hailo and real Jan connection require
separate acceptance and must not be reported as passed from mocks.


## Phase 2 integration boundary

- **New typed contracts:** `typed.rs` provides `AppId`,
  `RegistryModelId`, `HostId`, `HostModelId`, `JobId`, `TaskKind`,
  read-only Registry records, Host route/lease callbacks, and a mock-only
  app adapter that is registered explicitly with the permissioned bridge.
- Exact one-shot action tickets now bind the **owner session** as well as
  client and entire request. A distinct validated host model ID is
  required; no Registry candidate is ever treated as verified runnable
  merely because it was discovered in Jan.
- The adapter reserves a non-allocating owner-specific close handle before
  mock host dispatch. Actual last-lease unload authority remains with
  the coordinator's single core ledger / authorized GPU Host, never the
  MCP bridge. Borrowed Jan, Ollama, readsb and tar1090 remain untouched.
- Read [PHASE2_TYPE_BOUNDARY.md](PHASE2_TYPE_BOUNDARY.md) for the explicit
  cross-workstream ID/provenance mapping and trust boundary; read
  [PHASE2_ACCEPTANCE.md](PHASE2_ACCEPTANCE.md) for scenario commands,
  unresolved CI and physical-hardware gates.
- Existing Phase 1 mocked tests remain in `tests/mock_integration.rs`.
  Phase 2 adds `tests/typed_phase2.rs` (typed query, action approval,
  STOP and borrowed external tests). **No live MCP/Jan bridge exists.**
