# AI-09 — disposable synthetic transaction and rollback QA (NO installer release)

**Stream:** [issue #37](https://github.com/Tihulu/Tihulu-AI-Tools/issues/37); base **exact** Phase 6 live-IPC [Draft PR #34](https://github.com/Tihulu/Tihulu-AI-Tools/pull/34) `283edc8a1101f63e08f12c172d6b2d6688b9a5ca`. Independent prior preflight [Draft PR #39](https://github.com/Tihulu/Tihulu-AI-Tools/pull/39) `ea6667b72d01fadf222c1c50363e4df7a2a3b7f3` was reviewed, NOT edited or silently incorporated. Frozen Phase 5 and Phase 6 native sources, installed apps and earlier PRs are untouched.

## Identity and write contract

- `app_id=tihulu-ai-tools`, fixed test-only manifest schema 1. The `source_commit` field records **only the immutable baseline under comparison**, not evidence that a native binary was built from it; the file is an obvious `TIHULU_AI09_SYNTHETIC` fake shell fixture.
- All test writes are bounded to a new direct child `RUNNER_TEMP/ai09tx-*` under a private runner-owned parent/root; the lab root must be `0700`. Root, version, payload, lock, manifest and pointer opens use descriptor-relative `O_NOFOLLOW` checks. The code requires `CI=true`, `GITHUB_ACTIONS=true`, matching `RUNNER_TEMP` and expected directory identity. Such environment flags are **test guardrails**, not authentication against a malicious same-UID process.
- Versioned fake copies live only in `versions/vN/` with a SHA-256-verified `fake-executable` and manifest. `current` is an activation pointer written using temp-file fsync, `os.replace()` and directory fsync. Journal is written before activation and archived on successful completion or controlled recovery. No installed version is deleted.
- Nonblocking `flock` excludes cooperative overlapping transactions; unexpected foreign root files, staged files, symlinks, corrupted manifests and invalid journal records cause HOLD, rather than automatic cleanup.
- Executable/package locations are separated from app model roots and map packs as required by [APP-PATH-01 #18](https://github.com/Tihulu/Tihulu-AI-Tools/issues/18). No real binary, app launcher, package manager, model/map store, GPU, SDR, Jan/Ollama/OpenCode process or external provider is operated.

## Reproducible bounded tests (Ubuntu 24.04 disposable runner only)

    python3 -m compileall -q qa/phase6/installer/ai09_transaction_lab.py qa/phase6/installer/test_ai09_transaction_lab.py
    PYTHONPATH=qa/phase6/installer python3 -m unittest discover -s qa/phase6/installer -p 'test_ai09_transaction_lab.py' -v

The additive workflow `.github/workflows/phase6-ai09-transaction-lab.yml` asserts the frozen exact SHA as ancestor and a **four-path changed-file allowlist** before executing tests. It redirects HOME, XDG roots and TMPDIR to ephemeral runner storage. No sudo, OS package-manager actions, hardware access or release artifacts.

**18 synthetic tests:** initial fake version and SHA, idempotent reinstall, v1→v2→v1 rollback preserving both sets of bytes, logical deactivation/reactivation (**not full uninstall**), duplicate version hash mismatch, foreign-file sentinel preservation (root and version namespace), symlink escapes, corruption, runner/temp/mode refusal, cooperative lock collision, lock substitution refusal, **real SIGKILL of an explicitly spawned fake test child** before and after pointer activation, recovery to correct old/new state, unrecovered transaction hold, forged journal/marker/current refusal and invalid versions/payloads. No arbitrary process PID is discovered or terminated.

## Security acceptance matrix

| Gate | Evidence | State |
| --- | --- | --- |
| Fake binary idempotence, upgrade/rollback, old-version hash preservation | Deterministic synthetic CI file operations | **Scoped GO only if exact-head CI PASS** |
| Two precise SIGKILL crash points | Kills only dedicated synthetic child; recovers validated old or new pointer; never removes version files | **Scoped GO only if exact-head CI PASS** |
| Foreign files, fake symlinks, corrupt manifest/journal, cooperating lock conflict | Refuse and preserve sentinels in synthetic tests | **Scoped GO only if exact-head CI PASS** |
| Actual native installer, signed provenance, installed launcher, full uninstall/reinstall, independent WebView↔Rust IPC from installed path | No native packaged application installed; `source_commit` field and fake executable are not cryptographic package provenance | **HOLD** |
| Hostile same-UID TOCTOU, mutable ancestor path and lock-inode replacement, journal archive/unlink substitution | Python preflights + `flock` cannot protect every rename race; journal cleanup includes a non-atomic same-UID substitution window | **P0/P1 HOLD** |
| Crash during payload staging, after journal archive before unlink, disk full, real power loss and portable directory fsync | Not tested or safely resolved | **HOLD** |
| Windows x64, macOS ARM64, Raspberry Pi OS ARM64 installed acceptance | Planned only; no native installed jobs | **HOLD** |
| Existing owner apps, configs, Jan/OpenCode, model/map stores, borrowed services and SDR | Never touched in disposable tests; real-host preservation remains unproven | **HOLD for real hosts** |
| Aggregate inherited #34 workflow / independent security review / release | Original strict workflow will reject any newly added workflow file; it must NOT be weakened | **MERGE/RELEASE HOLD** |

**Next:** Independent hostile same-UID QA, hardened production installer design with verifiable package owner/provenance and robust journal recovery, full uninstall/foreign-file matrix, actual installed native launch/window/IPC on disposable Ubuntu, then platform-specific native installed checks with job receipts. Frozen inherited QA requires a **separate coordinator-owned composite branch/workflow** that preserves its original strict assertions. Never modify #34, #39, main or any existing installation.

**No merge, tag, signed distribution, release, user-machine access, provider/model operation, GPU/SDR or other TIHULU repo changes.** This reports fixture behavior, not verified production install or overall release readiness.
