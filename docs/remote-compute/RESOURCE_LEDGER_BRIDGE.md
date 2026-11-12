# Phase 2 — Registry ↔ Remote Host ↔ ResourceLedger contract

**Status 2026-11-12: proposed integration bridge with typed Rust contracts and deterministic in-memory tests.** No direct dependency on coordinator PR #12 has been added because that coordinator-owned module is on a different unmerged Draft PR. **No real GPU, TLS, OS runtime or backend unload is executed.**

## Reviewed source contracts

- Registry Agent 02 Draft PR [#9](https://github.com/Tihulu/Tihulu-AI-Tools/pull/9): `ModelRecord.id: String` is a provenance-based **reference key**, generally `ref-v1-...` (not a verified file digest). A source file owned by Jan may still back a separately TIHULU-owned runtime context. Registry `LeaseRegistry` currently emits `ReleaseAction::UnloadOwned` after its final local reference; **this action MUST NOT be wired as an independent command to unload a GPU host model**.
- Coordinator Draft PR [#12](https://github.com/Tihulu/Tihulu-AI-Tools/pull/12): `resource_lifecycle::{OwnerId(u64), ResourceId(u64), ResourceLedger, Provenance, ResourceKind, CleanupRequest}`. `release_one` / `stop_owner` yield a `CleanupRequest` on the last managed reference; `complete_cleanup` is called only after the authorized backend performs/verified release. Borrowed external provenance NEVER yields an unload.
- This Remote Host Draft PR [#13](https://github.com/Tihulu/Tihulu-AI-Tools/pull/13): `ModelId`, `RuntimeId`, `HostId`, `ClientId`, `AppId`, `AppSessionId`, `TaskId`, `LeaseId`, `RequestId`, `LedgerOwnerId` and `LedgerResourceId` are distinct types. `LedgerRuntimeBinding` maps a registry reference to one loaded runtime instance / core resource. `LedgerLeaseEvent` represents a per-client/per-task lease acquisition/release.

## Authority: one cleanup decision, not three

**Production contract: choose `LifecycleAuthority::CoordinatorLedger`.** The standalone `MockLocal` mode exists solely for pre-integration synthetic regression tests and its host-side last-lease synthetic unload must never be wired alongside Registry `ReleaseAction::UnloadOwned` or core `ResourceLedger` cleanup.

| Concept | Owner / authority | Mapping |
| --- | --- | --- |
| Source asset | Registered file/API owner (Jan/Ollama/user/AI Tools) | Registry `ModelRecord.id` → host `ModelId`, exact key; no client path |
| Loaded model/runtime context | Verified executor that owns the context, possibly different from asset owner | Host `RuntimeId`; registration is host-local and immutable for that instance |
| Ledger resource | Single trusted coordinator ledger on **the execution PC** | `ResourceId(u64)` ↔ host `LedgerResourceId(u64)` via host-admin `LedgerRuntimeBinding`; never derived from a hash/path |
| Ledger owner | Authorized app **launch** on a given paired client, not arbitrary app name | `OwnerId(u64)` ↔ host `LedgerOwnerId(u64)` assigned by coordinator and keyed by (client, app, app_session) |
| Work/task | A particular Advisor/Agent or app task within its launch | Host `TaskId` ↔ Registry `Consumer.task_session` |
| Remote lease | One authorized task→host-runtime reference | Host `LeaseId`; repeated opens with the identical scope are idempotent; used IDs never recycled during host lifetime |
| Inference invocation | One attempt/stream belonging to the lease | Host `RequestId`; replay of exact same spec accepted only for same authorized task/client |
| Device host identity | Paired PC, independent of GUI window | `HostId` and trusted pin; independent host must remain if other clients use it |

`ModelId` is the **stable Registry reference ID**, not the runtime ID, not a model file path, not the SHA-256 digest of a staged asset and not an unverified served-model name. Different compatible files and execution contexts can point to different runtime resources even when they refer to the same catalog model. Source ownership and runtime provenance must be recorded independently. `RuntimeOwnership::BorrowedExternal` maps to coordinator `Provenance::BorrowedExternal`; a TIHULU-owned context backed by Jan's file maps to `Provenance::SharedManaged` (or `Exclusive(owner)` when truly exclusive), **not** BorrowedExternal solely because its model file resides in Jan.

## Exact proposed event delivery algorithm

This is a target adapter algorithm for coordinator review, **not an implemented cross-crate integration**.

1. A trusted host admin registers an allowed Registry model reference, chooses an actual runtime ID and allocates a **new** numeric `ResourceId` for that runtime context. The coordinator opens a unique numeric `OwnerId` for each authenticated app launch on each paired device. The host is configured with `set_lifecycle_authority(CoordinatorLedger)`, `bind_ledger_runtime` and `bind_ledger_owner` **before** it accepts that model's lease. No client may select an owner/resource number or raw file path over the network.
2. After a real backend confirms its model/context is loaded, coordinator calls `ResourceLedger.register(owner, resource, ModelContext, SharedManaged)` for the first authorized reference (or `BorrowedExternal` for an actually borrowed runtime). For subsequent acquired leases of this runtime, it calls `ResourceLedger.attach(owner, resource)`. The proposed adapter consumes the host's ordered `LedgerLeaseEvent::Acquired` events and must serialize ledger mutations with the host acceptance decision to prevent split-brain registration.
3. When a task/app exits, the host rejects new work on *that* lease, cancels its queued/running work and emits exactly one `LedgerLeaseEvent::Released` **after** the local mock job finishes/cancels. The adapter deduplicates by `(HostId, event.sequence)` with durable replay protection, then calls core `ResourceLedger.release_one(owner, resource)`. The first client's release returns **None** if another Pi/TIHULU app still holds a reference. No other application's work is cancelled.
4. On final **managed** reference release, core returns exactly one `CleanupRequest { resource, kind: ModelContext }`. Only that request causes a verified owner-specific runtime adapter to unload the context. On success the adapter calls `ResourceLedger.complete_cleanup(resource, true)` and then a trusted host-local `acknowledge_ledger_unload(model, resource, generation, true, time)`; the host mock finally decreases its owner-attributable **synthetic** VRAM count and permits a new runtime generation. The model file is not removed.
5. On runtime unload failure core records `CleanupFailed` with `complete_cleanup(..., false)`; owner diagnostics remain visible and the host remains `awaiting_ledger_unload`. Only the coordinator's `retry_cleanup(resource)` should retry backend teardown; never interpret a mock timeout as physical release.
6. With `BorrowedExternal`, the final `release_one` produces **no** `CleanupRequest`. The adapter drops our client references only. No TIHULU host command unloads Jan/Ollama/readsb/tar1090 or their memory.
7. The Registry's `LeaseRegistry` may provide consumer/session metadata, but **must not issue independent unload calls** when the coordinator ledger owns the runtime. Its `UnloadOwned` is at most a compatibility hint to reconcile with the core cleanup request; it is not another cleanup authority. If the Registry ever tracks already-loaded contexts, release events must reconcile with core, not double-decrement core references.
8. `acknowledge_ledger_event` accepts only the oldest queued event. The host will refuse a runtime cleanup acknowledgement while there are **unacknowledged acquire/release events** for that model. Retried event delivery is safe only if the coordinator actually deduplicates and durably records each event before acknowledging it.

The test-only `LedgerFixture` checks event replay/order, two clients sharing a resource and exactly one final cleanup request. **It is not the coordinator's actual `ResourceLedger`**, cannot validate cross-PR compilation and must be replaced by a true integration test after the coordinator merges/rebases reviewed contracts.

## Shutdown rules / race handling

- `close_task` drops only (client, app, app_session, task) leases; `close_app` drops the calling app launch, not another instance; `close_client` is a privileged device-wide host callback, **not** a generic stream-close callback.
- Lost one Pi link does not disconnect the other: missing heartbeat expires only its leases. A late `finish_job` cannot resurrect cancellation. Close and cancel only accept the original scoped owner; lease tombstones prevent old IDs from binding a new context. TTL is evaluated by host local monotonic time in production (the mock receives synthetic millisecond ticks); real handshake must handle clocks, persistence and restart.
- Newly opened leases on a model awaiting authoritative unload are rejected `LedgerCleanupPending`, preventing a late cleanup acknowledgement from unloading a newer session. `generation` and `ResourceId` fence stale ACKs.
- Host-owned model load/unload counters and VRAM values are *synthetic* only. An independently enabled GPU host stays alive during a client's Quit. An explicit host-owner drain remains `Draining` until outstanding model cleanup ACKs and ledger events are complete.
- Grant changes are host-local. A narrowed grant closes forbidden leases; app-session credentials must be separately attested from device identity. Cross-client/per-app cancellation is denied even if the client knows a request ID.
- The mock uses capped identity/event tables and refuses new leases if it cannot reserve capacity for future closing events. Production needs persisted monotonic event IDs, fenced host-incarnation IDs, durable deduplication, bounded spool/storage and recovery after host restart. Mock in-memory state alone does not guarantee exactly-once delivery through a crash.

## Remaining implementation gates

- Coordinator must review/approve a shared type policy and integrate the actual `tihulu-ai-core::resource_lifecycle` implementation from Draft PR #12 with Registry's LeaseRegistry in one atomic host-side component. This branch **deliberately does not** add a `tihulu-ai-core` path dependency on an unmerged PR.
- Proven mTLS and per-app cryptographic attestation, rotation/revocation, TLS cert storage, streaming, origin policy, UI and staging authorization are not implemented. `PeerEvidence` values are **simulation inputs**; never construct them from unchecked JSON.
- Real backend should expose verified unload/worker join/error reporting and measured owner-attributable GPU VRAM. Mock counters do not prove actual CUDA/Hailo/SDR release.
- Coordinator-owned cross-platform CI must actually invoke `cargo fmt/test/clippy --manifest-path crates/tihulu-ai-transport/Cargo.toml`. Root CI green status alone cannot validate this crate. The shared root Cargo.toml and CI remain untouched here.
