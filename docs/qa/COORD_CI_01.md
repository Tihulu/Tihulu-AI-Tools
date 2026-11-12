# COORD-CI-01 — Immutable cross-PR source/fixture attestation (QA ONLY)

Status: **NEW ISOLATED EXPERIMENT**. This branch is a coordinator-owned additive QA proposal for
[Issue #41](https://github.com/Tihulu/Tihulu-AI-Tools/issues/41).
**No combined GO, no installer/real inference/release approval.**

## Frozen inputs

- Phase 6 live read-only IPC: [Draft #34](https://github.com/Tihulu/Tihulu-AI-Tools/pull/34)
  at `283edc8a1101f63e08f12c172d6b2d6688b9a5ca`.
  The original exact-head run [43287486919](https://github.com/Tihulu/Tihulu-AI-Tools/actions/runs/43287486919)
  remains immutable **scoped PASS**. This PR neither changes nor reinterprets that workflow.
- AI-09 isolated preflight: [Draft #39](https://github.com/Tihulu/Tihulu-AI-Tools/pull/39)
  `ea6667b72d01fadf222c1c50363e4df7a2a3b7f3`.
- AI-10 policy oracle: [Draft #40](https://github.com/Tihulu/Tihulu-AI-Tools/pull/40)
  `b28a4b1b1869289fd77392f8e1985f9b3b00f3b5`.
- AI-10 synthetic provider session QA: [Draft #42](https://github.com/Tihulu/Tihulu-AI-Tools/pull/42)
  `c817eed1a1088fa642102fefce6cc57b6ac981a6`.

Each SHA is hard-pinned. The attestor compares **full recursive Git trees**, checking that every
#34 inherited path preserves original blob SHA, object type and file mode, and that each
additive PR contributes *exactly* its own four new regular (100644) files. Any prior-file
change/removal, unknown addition, altered mode/symlink or cross-agent path overlap fails closed.
A separate coordinator base check accepts only this PR's four own new QA/docs/workflow paths.

## Synthetic execution only

The workflow runs 15 contract tests for the scope checker, then extracts **only the
pinned Python fixture blobs** (never the workflow files) into a newly allocated private
`RUNNER_TEMP` directory. Python unittest discovers and executes:

- AI-09 installer-preflight synthetic suite — expected 10 tests.
- AI-10 route-policy + provider-session suites — expected 15 + 34 = 49 tests.
- Expected aggregate: **59/59 synthetic**. Failure to execute or a different test count is HOLD.

No executable from the owner's machine, no model inference, no real native installer, no
Jan/Ollama/OpenCode endpoint, no SDR, no GPU, no map download and no automatic background
service is touched. Workflow executes in a disposable GitHub Ubuntu runner only.

## Independent CI governance is still missing

The original #34 workflow's strict changed-path guard was purposely built as a *single-PR*
proof; descendant #39/#40/#42 workflow runs fail on new CI YAML paths **before** actual
IPC tests. New green coordination CI cannot override a required red status or certify
the inherited original #34 test as running on a new production/source composition.
Do **not** alter, disable, weaken, bypass, or rename the frozen original workflow.

This new CI verifies source invariance and synthetic fixture coexistence independently
at fixed snapshots; it does **not** solve the general GitHub required-check governance
problem or mean combined production application code is integrated. An independent
review and owner-approved future versioned check policy are mandatory before any merge.

## Acceptance boundary

- Original Phase 5 source/mock — independently accepted at its exact commit only.
- Original Phase 6 Ubuntu instrumented IPC — exact #34 SHA scoped PASS, unchanged.
- Pinned #39/#40/#42 synthetic union — **pending this new workflow and security review**.
- Real Linux/Windows/macOS/Pi installed app and rollback — HOLD.
- Genuine CPU/Jan/Ollama/OpenCode/agent/Metal/CUDA/Hailo inference, physical cleanup — HOLD.
- Full cross-app Stable First, licensing, native hardware, packaging/release — HOLD.

Publish exact SHA and job URL to the Draft PR, [AI coordinator #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15),
and [master #8](https://github.com/Tihulu/tihulu-survival-kit-plan/issues/8).
No merge, owner-machine deployment, provider/device actions or changes to any earlier PR.
