# Planning rules for AI assistants and contributors

**Repository:** `Tihulu/Tihulu-AI-Tools`  
**Current phase:** Independent AI Tools Rust foundation approved on 2026-11-12. **Only this repository** may gain implementation code in isolated feature branches; existing TIHULU application preservation work remains unchanged.

## Allowed on AI Tools implementation branches

- Add reviewed Rust-first AI Tools source, unit tests and cross-platform CI on an authorized feature branch.
- Update Markdown product requirements, decisions, diagrams, design tradeoffs and acceptance tests.
- Improve the **non-functional** UI concept under `ui/` and document that all interaction is simulated.
- Research possible architectures and compare compatibility, clearly separating verified facts from untested assumptions.
- Add reviewed proposed data contracts (for example `aipath.txt`) as **drafts**.

## Not allowed without a separate explicit user instruction

- Deploy or start Rust servers/agents/installers/background daemons on real devices, or claim hardware acceptance without tests.
- Change any TIHULU application repository, Git branch, release, running service, model files or app settings.
- Download or move models, edit Jan/Ollama stores, reconfigure hardware, open public service ports, or handle credentials.
- Merge to main, release or run production integration with other TIHULU apps while their audit/split is ongoing, unless separately authorized.
- Describe a simulated UI control as a working real download, AI inference, GPU host or third-party application integration.

## Documentation expectations

1. The canonical product baseline is [docs/TIHULU_AI_TOOLS_PLAN.md](docs/TIHULU_AI_TOOLS_PLAN.md).
2. Keep [docs/DECISIONS.md](docs/DECISIONS.md) and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) consistent with any new approved decision.
3. Classify every new item as **confirmed**, **proposed**, or **experimental**. Never silently upgrade proposal to confirmed.
4. Use English for technical writing; note EN/TR/DE UI requirements in plans and mockups.
5. Be explicit about safety: managed and foreign model paths, runtime compatibility, GUI independence, secure remote pairing, scoped agent permissions, and reversibility.
6. Track test prerequisites and distinguish architectural intent from implementation evidence.
7. Keep each planning change reviewable, with a concise Git commit message and changed-doc summary.

## Next phase gate

**AI Tools-only development was explicitly authorized on 2026-11-12**, and may proceed on isolated branches with code, tests and Draft PRs. Integration into the existing Media Center, Flight Radar, Satellite Center, RF Geolocation, Text Center and Survival Kit repositories remains gated on preservation/split completion and explicit permission. See [docs/AGENT_WORKSTREAMS.md](docs/AGENT_WORKSTREAMS.md).
