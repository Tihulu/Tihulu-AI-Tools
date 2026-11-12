# Phase 2 — Proposed shared Rust interface boundaries

**Status:** Coordinator proposal for agent review, **not a deployed or accepted API**. All agent PRs remain Draft and target `feat/rust-foundation`. Treat these as conceptual contracts until the reviewed code agrees.

## Ownership boundaries

| Component | Authoritative over | Must **not** do |
| --- | --- | --- |
| **Registry (Agent 02)** | Model discovery identities and source provenance; reference-in-place paths, endpoint registrations and verification state | Decide to kill a GPU worker/Jan process based only on model file registration |
| **Market (Agent 03)** | Model/task/runtime compatibility evaluation based on trusted probes, model manifests and app adapter capabilities | Mark installed model as runtime-verified without a real test or run arbitrary LLM-produced action |
| **Core ResourceLedger (Coordinator PR #12)** | App/session/runtime resource owners, actual active lease count, last-owner cleanup request and completion acknowledgment | Claim physical GPU/SDR release merely by changing ledger state |
| **Remote GPU Host (Agent 04)** | Authenticated paired-client identity and grants, host/job IDs, request cancellation/timeout, model-host execution lease mapping | Accept client-provided model paths or allow one client to terminate another client's shared host job |
| **Integration SDK (Agent 05)** | App adapters, MCP tool exposure and per-action approval, app-side cancellation and configuration diff approval | Give Jan/global LLM arbitrary shell or filesystem write authority |
| **UI (Agent 06)** | User's selected model/host/task, approved intents, conversation state and status displays | Implement hardware/process cleanup in JavaScript alone or label a mock GPU host as connected |

### Single authoritative unload decision

Core `ResourceLedger` is the **proposed** authoritative registry of *loaded* TIHULU-owned runtime contexts and device handles. It does not own model files.

- Registry's `LeaseRegistry` may represent **task tokens and metadata**, but it must not independently invoke physical unload when a core ResourceLedger lease is still active; a Rust adapter should coordinate/acquire/release the core lease.
- Remote Host tracks **network and client lease IDs**, validates user/app grants and maps these to a core owner/session lease. It must issue **one** last-lease cleanup request via the authoritative ledger and wait for runtime adapter acknowledgment.
- Imported Jan/Ollama models are *file owner = external*. If runtime = external (Jan/Ollama server), core provenance is `BorrowedExternal`, so release only the TIHULU request/stream. If TIHULU separately loads a Jan-owned file into its own verified runtime, *the runtime context* is TIHULU-managed and may unload on its last lease **without altering Jan's files or Jan's process**.
- SDR direct handle ownership is independent of model ownership; a client of pre-existing readsb/tar1090 does not own their USB device handle or process.
- Runtime adapter must confirm actual cleanup (model unload, GPU allocations, subprocess termination, SDR close). Failed cleanup stays visible for bounded retry and diagnostics; no process-name killing.

## Conceptual identifiers

| Logical key | Producer/authority | Consumer |
| --- | --- | --- |
| `ModelRef` | Registry: opaque ID + provenance + optional version/digest + source/endpoint | Market, app mapping, host |
| `AppId` / `TaskKind` | Integration SDK/actual app adapter with stable names | Market, GPU host, UI |
| `RuntimeId` / `ResourceId` | Trusted runtime adapter/Core on the machine where model really loads | Core ledger, host |
| `OwnerId` | Trusted supervisor for app/agent/job session | Core ledger |
| `LeaseId` | Trusted model runtime/host only; bound to authorized app/session | GPU host, core adapter |
| `HostId` | Verified pairing registry + trusted host identity | Client router/UI |
| `RequestId` | Authorized client scoped to host/app + replay defense | GPU host, audit |
| `McpToolId` | Integration SDK from implemented adapters only | Jan Desktop MCP client |

Avoid embedding model filesystem paths or executable commands in network messages; a `ModelRef` must be resolved by the **target host's own** user-approved model registry. Never store credentials in `aipath.txt`.

## Mock end-to-end request contract (proposed)

1. Authorized application describes `(AppId, TaskKind, input modality)`.
2. Registry resolves a user-approved `ModelRef` and source ownership.
3. Market checks verified runtime, task, language, memory, accelerator and license suitability; unknown remains unknown.
4. Host router selects local CPU/GPU/NPU or authenticated paired host using approved policy and host-specific verified model.
5. A trusted supervisor opens `OwnerId`, acquires one runtime resource lease per active task, and supplies a cancellable job ID.
6. Integration SDK allows only explicitly approved app capability; Jan MCP remains a **client of permissioned tools**, not a backend owner or root supervisor.
7. UI displays true execution source, current job state, active leases, shutdown status and failed cleanup.
8. On task cancellation/App Quit/Hard Stop, supervisor rejects new work, drains its jobs, releases its leases, and only if the **last** owned runtime lease ended issues cleanup to the trusted backend. Borrowed services and other app clients continue.
9. GPU host's independent opt-in service may remain running after a UI closes; model remains loaded only with other active authorized leases or an explicit retention setting.

## Gate conditions before merging

- Define a single concrete Rust ID mapping or adapter design with all six agents before schema freeze.
- Run the independent agent tests through [Phase 2 standalone CI](PHASE2_AGENT_VALIDATION.md) and fix rustfmt/Clippy/test issues on agent branches.
- Build a coordinator-owned mock assembly branch and perform the first actual **inter-crate** compile+test.
- Check intentional resource cleanup, cross-app preservation, error/crash/disconnect races and EN/TR/DE UI status transitions.
- No remote server launch, hardware allocation, model download/relocation, Jan settings write, source-app edits or release is authorized by this proposal.
