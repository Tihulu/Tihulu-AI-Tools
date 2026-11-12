# COORD-CI-03 — externally supervised fixed-source QA experiment

**NEW isolated Draft candidate; release and full adversarial security HOLD.**

This addition responds to independent [AI-08 PR #45 P1-03/P1-04 findings](https://github.com/Tihulu/Tihulu-AI-Tools/pull/45#issuecomment-6839299527).
The parent is a separate Python/OS process from the test worker; the only permitted
test source comes from the four **fixed, previously independently inspected** heads:

| Agent input | Exact commit | Tests |
| --- | --- | ---: |
| AI-09 preflight #39 | `ea6667b72d01fadf222c1c50363e4df7a2a3b7f3` | 10 |
| AI-09 fake-version transaction #43 | `c11d2bb38f04d0ac4c03529a70284c136e58c254` | 18 |
| AI-10 routing mock #40 | `b28a4b1b1869289fd77392f8e1985f9b3b00f3b5` | 15 |
| AI-10 session mock #42 | `c817eed1a1088fa642102fefce6cc57b6ac981a6` | 34 |

**Frozen baseline**: COORD-CI-02 [#45](https://github.com/Tihulu/Tihulu-AI-Tools/pull/45)
`61708417c7573e3dbacdcd61e947b76f69214a74`, itself based on
#44 `127b5331236cc9d5adb37122d8ab52f7492203c5` and #34
`283edc8a1101f63e08f12c172d6b2d6688b9a5ca`.
The current branch adds only **four new QA, documentation and workflow files**.
No original source, existing CI workflow, other agent branch, main or
owner installation is changed.

## What the new CI really tests

1. On a fresh GitHub-hosted Ubuntu runner, the supervisor checks that **all of
   #45's previous Git blobs and file modes are unchanged** and that only the four
   enumerated coordinator-owned source paths are added.
2. A trusted external parent extracts exactly two previously inspected Python
   files from a pinned Git blob SHA into a private `0700` lab as `0600` files.
   It verifies the Git blob identity, computes independent pre-execution SHA-256,
   and statically inventories exact `test_*` method names via Python AST.
3. It then runs the imported tests in a **separate interpreter subprocess**,
   enforcing finite timeout, exit code, a unique final completion receipt and a
   matching test-method inventory. The external parent *continues running*
   after its test child exits, and independently validates file hashes and
   unexpected workspace artifacts.
4. Each of the four suites runs on its **own GitHub-hosted job** without a
   shared runner, cache or artifact. An additional job reproduces previously
   reported simple `os._exit(0)`, pre/post file modification, direct
   `unittest.TestSuite`/runner monkeypatch, duplicate/fake receipt, symlink
   and world-writable runner-root negative cases.
5. No fake `Ran N tests` stderr is used as an acceptance predicate.

## What it does NOT prove — important

This is a **narrow remedial test of the reported failure modes on fixed,
reviewed test fixtures**, not a Python security sandbox. A child executed
under the same OS UID still has access to other writable same-user paths;
the child-generated receipt is not cryptographically authenticated, and a
sophisticated malicious module may forge case names/counts, dynamically
evade static AST checks or modify other same-UID paths. A fake environment
variable `RUNNER_ENVIRONMENT=github-hosted` does not authenticate the host.

To securely accept arbitrary malicious Python input, a different architecture
with OS privilege separation / a disposable VM or equivalent sandbox and
independent instrumentation would be required. **Do not mark P1 hostile Python
execution immunity PASS on this branch.** Retain a security HOLD for such a
threat model even if the exact-source CI check succeeds.

**Historical CI policy remains separate:** original Phase 6 #34 and CI-01 #44,
CI-02 #45 narrow path allowlist checks cannot simply be made green on a
new descendant with additional paths. Their actual red runs must not be
suppressed, weakened or treated as implicitly passed. A separate owner-reviewed,
versioned required-check governance decision is necessary for merger.

## Still HOLD

No authentic installed native package/launcher/rollback/uninstall,
user-host or legacy-app coexistence, real model/CPU inference or Jan/Ollama
authorization, GPU/CUDA/Hailo/Metal, SDR, Maps, Windows/macOS/Pi, release
licensing, data migration, deployment or publishing is performed.
Do **not** merge, release or contact user devices/borrowed providers.
Request AI-08 independent exact-HEAD adversarial re-review and publish
P0/P1/P2, actual CI run IDs and evidence on this PR, [issue #41](https://github.com/Tihulu/Tihulu-AI-Tools/issues/41),
[coordinator #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15)
and [master #8](https://github.com/Tihulu/tihulu-survival-kit-plan/issues/8).
