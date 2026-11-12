# Agent 02 Phase 2 — single authority / typed identity handoff

**Status:** feature-gated real-core forwarding is implemented and tests are
authored, but remain unexecuted in this authoring container. The coordinator
core ResourceLedger lives on sibling Draft PR #12 and sandbox PR #17,
not in Agent 02 PR #9's foundation base.
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

## Phase 3: concrete feature-gated core adapter (PR #17)

Added optional Cargo feature **core-ledger** with a path dependency on
coordinator-owned tihulu-ai-core. This does not change the root workspace or
core, and the default standalone Registry build stays feature-free.

When core contains the PR #12 ResourceLedger API (as copied into sandbox
PR #17), src/core_adapter.rs implements ResourceLedgerPort *directly for*
core ResourceLedger. It uses zero additional runtime counters, and maps:

| Registry port operation | Exact core call |
| --- | --- |
| register_model_context(owner, binding) | ResourceLedger::register(OwnerId(owner.0), ResourceId(binding.resource.0), ResourceKind::ModelContext, mapped Provenance) |
| attach(owner, resource) | ResourceLedger::attach(OwnerId(owner.0), ResourceId(resource.0)) |
| release_one(owner, resource) | ResourceLedger::release_one(...) -> optional mapped CleanupRequest |
| stop_owner(owner) | ResourceLedger::stop_owner(OwnerId(owner.0)) -> all mapped cleanup requests, including NON-model kinds |
| active_lease_count(resource) | ResourceLedger::active_lease_count(ResourceId(resource.0)) |
| complete_cleanup(resource, success) | ResourceLedger::complete_cleanup(ResourceId(resource.0), success) |

All core ResourceKind variants are mapped exhaustively: ModelContext,
GpuAllocation, AudioRamBuffer, AiWorker, SdrHandle, NetworkSession,
RemoteInferenceLease, TemporaryFile. ManagedShared => SharedManaged,
ManagedExclusive(OwnerId) => Exclusive(OwnerId), BorrowedExternal =>
BorrowedExternal. A borrowed provider is never escalated into owned cleanup.
ModelId remains a private registry key, never converted to ResourceId or
OwnerId. External Jan/Ollama paths are never changed.

**Receipt handling:** stop_app_with_receipts calls core stop_owner once and,
only after core confirms success, retires receipts for the stopped owner.
Receipts for unrelated owners remain usable. This avoids a double
release_one after app-wide Stop, without maintaining another lease count.
The outer supervisor must serialize register/attach/release/stop around
the same ledger, and must still cancel actual owned backend work.

**Start/Quit ordering:** tests cover stop-first (attach/register denied),
start-first (one cleanup request after the last owner stops), duplicate Stop,
and a real-core Arc<Mutex<ResourceLedger>> concurrent attach/stop test.
The concurrent test intentionally accepts either permitted lock ordering,
but checks identical final resource state: other clients survive and core
emits at most one last-lease cleanup. These are synthetic invariants,
not a demonstration of physical RAM/VRAM/SDR release.

**Exact commands on an authorized PR #17-type sandbox**, after overlaying
the current Agent 02 crate and removing the crate-local workspace marker
ONLY in coordinator-owned sandbox code:

    cargo fmt --all -- --check
    cargo test -p tihulu-ai-registry --all-targets
    cargo clippy -p tihulu-ai-registry --all-targets -- -D warnings
    cargo test -p tihulu-ai-registry --features core-ledger --all-targets
    cargo clippy -p tihulu-ai-registry --features core-ledger --all-targets -- -D warnings

**On current Agent 02 branch (without the PR #12 core source)**:

    cargo fmt --manifest-path crates/tihulu-ai-registry/Cargo.toml -- --check
    cargo test --manifest-path crates/tihulu-ai-registry/Cargo.toml
    cargo clippy --manifest-path crates/tihulu-ai-registry/Cargo.toml --all-targets -- -D warnings

The core-ledger feature is intentionally NOT enabled on this current
branch because core ResourceLedger has not been merged into the base
branch. Running the feature there should give an unresolved core
resource_lifecycle import, not be misreported as a backend bug. On the
sandbox the feature uses the real coordinator core API.

## Integration blockers and explicit next steps for coordinator

1. PR #12 is an unmerged sibling PR, not an available API on PR #9's
   current base feat/rust-foundation. This branch intentionally does NOT
   import non-existent resource_lifecycle symbols from core.
2. The opt-in direct ResourceLedgerPort implementation is implemented in
   Agent 02's core_adapter.rs. The coordinator must verify it against real
   PR #12 core in the authorized sandbox, explicitly enable core-ledger
   and run full crate tests; no second reference count is allowed.
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
