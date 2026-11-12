# Agent 02 Phase 2 — single authority / typed identity handoff

**Status:** proposed integration contract, backed by synthetic registry
unit tests only. The real coordinator core ResourceLedger is present
on sibling Draft PR #12, not in the base of Agent 02 Draft PR #9.
References: Issue #15, Issue #3, Resource Lifecycle v0.9 and PR #12.

## Decision: ONE runtime/model resource ledger

**Authoritative state lives only in coordinator core ResourceLedger.**
The Phase 1 registry LeaseRegistry was an independent runtime reference
counter and emitted its own UnloadOwned / DisconnectBorrowedClient actions.
That was incompatible with PR #12: the same last lease could produce two
different unload decisions, and the duplicate state could diverge on Quit.
Phase 2 removes that entire *production* LeaseRegistry and its unload logic.

Agent 02 keeps only the reference-in-place *catalog*, typed task metadata
and a ResourceLedgerPort forwarding boundary. The test-only
MockResourceLedger is not linked in production or an alternative
ownership authority.

## ID and ownership mapping

| Registry / adapter type | Core / runtime mapping |
| --- | --- |
| ModelId::Artifact | Full source adapter, model file owner, original and canonical paths; private stable provenance identity, NOT a runtime handle |
| ModelId::Endpoint | Full source adapter, provider owner, approved endpoint URL, served model ID; NOT a model weight file |
| ModelOwner | Jan Desktop / Ollama / Hugging Face / named user / TIHULU file owner; never implies authority over a live runtime |
| ModelRuntimeBinding.model_id | References a registry source ID only; one source may be loaded in multiple distinct contexts |
| RuntimeInstanceId | Trusted host-side instance name; metadata only, no kill by process name |
| LedgerOwnerRef(u64) | One-to-one conversion to PR #12 core OwnerId(u64), allocated by trusted session authority |
| LedgerResourceRef(u64) | One-to-one conversion to core ResourceId(u64), unique for the lifetime of the actual ledger; never derived from a ModelId or reused |
| ConsumerTask | AppSessionId + TaskSessionId for task attribution; core still owns per-owner reference counts |
| ContextAuthority::ManagedExclusive(owner) | core Provenance::Exclusive(OwnerId(owner.0)) |
| ContextAuthority::ManagedShared | core Provenance::SharedManaged |
| ContextAuthority::BorrowedExternal(owner) | core Provenance::BorrowedExternal; the owner label is descriptive, NOT permission to terminate |
| LedgerResourceKind | One-to-one names of all eight PR #12 core ResourceKind variants |
| LedgerCleanupRequest | One-to-one core CleanupRequest; output of core only |

ModelId is a typed, ordered enum that embeds the complete provenance and
original path without lossy string formatting, hash collisions, or
registration-order suffixes. This is stable while provenance and target
paths remain the same. It is NOT a SHA-256 digest, a public opaque ID, a
model version, a security capability or verified runtime compatibility.
It may contain private file paths: do not log, send over the network,
put in GitHub issues, or persist outside a private user-owned registry.
The coordinator may introduce opaque transport handles separately.

## The adapter contract

ResourceLedgerPort provides precisely these operations:

1. register_model_context => ResourceLedger.register with
   ResourceKind::ModelContext and mapped Provenance. **Core register already
   creates the first holder lease.** Therefore register_initial_task must
   NOT also call attach.
2. attach => ResourceLedger.attach; one new authorized task receipt, one
   additional core reference.
3. release_one => ResourceLedger.release_one; core alone decides whether
   the last holder produces a cleanup request.
4. stop_owner => ResourceLedger.stop_owner; core atomically stops future
   acquisitions and releases all this app owner's references, including
   contexts/buffers/workers tracked outside this registry. Do not filter
   out non-model cleanup requests.
5. active_lease_count => ResourceLedger.active_lease_count; never
   duplicate or cache a local runtime reference count.
