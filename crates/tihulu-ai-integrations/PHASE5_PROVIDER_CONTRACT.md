# Phase 5 — optional Jan Desktop and Ollama provider contracts

**Status: offline-only, mock-backed integration SDK.** AI-05 PHASE5-JAN,
based on coordinator Draft PR #19 (`coord/phase4-native-inventory`).
This crate ships no live Jan MCP bridge, Ollama HTTP client, inference worker,
filesystem model scanner, process manager, configuration writer or installer.

## Safety and authorization contract

1. Jan Desktop and Ollama are independent **BorrowedExternal** services. Their
   configurations, model files, processes and retained RAM/VRAM remain theirs.
   Do not connect, enumerate, execute, unload, restart or terminate them
   without the applicable separately authorized capability. No API in this
   crate can terminate or modify a borrowed provider.
2. Trusted host code obtains **explicit owner consent** for the selected
   `ProviderKind`, authenticated `ProviderCaller` (client/app/session) and
   `ProviderGrant`. `approve_connection` only records an in-memory grant:
   it never probes or connects. With no consent, even a read-only status or
   model list does **not** call any provider port.
3. `ProviderDiscoveryPort` supports status and bounded model-list metadata.
   Status is Ready, Degraded or Unavailable only after authorized probing;
   otherwise AuthorizationRequired or Stopped is returned without I/O.
   Degraded permits metadata listing but **never** mock execution.
4. A `ProviderModel` separates the stable `RegistryModelId` from the
   provider-specific `ProviderModelId`. The former is a provenance reference,
   not a path or checksum. `ListedOnly` does NOT mean verified runnable.
   `VerifiedRunnable` must eventually come from independently authenticated
   runtime evidence, never from untrusted model or provider text.
5. Each grant has exact allowed Registry model IDs, allowed task kinds, a
   read-only discovery flag and an input-size limit. No wildcard execution
   grant, arbitrary endpoint, path or credential is accepted.
6. An action additionally requires a **single-use, exact-request** approval
   issued only by a trusted host after user confirmation. The ticket binds
   client, app, session, provider, Registry model, provider model, task,
   job ID and input size. Renewing consent or revoking a provider invalidates
   its old tickets; a denied or mismatched action cannot dispatch a job.
7. `submit_mock` verifies the still-running owner session and active grant,
   consumes the ticket, checks provider Ready and matching verified model,
   prepares an *inert owned request handle*, registers it with the lifecycle
   supervisor, and calls the **offline fake-server** execution port. There is
   no live inference, socket or Jan/Ollama connection in this milestone.
8. `cancel_owned` cancels only the authenticated caller's specific request.
   `revoke_provider` cancels only that caller's streams for the selected
   provider, leaving the other provider and other clients undisturbed.
   `stop_session` and Quit close only that app's tracked owned requests.
   Repeated Stop is idempotent; the session remains a bounded tombstone.
9. `force_close_owned` is only for the *captured app-owned stream handle*.
   It must not kill a process by name, shut down a foreign service, unload
   someone else's model, stop the shared host or force-release an SDR lock.
   The coordinator's authoritative model/resource ledger decides shared
   last-lease unloading. No duplicate unload decision is introduced here.

## Offline mock test coverage

`tests/phase5_providers.rs` implements `MockServer` with in-memory state
and no network, filesystem, external process or model execution. It covers:

- Denied/missing consent: no provider status probe, model listing or execution.
- Separate Jan/Ollama permissions; read-only metadata after consent.
- Unavailable and degraded handling; no job submission.
- Exact single-use approval, replay and cross-client denial.
- Model/task/size limits, ListedOnly, contradictory/mismatched model references.
- Ownership-isolated cancellation, app-specific Stop/Quit and repeated Stop.
- Jan/Ollama/readsb borrowed handles preserved; other clients unaffected.
- Provider-level revoke, invalid input/URL/path labels, session impersonation.

The trusted host API must not be exposed as MCP tools, LLM instructions, raw
Tauri IPC parameters or unverified JSON fields. Real pairing/authentication,
transport privacy, origin verification, safe installed-model enumeration,
provider version detection, inference, cancellation/stream backpressure and
actual VRAM/process cleanup are separate future integration gates.

### Required acceptance commands

```sh
cargo fmt --manifest-path crates/tihulu-ai-integrations/Cargo.toml --check
cargo test --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets
cargo clippy --manifest-path crates/tihulu-ai-integrations/Cargo.toml --all-targets -- -D warnings
```

CI results must be distinguished from source-only fixture review, and mock
success is never proof of physical GPU/NPU/SDR release, installed Jan/Ollama
survival or real provider authentication. No tokens are stored in `aipath.txt`.
