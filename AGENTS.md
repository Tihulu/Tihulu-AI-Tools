# Planning rules for AI assistants and contributors

**Repository:** `Tihulu/Tihulu-AI-Tools`  
**Current phase:** Planning only (Draft v0.4). This file is a workflow policy for contributions, not permission to implement software.

## Allowed during this phase

- Update Markdown product requirements, decisions, diagrams, design tradeoffs and acceptance tests.
- Improve the **non-functional** UI concept under `ui/` and document that all interaction is simulated.
- Research possible architectures and compare compatibility, clearly separating verified facts from untested assumptions.
- Add reviewed proposed data contracts (for example `aipath.txt`) as **drafts**.

## Not allowed without a separate explicit user instruction

- Implement/deploy Rust executables, servers, agents, auto-installers or background daemons.
- Change any TIHULU application repository, Git branch, release, running service, model files or app settings.
- Download or move models, edit Jan/Ollama stores, reconfigure hardware, open public service ports, or handle credentials.
- Merge, migrate or run production integration work while the independent application audit/split is ongoing.
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

Implementation and integration proposals can be considered **after** the existing Media Center, Flight Radar, Satellite Center, RF Geolocation, Text Center and QA/history repo preservation/split work is complete and the user explicitly authorizes the next phase.
