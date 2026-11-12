# Six parallel agents — TIHULU AI Tools (independent repo only)

**Work phase:** Implementation on separate feature branches, starting from `feat/rust-foundation`. No changes to Media, Text, Satellite, Flight, Geolocation, Survival Kit or their existing audits.

## Branch + path ownership

| Agent | Branch | Owned files/paths | Deliverable |
| --- | --- | --- | --- |
| 01 Platform / Cross-OS | `agent/01-platform` | `docs/platforms/**`, `scripts/**`, isolated CI proposals | Cross-platform packaging matrix, installer safety and native validation plans; no releases |
| 02 Model Registry / Discovery | `agent/02-discovery` | `crates/tihulu-ai-registry/**`, `docs/discovery/**` | Read-only owner-aware registry / Jan paths / stable IDs, no model moves |
| 03 Model Market / Compatibility | `agent/03-market` | `crates/tihulu-ai-market/**`, `docs/market/**` | Offline-first manifest, deterministic compatibility/matching, tests |
| 04 GPU Host / Protocol | `agent/04-compute` | `crates/tihulu-ai-transport/**`, `docs/remote-compute/**` | Authenticated host/client protocol spec and mock fixtures; no public network exposure |
| 05 Jan MCP / App SDK | `agent/05-integrations` | `crates/tihulu-ai-integrations/**`, `docs/integrations/**` | Permissioned Jan MCP bridge design and adapter SDK tests; no Jan config modification |
| 06 Desktop UI / Responsive UX | `agent/06-ui` | `apps/tihulu-ai-desktop/**`, `apps/tihulu-ai-web/**`, `docs/ux/**` | Tauri v2/remote Pi web app foundation plan, chat UX, EN/TR/DE, On-Demand |

### Important shared-file rule

- The bootstrap Rust core (`crates/tihulu-ai-core/**`), CLI (`apps/tihulu-ai-cli/**`) and root `Cargo.toml` are **coordinator-owned**. Agents should propose root workspace changes in their PR descriptions rather than editing these shared files.
- Existing `ui/index.html` is the frozen concept reference unless Agent 06 receives an explicit UI migration task; do not cause concurrent edits.
- All agent branches should be based on `feat/rust-foundation` and their **Draft PRs should target that branch** until foundation approval. Do not merge PRs or push main.
- All agent code must be independent and runnable with `cargo test --manifest-path <owned-crate>/Cargo.toml` when possible. An agent may depend on `tihulu-ai-core` via path dependencies but must not change it without coordinator review.
- Maintain English engineering docs and an EN/TR/DE UI target, Rust-first implementation, supported per-runtime hardware verification.

## Assignment issues (one chat agent per issue)

1. [Agent 01 — Platform and CI](https://github.com/Tihulu/Tihulu-AI-Tools/issues/2)
2. [Agent 02 — Model registry and discovery](https://github.com/Tihulu/Tihulu-AI-Tools/issues/3)
3. [Agent 03 — Market and compatibility](https://github.com/Tihulu/Tihulu-AI-Tools/issues/4)
4. [Agent 04 — GPU host and secure protocol](https://github.com/Tihulu/Tihulu-AI-Tools/issues/5)
5. [Agent 05 — Jan MCP and app SDK](https://github.com/Tihulu/Tihulu-AI-Tools/issues/6)
6. [Agent 06 — Desktop/Pi web UI](https://github.com/Tihulu/Tihulu-AI-Tools/issues/7)

Each branch was updated to the tested foundation head as of 2026-11-12. Agents have **not** started work merely because these issues and branches exist. The user can provide each issue link to an independent chat agent.

## Agent acceptance contract

1. Read `AGENTS.md`, `docs/IMPLEMENTATION_START.md`, the relevant GitHub Issue and the v0.7 full plan.
2. Use the assigned branch and stay **within path ownership**. Keep GitHub issue/PR progress documented.
3. All dependencies, compiled targets and runtime assumptions must be explicit. Avoid unauthorized filesystem reads, model downloads, services, GPU jobs or real Jan changes.
4. Add focused deterministic tests and report what **actually ran**. Never equate mocked inference with hardware acceptance.
5. Prefer no new runtime dependencies without justification; never commit secrets, downloaded model weights or local machine paths.
6. Make no changes to other TIHULU repos, their open preservation audits, installations, active hardware or releases.
7. Submit source-backed proposals and **Draft PRs only** for coordinator review. Do not merge without user authorization.

## Integration order

Foundation PR → review baseline builds → independently review the six agent PRs → reconcile model/task/host contracts → integrate guarded components in stages → test on actual Pi 5 with and without AI HAT+ 2, RTX PC, Windows and Apple Silicon → only then issue separate app-repository integration work.
