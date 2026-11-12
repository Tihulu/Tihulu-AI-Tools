# COORD-CI-02 — Answer to independent AI-08 P1 findings (QA only)

**Status:** new isolated Draft PR candidate. **Not production/security/release approval.**
This does NOT change the frozen source, [Phase 6 #34](https://github.com/Tihulu/Tihulu-AI-Tools/pull/34),
original [COORD-CI-01 #44](https://github.com/Tihulu/Tihulu-AI-Tools/pull/44) or the four agent
PRs. Based on #44 exact `127b5331236cc9d5adb37122d8ab52f7492203c5`.

## Independent attack report

[AI-08's independent PR #44 audit](https://github.com/Tihulu/Tihulu-AI-Tools/pull/44#issuecomment-6838624947)
found:
- **P1-01:** CI-01 counted the FIRST stderr match `Ran N tests`, which fixture code
  could forge despite only one actual unittest executed.
- **P1-02:** CI-01 extracted multiple groups as writable files under the same UID,
  enabling a prior group's code to modify a later group's test before execution.
- **P2-01:** Arbitrary absolute `RUNNER_TEMP` path with manually set CI variables
  could pass the old validator; P2-02 required new verifier SHA-specific review.

## New isolated QA approach

1. Hard pin frozen #34, prior #44, and all agent #39/#43/#40/#42 commit SHAs.
   Compare complete recursive Git trees: all inherited object SHA/mode/type unchanged,
   exactly four added 100644 files per PR, exactly four new files in this candidate.
2. A dedicated source/negative-test job executes fail-closed experiments with
   forged `Ran 28 tests` and `Ran 49 tests`, arbitrary root paths, symlinks,
   hardlinks, permission drift and changed file hash. **No stderr count parsing**:
   test discovery count and `unittest.TestResult.testsRun` must both match expected.
3. The four fixture groups execute as **four separate GitHub-hosted Ubuntu 24.04
   jobs**, with no files, caches, artifacts, or worktrees shared between those jobs.
   Each job fetches fixed Git commits and extracts **only its own two Python source
   blobs** into a newly private `0700` temporary lab, with `0600` files.
   Check exact Git blob SHA-1, local file SHA-256 before/after executing that one
   suite and ensure the lab has no untracked artifacts.
4. Expected actual unittest cases: AI09-preflight **10**; AI09-transaction **18**;
   AI10-policy **15**; AI10-sessions **34** = **77 tests across four independent
   runner jobs**. There are additional attack tests in a fifth separate job.
5. Hosted runner gate requires GitHub CI markers, runner environment
   `github-hosted`, an owner-controlled non-root temp location, no symlink
   ancestors, no arbitrary `/tmp` or `/var/tmp` root and no checkout-as-temp.

**Limits:** These are fixed, publicly reviewed synthetic Python fixtures. Running
arbitrary untrusted Python as the same UID is **not** an authenticated security
sandbox; a malicious test may modify its own runner or forge some in-process
claims. Separate GitHub-hosted jobs prevent the specific reproduced *cross-group*
shared-lab mutation; they do not make arbitrary code safe to execute. No original
Phase 6 #34 live IPC is rerun here; its exact-head success remains limited
to #34 only. The historical #34 changed-path check stays immutable and
its descendant failures still require **owner-reviewed, versioned required-check
governance**, not a silent waiver or a new green replacement.

## HOLD gates

- Independent AI-08 re-review of this exact new SHA, including adversarial
  count and cross-runner isolation evidence, before scoped security acceptance.
- Real native Ubuntu installed package/launcher/rollback, Windows/macOS/Pi,
  actual consented model inference, Jan/Ollama/OpenCode, physical CUDA/Hailo,
  source licensing, map downloads and cross-app Stable First: **NOT RUN**.
- No merge, release, user-machine installation, original TIHULU apps or
  receiver/services/models/providers touched.

Publish new SHA, real GitHub CI jobs and P0/P1/P2 to [coordinator #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15),
[issue #41](https://github.com/Tihulu/Tihulu-AI-Tools/issues/41),
and [master #8](https://github.com/Tihulu/tihulu-survival-kit-plan/issues/8).
