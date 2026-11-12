# TIHULU AI Tools

**Status: Planning only — Draft v0.7 (12 November 2026)**

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

## v0.5: shared host for AI Tools itself

TIHULU AI Tools on a Raspberry Pi 5 can be configured to use a paired x86-64 PC GPU as the inference host for **its own AI Advisor and App Control Agent chats**. The same paired PC may be selected per AI task by independent TIHULU applications, including Survival Kit, without a separate copy of PC-hosted model weights on Pi. A remote Rust host service must be running and securely paired for remote inference; an application does not need the AI Tools **GUI** open to start.

The two experimental AI tabs now use **chat-style mockups** with message history, input composer, compute selectors (**Auto / This device / Paired PC GPU**), and visible permission controls. The GPU Server tab previews routing its host to AI Tools features and selected TIHULU apps. All UI responses and host assignments are **simulated**; no inference, file or app changes occur.

See [v0.5 feature specifications](docs/TIHULU_AI_TOOLS_PLAN.md#35-v05-approved-requirement--ai-tools-itself-consumes-a-paired-gpu-host), [editable diagrams](docs/ARCHITECTURE.md), and the [interactive concept](ui/index.html).

## v0.6: Existing AI apps, model matching and optional Jan chat tools

**User-confirmed plan:** AI Tools may discover models in an installed application by its name, its user-approved data directory, an explicitly selected model folder, or a documented local inference endpoint. With read-only consent it will add those external models to **My Models** as references, not duplicate files. **Jan** is a first-class discovery target, and other providers can be added via extensible source adapters.

The **Model Market** and **TIHULU Apps** screens will explain which pre-installed models can serve each supported app's AI tasks and why. Users may choose **Use this model**, after compatibility checks and an approved mapping preview, without moving the provider-owned model. Separate safe copy/move and rollback plans remain optional. An experimental AI Matching Advisor can opt into Internet research and cited recommendations; it cannot silently change files or download models.

**Jan Desktop:** planned optional Rust **TIHULU MCP Server** exposing only permissioned TIHULU app tools for Jan chat. **Jan Agent:** optional, separate skill/plugin wrapper around the same integration. Real Jan tools will exist only when corresponding TIHULU app adapters are implemented and authorized. No Jan configuration or model files have been changed.

- [Universal Model Discovery & Matching](docs/MODEL_DISCOVERY_AND_MATCHING.md)
- [Jan Desktop MCP and Jan Agent Plugin Proposal](docs/JAN_INTEGRATION.md)
- [Model reference / aipath contract](docs/AIPATH_CONTRACT.md)
- [Updated interactive HTML concept](ui/index.html)

## v0.7: Jan one-click guided setup

**Confirmed requirement:** An in-app **How to Connect Jan** guide and **Set Up Jan Integration** button in **External Apps → Jan**. The future Rust wizard will detect Jan, review permissions, register the TIHULU MCP bridge through supported Jan MCP settings/CLI, verify the connection and allow a reversible disconnect. If automation is unsupported, it will show manual instructions. The updated HTML prototype provides an interactive **simulation only**; no Jan files or settings are altered.

- [Jan Setup Wizard product/design guide](docs/JAN_SETUP_WIZARD.md)
- [Existing optional Jan MCP and plugin architecture](docs/JAN_INTEGRATION.md)

## Planning workflow

1. Record every approved decision in the Markdown plan / decision log.
2. Keep architecture diagrams, UI examples and acceptance tests aligned with the plan.
3. Distinguish **confirmed requirements**, **proposals** and **experimental concepts**.
4. Make **documentation-only** changes while the six-application preservation/split work is ongoing.
5. After that work is completed, design scoped integration requests per app; only then consider implementation.

This repository is the intended source of truth for TIHULU AI Tools planning going forward.
