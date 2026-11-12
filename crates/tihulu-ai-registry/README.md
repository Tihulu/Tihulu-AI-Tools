# tihulu-ai-registry — experimental standalone Rust crate

This is an isolated **implementation prototype**, not a running model service.
Use Rust 1.82+ and run:

    cargo test --manifest-path crates/tihulu-ai-registry/Cargo.toml

The crate is deliberately not in the root workspace until the coordinator adds
it to the root Cargo.toml. Remove the crate-local empty workspace section in
that integration commit.

Modules:
- discovery: explicitly approved, bounded, metadata-only Jan Desktop,
  Hugging Face cache and custom-folder scans; caller-supplied authorized local
  Jan/Ollama/OpenAI-compatible API model listings.
- registry: reference-in-place model records; file hints, separate validation,
  endpoint availability and verified-runtime states. Reference IDs are
  noncryptographic and never represent verified content digests.
- leases: task/app lease counts and exact-owner unload signals. It has no
  runtime or process handles and never kills Jan, Ollama or other services.

No filesystem permissions are requested automatically, and scanning never
opens weight payloads. Source directories, endpoint URLs and user-chosen owners
must be provided by the consent UI. Tests create only disposable synthetic
fixtures under the OS temporary directory.

No local Rust toolchain was present in the initial authoring environment;
tests should be executed with Rust 1.82+ by a reviewer or dedicated CI.
Hardware/RAM/VRAM release, genuine API reachability, running Jan/Ollama,
Hugging Face trust semantics and target runtime compatibility are **not**
validated by synthetic tests. See docs/discovery/README.md.
