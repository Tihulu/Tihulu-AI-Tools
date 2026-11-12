# AI-09 initial isolated installer-lab preflight — **NOT installed acceptance**

**Scope:** Phase 6 AI Tools [issue #37](https://github.com/Tihulu/Tihulu-AI-Tools/issues/37). Baseline: [Phase 6 PR #34](https://github.com/Tihulu/Tihulu-AI-Tools/pull/34) at `283edc8a1101f63e08f12c172d6b2d6688b9a5ca`, stacked on earlier frozen #33 and #32. All those PRs remain unmodified. This new PR only adds a CI-only, non-mutating *preflight*, synthetic negative tests and independent workflow. It **does not build, install, update, uninstall or rollback** AI Tools.

## Why an initial preflight

Before any future disposable installation transaction, test machinery must refuse to operate outside a designated new private `RUNNER_TEMP/ai09-...` child. The preflight requires explicit GitHub CI environment flags, absolute scoped paths, private `0700` runner-owned lab, safe parent ownership/mode, Linux `O_NOFOLLOW|O_DIRECTORY` pinned descriptor, matching inode/device during inspection and no existing contents. It never removes a foreign file. Merely satisfying those conditions does **not** authorize a later destructive operation.

## Synthetic tests

- New private directory succeeds.
- Non-CI or forged environment flags refused.
- Relative path and outside-runner path refused.
- Symlink root refused.
- Foreign sentinel file preserved and refused.
- Non-private directory permissions refused.
- Root replacement between *separate* calls demonstrates the limitation: fresh preflight cannot authenticate past transaction state.
- Unexpected namespace refused.
- RUNNER_TEMP symlink alias refused.
- Foreign subdirectory preserved and refused.

CI must pin inherited source `283edc8a1101f63e08f12c172d6b2d6688b9a5ca` as ancestor and reject any unexpected changed paths (only this namespaced fixture, doc and workflow allowed).

## Known security limitations / mandatory HOLD

- No actual installer, production transaction journal, signed package, package-manager integration or real launcher/IPC test is here.
- Same-UID concurrent adversary can rename parent paths or race after any preflight. Future implementation must use persistent dirfd/inode-pinned operations, exclusive locks and verified commit/recovery state; no path-based multi-operation sequence is proven atomic by this test.
- CI `RUNNER_TEMP` and `GITHUB_ACTIONS` checks are safeguards for the test harness, **not** a security boundary against hostile same-UID processes.
- No real user machine, files, installed apps, SDR, models, Jan/Ollama/OpenCode, downloaded maps, hardware, licenses or secrets touched.
- `GO` can apply only to the narrow disposable preflight tests after exact-head CI. Native installer, rollback, adversarial race security, installed GUI/IPC, actual models/GPU and **release remain HOLD**.

**Next AI-09 agent acceptance:** build versioned user-level installer **only in a disposable CI host**, test start/stop/restore, foreign files, intentionally killed processes, true launcher/IPC, repeat installs and independent security review. Keep current stable apps and Phase 5/6 source frozen. Report exact SHA and per-gate PASS/HOLD in own Draft PR, AI coordinator issue #15, master issue #8. No merge or release.
