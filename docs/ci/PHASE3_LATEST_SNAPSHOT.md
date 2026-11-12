# Phase 3 — Combined snapshot 2026-11-12

Coordinator-owned CI sandbox only. Agent branches and main are not merged or changed. Each row points to the exact code snapshot copied into this branch:

| Agent branch | Source SHA |
| --- | --- |
| `agent/01-platform` | `350222c632ec8e9ce32a62a1b8295348c51d974a` |
| `agent/02-discovery` | `3e00253bf18201ccd9ad4e20e884e3cbbf5876a5` |
| `agent/03-market` | `12d7bc727285906a6846707fa6aca412b510c519` |
| `agent/04-compute` | `d397b50e39e086e0d631f1b9bd0f75b5a32e84f2` |
| `agent/05-integrations` | `a0ab494c9daad0895bb625bef2a51c5f0f2fb76e` |
| `agent/06-ui` | `ad9bf06e2e26f9cc802db5f4748e7d3e2cc75703` |

- The copied standalone Rust manifests have their own nested `[workspace]` markers removed only in this sandbox so the root workspace includes all four independent crates.
- Tauri 2 native build is excluded from root workspace and is not accepted as a production binary.
- This replaces older source snapshots in the sandbox and may invalidate older fixes; new CI must determine the actual state.
- This is mock-only code; passing CI is not evidence of real GPU memory release, Jan Desktop connectivity, TLS, SDR hardware, Pi/Hailo or complete UI/backend wiring.
