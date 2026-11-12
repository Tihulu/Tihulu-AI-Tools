# Coordinator Integration Sandbox — Phase 3 repair snapshot

The coordinator copied the reviewed GPU Transport source at **`d397b50e39e086e0d631f1b9bd0f75b5a32e84f2`** from `agent/04-compute`, which includes the scoped `new_binding.model` ownership correction for E0505.

In this **sandbox branch only**, the Jan Integration mock lifecycle test was corrected to assert the production contract:
- A shutdown with an unresolved owned handle remains `SessionState::Stopping`, never falsely `Stopped`.
- A newly opened fresh session is `SessionState::Running`.

Neither Agent 04 nor Agent 05 branches have been modified by this coordinator experiment. Agent 05 must adopt the equivalent typed assertion correction in its own branch when ready.

The four independent crates are still workspace members only in this sandbox. This patch does not claim real Jan MCP, authenticated GPU inference or hardware release. Multi-OS CI remains the authoritative build/test evidence.
