# TIHULU AI Tools

**Status: Planning only — Draft v0.4 (12 November 2026)**

TIHULU AI Tools is a planned **Rust-first**, cross-platform AI model management and optional inference-hosting application. The current repository contains **product plans, contracts and interface mockups**, not a functioning installer, AI runtime, marketplace, server, or agent.

> **Development gate:** Do not implement integrations or change existing TIHULU applications before the independent repository separation and preservation audits are complete. No automatic model moves, background inference, external API exposure, or installer changes are authorized by these planning documents.

## Start here

- [Full architecture and product plan](docs/TIHULU_AI_TOOLS_PLAN.md) — original v0.4 design/requirements baseline.
- [Architecture diagrams (Mermaid)](docs/ARCHITECTURE.md) — source-editable diagrams rendered directly by GitHub.
- [Decision register and implementation gates](docs/DECISIONS.md) — confirmed requirements versus proposed implementation.
- [UI concept](ui/index.html) — **interactive mock only**; no real system or network actions.
- [Proposed `aipath.txt` contract](docs/AIPATH_CONTRACT.md) — optional application mapping, path fallback and migration safety.

## Design goals

| Area | Goal |
| --- | --- |
| Platforms | Linux x86-64, Raspberry Pi 5 ARM64/8 GB, Raspberry Pi 5 + AI HAT+ 2/Hailo-10H; later Windows x64/macOS ARM64 |
| Model Market | Hardware-aware catalog, explicit installation/update, custom model import |
| Shared models | Discover existing Jan/Ollama/other libraries, avoid duplicate downloads, safe link/copy/move |
| Storage | User-visible `~/TIHULU AI Tools/Models/` by default; configurable and relocatable |
| Inference | Local CPU/GPU/NPU with verified compatibility, optional secure LAN/Ethernet GPU host |
| Clients | TIHULU Media, Text, Flight Radar, Satellite, Geolocation, Survival Kit; opt-in external apps |
| Independence | Apps start without the AI Tools **GUI**; local config and legacy fallback remain functional |
| Assistants | Experimental AI Advisor and permission-gated App Control Agent |
| Implementation | Rust-first core and control plane; task-appropriate external inference runtimes via adapters |
| Policy | On-Demand default, explicit permissions, offline-first, EN/TR/DE UI; English engineering docs |

## Hardware preview presets

`Auto Detect` · `Regular Computer` · `Performance / Experimental` · `Raspberry Pi 5 — 8 GB` · `Raspberry Pi 5 + AI HAT+ 2` · `Custom`

**Compatibility is verified per model, runtime and device; architecture support alone does not mean Hailo NPU support.** A remote browser client does not automatically contribute phone GPU capacity.

## Planning workflow

1. Record every approved decision in the Markdown plan / decision log.
2. Keep architecture diagrams, UI examples and acceptance tests aligned with the plan.
3. Distinguish **confirmed requirements**, **proposals** and **experimental concepts**.
4. Make **documentation-only** changes while the six-application preservation/split work is ongoing.
5. After that work is completed, design scoped integration requests per app; only then consider implementation.

This repository is the intended source of truth for TIHULU AI Tools planning going forward.
