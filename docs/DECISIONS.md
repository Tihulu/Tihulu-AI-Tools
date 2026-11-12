# Decision register and project gates

**Baseline:** v0.6, 2026-11-12. This is a planning repository, not an implementation repository yet.

## Confirmed product requirements

| Topic | Confirmed direction |
| --- | --- |
| Independent product | TIHULU AI Tools with Rust-first core and cross-platform model management |
| Platforms | Linux x86-64, Raspberry Pi 5 ARM64/8 GB, Pi 5 + AI HAT+ 2; additional OS targets later |
| Default model root | `~/TIHULU AI Tools/Models/`, overridable, separate from source code |
| Existing models | Detect Jan, Ollama and other common model locations with consent; no duplicate downloads where compatible |
| Model operations | Market, custom imports, versioning, validated reuse, safe relocation and rollback |
| Application independence | Existing applications preserve standalone execution, configuration and legacy fallback |
| Client configuration | Per-app `aipath.txt` compatibility bridge; no AI Tools GUI needed at app startup |
| Compute | On-demand CPU/GPU/NPU, explicit secure host pairing on LAN/Ethernet; client/device-stored model execution when compatible |
| Integration | Model/task assignments for the six TIHULU app families and explicit external app access |
| UI | Auto Detect, Regular Computer, Performance / Experimental, Raspberry Pi 5 — 8 GB, Raspberry Pi 5 + AI HAT+ 2, Custom |
| Experimental assistants | AI Advisor for planning/model choice; separately permissioned App Control Agent |
| AI Tools own remote inference | The Pi's Advisor, Agent and compatible features may use the paired PC GPU host |
| Shared host assignment | The same paired host can be granted per TIHULU app and AI task, without a second model installation on Pi |
| Chat UX | Advisor and Agent use chat timelines/composers with active model, runtime host and permission states |
| External AI app inventory | Search by app name, installation/data folder, custom model path or approved local API; scan read-only by consent |
| No-duplication reuse | Recognize compatible externally managed Jan/other models as installed references without moving or downloading them |
| Task-level matching | Suggest and bind existing models to TIHULU app AI tasks using verified task/runtime/hardware compatibility and approved adapter |
| Safe paths | Link in place by default; optional staged copy/relocation/consumer repoint with backups and rollback |
| AI-assisted matching (experimental) | Internet research OFF by default; cited candidate recommendations; apply only after deterministic Rust-side checks and explicit approval |
| Jan optional integration | Jan Desktop via optional TIHULU Rust MCP server; Jan Agent plugin candidate is separate from Jan Desktop MCP connector |
| Policy | Default offline/on-demand; no hidden model downloads, service takeover, or user-data movement; UI EN/TR/DE |

## Proposed design — not final or proven

| Topic | Current candidate | Validation needed |
| --- | --- | --- |
| Desktop UI | Tauri 2 | Linux and Pi packages, WebKitGTK behavior, native installers |
| Headless/browser host | Rust Axum | Authentication, TLS, service lifecycle and firewall rules |
| Registry | SQLite + model hashes + adapter metadata | Schema, migration, version pinning, rollback |
| Remote protocol | HTTPS + OpenAI-compatible supported subset | Auth, device pairing, feature parity and quotas |
| Secrets | OS credential vault | Per-platform integration |
| Model runtime | Native adapters for llama.cpp, Ollama, Whisper, Argos, Hailo, etc. | Per-task runtime matrix and acceptance tests |
| Native mobile compute | Experimental future option | Platform hardware APIs, power, security, packaging |
| GPU resource pooling | Queue, budgets, verified reuse | Performance and isolation tests |

## Non-negotiable technical constraints

1. ARM64 compatibility **does not** imply Hailo NPU runtime compatibility.
2. Pi AI HAT+ 2's separate memory is not NVIDIA-compatible VRAM.
3. Jan/Ollama-managed model stores remain owned by their apps until explicit approved migration.
4. Sharing a model **file** is separate from sharing a live **inference process**.
5. A phone browser alone does not offer the phone GPU as a remote compute endpoint.
6. No remote model server is made publicly reachable without secure pairing, authentication and network policy.
7. The AI Tools GUI is never a mandatory dependency for launching Media, Text, Satellite, Flight Radar, Geolocation or Survival Kit.
8. `aipath.txt` must contain no API keys, bearer tokens or secrets.
9. A remote LLM's text is **not** permission to execute tools. Every action stays behind Rust-side scoped authorization.
10. AI Tools GUI and remote host service have independent lifecycles; app launch does not require GUI, while remote inference requires a reachable service.
11. External app model registries remain foreign-owned and are accessed read-only by default; a model's path is never changed merely because it is registered.
12. LLM-generated app matches must pass deterministic compatibility, tool/app adapter checks, diff review, and user permission before config writes.
13. Jan MCP tools have bounded capabilities and per-action user approval; AI Tools must not change Jan's global auto-approval setting.

## Rollout gates

| Gate | Required outcome |
| --- | --- |
| G0 — Current | Preserve requirements and diagrams in this repo; **no product or third-party app source modifications** |
| G1 | Six-app repo split and preservation audits completed and accepted |
| G2 | Agree on model registry schema, app path contract, per-task Rust compute routing, host pairing, runtime interfaces and security model |
| G3 | Rust core model inventory with test fixtures; no destructive migrations |
| G4 | Market, hardware compatibility and opt-in GPU host tested on Linux x86-64 and Pi ARM64 |
| G5 | Per-app integration requests and reversible migrations |
| G6 | Experimental assistants with scoped capabilities and approval audit |

All work on this repository remains **planning-only** until explicitly promoted beyond G0. Any new choice should be added to this document and the [full plan](TIHULU_AI_TOOLS_PLAN.md), tagged **confirmed / proposed / experimental**.
