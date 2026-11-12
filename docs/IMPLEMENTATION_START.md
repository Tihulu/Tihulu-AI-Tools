# Independent Rust Foundation — Initial Development Phase

**Status:** Implementation started on `feat/rust-foundation`, November 12, 2026, explicitly approved by the user. The parent v0.7 Markdown plans remain requirements; this branch contains the first code. **No other TIHULU repositories may be modified.**

## What exists in this initial branch

- Rust 2021 workspace with `tihulu-ai-core` and `tihulu-ai-cli`.
- Conservative, **consent-gated read-only inventory** of explicitly selected directories for candidate GGUF, ONNX, SafeTensors, Whisper GGML, TFLite and Hailo HEF files.
- Six user-selectable hardware profiles; real OS and CPU architecture detected using Rust stdlib; GPU, memory, NPU and model readiness remain **unverified**.
- Pure local/remote candidate routing logic with per-runtime readiness checks; no network or GPU inference.
- Sample CLI: `cargo run -p tihulu-ai-cli -- profile` or `cargo run -p tihulu-ai-cli -- scan "/path/to/approved/models" --allow-read`.
- Unit tests and an OS/architecture CI matrix.

**Not implemented:** Jan/Ollama APIs, actual Market, model download/update, model binary parsing, registry persistence, GPU server, Tauri desktop app, Pi Hailo adapter, Jan MCP bridge, installers, encrypted pairing, app `aipath.txt` writing, AI Advisor/Agent inference.

## Cross-platform targets and evidence boundary

| User target | Architecture | Baseline CI runner | Additional future acceptance |
| --- | --- | --- | --- |
| Debian/Ubuntu/Pop!_OS desktop | Linux x86_64 | `ubuntu-24.04` | Native Debian/Pop!_OS packages, Tauri/WebKitGTK, NVIDIA GPU |
| Debian ARM / Raspberry Pi 5 (8 GB) | Linux aarch64 | `ubuntu-24.04-arm` | Native Raspberry Pi OS and resource-limited tests |
| Raspberry Pi 5 + AI HAT+ 2 | Linux aarch64 + Hailo-10H | `ubuntu-24.04-arm` (CPU only) | Real HailoRT and approved artifact inference, PCIe, NPU and memory |
| macOS M-series | Apple Silicon aarch64 | `macos-15` | Signed/universal/native installer, Metal or Core ML runtime evaluation |
| Windows 64-bit | Windows x86_64 | `windows-2025` | Native installer, path conventions, GPU driver tests |
| Windows ARM64 | Windows aarch64 | `windows-11-arm` | ARM64-native installer, available runtime compatibility |

CI runner availability follows [GitHub runner documentation](https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job). **Passing these runners proves only this Rust workspace builds/tests there.** It does not demonstrate a Raspberry Pi hardware fix or Hailo/CUDA/Metal inference.

Raspberry Pi AI HAT+ 2 has a **Hailo-10H (40 TOPS INT4, separate 8 GB memory)** and requires specific [Raspberry Pi software/runtime](https://www.raspberrypi.com/documentation/accessories/ai-hat-plus.html). It must not be treated as NVIDIA-compatible GPU.

## Security of this prototype

1. No model data, Jan data, logs, chat histories or secrets are read implicitly.
2. A scan requires both a path and `--allow-read`; it never leaves that scope via interior symlinks, never executes models and never writes to the scanned directory.
3. The inventory returns unverified **model file candidates** only; compatible engine/model/task/hardware verification is separate future work.
4. Bounded scan depth, total entry count and candidate count are applied.
5. The Rust core has **no network dependency** or listener; remote route selection has no side effects.
6. Existing app repositories, owner-managed model paths and background services are untouched.

## Governance and six agent workstreams

The `AGENTS.md` policy on this branch permits **independent AI Tools implementation only**. Six planned agent assignments are described in [AGENT_WORKSTREAMS.md](AGENT_WORKSTREAMS.md). Each task gets its own branch and narrowly scoped source area. Workstreams may propose adapters or APIs, but must not silently change the other TIHULU applications, existing Jan installations or live services.

## Validation status

- Source files committed to GitHub; Rust toolchain **not installed in the current authoring container**, so no local `cargo test` can be claimed.
- **Verified:** [GitHub Actions run #7](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43148925278) completed **successfully** on 2026-11-12: 5/5 native runner jobs passed (Linux x86_64, Linux aarch64, macOS Apple Silicon, Windows x86_64 and Windows aarch64), including `cargo fmt --check`, `cargo test --workspace --all-targets` and `cargo clippy -- -D warnings`. Earlier formatting-only failures were corrected, not ignored.
- No physical Raspberry Pi / AI HAT+ 2, macOS M-series or Windows hardware runs have been conducted.