6. complete_cleanup => ResourceLedger.complete_cleanup; only the actual
   backend may acknowledge a confirmed unload/close. Failed cleanup stays
   reported and requires core's explicit retry path.

TaskLeaseReceipt is a client-held single-use reference. Its released flag
guards accidental double invocation on *that same receipt*, not the
actual runtime reference count or the final-unload decision. App-wide
stop invalidates outstanding task receipts; do not call release_one
again for those already covered by stop_owner. Duplicate stop_owner must
return no new cleanup. Core owns its own Stopping/Stopped state.

**Important adapter rule:** Registering a model in the discovery registry
NEVER allocates an actual runtime resource. register_initial_task may be
called only after authorized runtime backend obtained the real model
context or after approved external provider connection setup.

## Safe worked synthetic scenarios

- **L-03, two apps / one context:** the first app registers shared
  managed model resource 100 with its first task receipt. The second
  authorized app attaches to resource 100. First app Stop removes its
  reference only, no cleanup. Second app Stop is the sole operation
  generating core CleanupRequest(ModelContext, 100).
- **L-04, final release:** an Advisor and Agent attached to the same
  resource receive two separate receipts. Advisor finishes: no cleanup.
  Agent finishes: core emits one cleanup request. Backend must unload,
  then call complete_cleanup. Duplicate receipt release does nothing.
- **L-07, borrowed Jan/Ollama:** existing provider sessions are registered
  as BorrowedExternal; App Stop releases references but does not emit
  provider unload or kill. TIHULU-owned client streams can be canceled
  by their separately tracked owner-only resource adapters.
- **L-15, same model / multiple contexts:** same ModelId may map to
  ResourceId 105 and 106 on an independently shared host. Stopping one
  app may unload 105 only; runtime instance 106 remains leased.
- **Duplicate Stop:** repeated stop_owner produces no new request and
  blocks a subsequent attach. No extra client/model cleanup occurs.
- **External file in TIHULU runtime:** a Jan-owned GGUF file can be used
  by a distinct TIHULU-managed runtime context; last lease unloads only
  that context. The registry never deletes, moves, chmods or alters
  the Jan-owned model file.

## Integration blockers and explicit next steps for coordinator

1. PR #12 is an unmerged sibling PR, not an available API on PR #9's
   current base feat/rust-foundation. This branch intentionally does NOT
   import non-existent resource_lifecycle symbols from core.
2. After approved coordinator integration, implement ResourceLedgerPort
   in a permitted integration-owned adapter using exact core types and
   one-to-one forwarding. The adapter must not introduce a second
   reference count, unload policy, or process-name termination.
3. Core uses OwnerId-based lease counts but does not identify TaskSessionId.
   The trusted app supervisor must maintain task receipt ownership and
   enforce authorization. A single global ledger on the runtime-owning
   host (or its authenticated host authority) is needed; two processes
   must not claim to be the ledger for one physical loaded model.
4. Ledger ResourceId and OwnerId allocation is not handled here; the
   coordinator must ensure uniqueness, no stale-ack replay after restart,
   and explicit app/host session lifetime mapping.
5. Concrete backend must supervise bounded worker teardown, terminate
   only owned children, close client streams, avoid killing Jan/Ollama,
   and verify owner-attributable memory/device release. None of this is
   proven by the synthetic crate tests.
6. Exact cross-branch integration tests should call the REAL core
   ResourceLedger on Linux x86/ARM, macOS Apple Silicon, Windows x86/ARM.
   Root-workspace or PR #12-only green CI does not execute this standalone
   registry crate until the coordinator explicitly adds it to test scope.
7. Real Pi 5, Hailo-10H, NVIDIA GPU, macOS Metal, Windows, real Jan/Ollama
   and SDR acceptance must remain pending until separately authorized.

No modifications to coordinator-owned core, CI, root Cargo.toml, main,
other workstreams, other TIHULU repositories or external applications.
