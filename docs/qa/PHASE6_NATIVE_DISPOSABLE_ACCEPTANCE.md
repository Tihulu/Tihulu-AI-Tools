# Phase 6 — first real native desktop observation in disposable CI

**Status: EXPERIMENTAL; review-only Draft PR. RELEASE HOLD.**

## Immutable input and isolation

The exact Phase 5 SOURCE/MOCK GO baseline is Draft PR #32,
`48669240e4595908363f48f4229e70f55e72d7db`. This QA branch is a
descendant of that commit and may add **only** this document, one GitHub Actions
workflow, one CI-only Python runner and its new negative tests. The workflow
enforces that path allowlist against the immutable Phase 5 baseline.

Never merge, rebase, overwrite or modify the frozen PR #32, any owner-local
installation or another TIHULU repository. Existing Media Center, Flight Radar,
readsb, tar1090, Jan and Ollama are out of test scope and are not controlled.

## What this independent new gate actually executes

1. Validate exact frozen-source ancestry and absence of any modified source,
   Rust crate, Tauri config, existing QA assertion or old workflow.
2. Run the existing browser mock regression suite and new no-owner-machine,
   traversal, symlink and window-ID refusal tests.
3. On disposable GitHub Ubuntu 24.04 x64, run the existing *actual* native
   Tauri unit/IPC contract tests and compile a native Tauri 2 demo executable.
4. Copy the compiled binary to a strictly private versioned test directory
   inside `RUNNER_TEMP` (NOT an OS-installed package or user global PATH).
5. Under private Xvfb, D-Bus session and throwaway HOME/XDG roots, launch that
   actual binary. Observe a visible X11 window with the TIHULU AI Tools title,
   request a normal window close and require its process to exit.
6. If the GUI fails to open/exit or CI cannot bind the executable/path safely,
   mark this gate **HOLD**, not PASS. Cleanup may terminate only the exact
   subprocess started by this disposable test; no service or process-name kills.
7. Emit a machine-readable report that explicitly marks native IPC live
   observation, installer, inference, GPU/NPU, models and owner machine as
   `false`. The existing Rust tests prove command contracts, **not** a real
   WebView↔IPC request/response.

No executable installation, system service configuration, SDR ownership,
GPU/NPU invocation, Jan/Ollama connection, model downloads, actual model
scanning or provider lifecycle occurs.

## Decision boundaries

| Gate | Evidence when green | Remaining verdict |
| --- | --- | --- |
| Phase 5 frozen/source/mock | Independent GO at pinned SHA | GO |
| Disposable Ubuntu Tauri native window / Quit | Only if new CI window process visibly opens/exits | Pending CI |
| Live WebView↔Rust IPC over running UI | No WebView automation/probe in this workflow | HOLD |
| Installer, repeated install, rollback, uninstall | No actual signed/packaged installer | HOLD |
| Other OS native startup and installed acceptance | No Windows/macOS/Pi native GUI launch here | HOLD |
| Approved real model inventory/inference | Intentionally absent | HOLD |
| Jan/Ollama, GPU Host, mTLS and physical cleanup | Intentionally absent | HOLD |
| Original legacy backups, side-by-side on owner PC | Not accessed or attested | HOLD |
| Release | All real gates and independent signoff needed | HOLD |

## Next safe order

After this CI and an independent review: build a **separate** real
WebView↔Tauri IPC observation fixture without adding privileged commands;
design an ownership-hardened versioned user-level installer and negative
rollback tests in an ephemeral HOME; then obtain owner-specific consent for
later real model/provider/hardware tests. Never treat synthetic ResourceLedger
cleanup receipts as proof of actual VRAM/RAM release. No release promotion or
real owner-machine installation is authorized by this PR.
