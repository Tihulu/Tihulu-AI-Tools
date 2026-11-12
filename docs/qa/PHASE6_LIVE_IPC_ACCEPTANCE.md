# Phase 6 — Live WebView↔Rust IPC, disposable Ubuntu only

**Status: EXPERIMENTAL, review-only Draft PR. RELEASE HOLD.**

Source baseline: Phase 5 GO `48669240e4595908363f48f4229e70f55e72d7db`.
Inherited Phase 6 GUI QA: [Draft PR #33](https://github.com/Tihulu/Tihulu-AI-Tools/pull/33),
`3684716f95a5cc411cb7999b94e5600f6cb94bef`.

This branch adds only a new workflow and test helpers; **none of the frozen Rust
or shipped HTML/JS sources or capability/CSP configuration are changed in Git**.

## Approach

In GitHub's disposable Ubuntu runner, compare the pristine frontend HTML byte
by byte with Phase 5 frozen Git. *Only in that temporary checkout*, copy the
strict probe module into the frontend and add a new local JS module script tag.
No privilege/capability/CSP changes and no new Rust commands.

Build the native Tauri demo. Under private Xvfb, Openbox, D-Bus and HOME/XDG,
the probe calls the **real** global Tauri bridge's two unchanged read-only
commands, then strictly validates every field in both returned DTOs. Any
unexpected value, missing command, fake hardware or cleanup claim yields FAIL.
When both calls succeed, the JS probe updates `document.title` with a
test-only PASS marker. An external X11 observer requires that marker on the
visible native process window and a clean native close; otherwise HOLD.

Native window title propagation may vary by WebKitGTK/Tauri version. A failure
is a genuine HOLD on this *observation method*, not proof that IPC is broken.
Do not bypass a failed probe, mock a real result or change native capabilities
to force a green CI.

## Decision matrix

- Phase 5 native/mock source: GO, unchanged.
- Ubuntu native window startup/quit: GO from PR #33.
- Live two-command WebView↔Rust IPC: PENDING exact-head CI, only a limited
  Linux sandbox probe if green.
- Debian, Pop!_OS COSMIC, Windows, macOS, Pi native live IPC: HOLD.
- Real transactional installer/rollback/uninstall: HOLD.
- Real model discovery/inference, Jan/Ollama, GPU Host/mTLS/NPU: HOLD.
- Media/Flight/other TIHULU installed app preservation: Untouched, no owner
  machine actions authorized.
- Release: HOLD.

The CI may install **OS build dependencies in GitHub's disposable runner only**.
It must not install this app on any owner device, handle models, readsb, tar1090
or physical hardware, publish installers or merge protected branches.
