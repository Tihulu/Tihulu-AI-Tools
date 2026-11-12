# tihulu-ai-registry — standalone experimental Rust crate

Phase 2: the coordinator's core ResourceLedger (Draft PR #12) is the
**only runtime/model lease authority**. The old production LeaseRegistry
and its competing unload decisions were removed.

Rust 1.82+, no external dependencies. Verification commands:

    cargo fmt --manifest-path crates/tihulu-ai-registry/Cargo.toml -- --check
    cargo test --manifest-path crates/tihulu-ai-registry/Cargo.toml
    cargo clippy --manifest-path crates/tihulu-ai-registry/Cargo.toml --all-targets -- -D warnings

This crate remains outside the root workspace. Only the coordinator may
remove its local workspace marker and add it to the root.

Modules:
- discovery: consent-gated, bounded, metadata-only Jan/HF/custom-folder
  discovery and caller-provided authorized local API model listings.
- registry: reference-in-place records; typed ModelId retains source owner,
  exact paths or endpoint identity. No filename hash is claimed as a digest.
- leases: typed ModelRuntimeBinding and TaskLeaseReceipt. Operations delegate
  to ResourceLedgerPort, with NO second production runtime count.

The initial ledger registration already holds ONE initial owner reference.
Subsequent tasks each attach exactly once. Task completion calls the ledger's
release_one, and app stop delegates to its stop_owner. All cleanup events
are produced by the ledger and must be handled/acknowledged by authorized
backend adapters.

Synthetic MockResourceLedger exists ONLY in test code, not in production.
The concrete ResourceLedgerPort implementation must be integrated against
the real core ResourceLedger after Draft PR #12 is reconciled. Rust mock tests
cannot demonstrate real GPU/NPU/SDR memory release or Jan/Ollama behavior.
Read docs/discovery/LEDGER_HANDOFF.md for the detailed contract and blockers.
