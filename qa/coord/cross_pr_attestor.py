"""CI-only exact-Git-tree cross-PR attestation; synthetic tests only.

Pins reviewed additive PR source snapshots; cannot certify production packages,
real inference, hardware, branch protection, or inherited required-check policy.
"""
from __future__ import annotations

import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile

BASE = "283edc8a1101f63e08f12c172d6b2d6688b9a5ca"
HEADS = {
    "ai09": (
        "ea6667b72d01fadf222c1c50363e4df7a2a3b7f3",
        frozenset({
            ".github/workflows/phase6-ai09-lab-preflight.yml",
            "docs/qa/installer/AI09_INITIAL_PREFLIGHT.md",
            "qa/phase6/installer/scope_guard.py",
            "qa/phase6/installer/test_scope_guard.py",
        }),
    ),
    "ai10_policy": (
        "b28a4b1b1869289fd77392f8e1985f9b3b00f3b5",
        frozenset({
            ".github/workflows/phase6-ai10-runtime-policy.yml",
            "docs/qa/model-runtime/AI10_POLICY_ORACLE.md",
            "qa/phase6/runtime/runtime_policy.py",
            "qa/phase6/runtime/test_runtime_policy.py",
        }),
    ),
    "ai10_sessions": (
        "c817eed1a1088fa642102fefce6cc57b6ac981a6",
        frozenset({
            ".github/workflows/phase6-ai10-provider-sessions.yml",
            "docs/qa/model-runtime/AI10_PROVIDER_SESSION_ACCEPTANCE.md",
            "qa/phase6/runtime/ai10_synthetic_sessions.py",
            "qa/phase6/runtime/test_ai10_synthetic_sessions.py",
        }),
    ),
}
SELF_FILES = frozenset({
    ".github/workflows/phase6-coord-cross-pr-attestation.yml",
    "docs/qa/COORD_CI_01.md",
    "qa/coord/cross_pr_attestor.py",
    "qa/coord/test_cross_pr_attestor.py",
})


class Hold(RuntimeError):
    """Fail-closed test result, never a release approval."""


def git(*args: str) -> bytes:
    proc = subprocess.run(
        ("git", *args), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=False, timeout=40,
    )
    if proc.returncode:
        raise Hold("Git identity/ancestry lookup failed: " + " ".join(args[:3]))
    return proc.stdout


def tree(commit: str) -> dict[str, tuple[str, str, str]]:
    """Return path -> (mode, object type, blob SHA), preserving file modes."""
    result: dict[str, tuple[str, str, str]] = {}
    for record in git("ls-tree", "-rz", "--full-tree", commit).split(b"\0"):
        if not record:
            continue
        try:
            header, raw_path = record.split(b"\t", 1)
            mode, kind, obj_sha = header.decode("ascii").split(" ")
            path = raw_path.decode("utf-8", "strict")
        except (ValueError, UnicodeError) as exc:
            raise Hold("Malformed Git tree record") from exc
        if path in result:
            raise Hold("Duplicate Git tree path")
        result[path] = (mode, kind, obj_sha)
    if not result:
        raise Hold("Empty Git source tree")
    return result


def added_only(base: dict, candidate: dict, expected: frozenset[str]) -> None:
    """Require all inherited entries byte/mode-identical and ONLY exact additions."""
    for path, original in base.items():
        if candidate.get(path) != original:
            raise Hold("Original blob or mode changed/deleted: " + path)
    additions = set(candidate) - set(base)
    if additions != expected:
        raise Hold("Unexpected/missing new paths: " + repr(sorted(additions ^ expected)))
    for path in expected:
        mode, kind, blob = candidate[path]
        if mode != "100644" or kind != "blob" or not re.fullmatch("[a-f0-9]{40}", blob):
            raise Hold("New file is not a regular 100644 Git blob: " + path)


def disjoint_groups(groups: dict[str, frozenset[str]]) -> None:
    seen: set[str] = set()
    for name, names in groups.items():
        if not names or (seen & names):
            raise Hold("Overlapping or empty additive group: " + name)
        seen.update(names)


def attest() -> dict[str, tuple[str, str, str]]:
    baseline = tree(BASE)
    own_head = git("rev-parse", "HEAD").decode().strip()
    git("merge-base", "--is-ancestor", BASE, own_head)
    added_only(baseline, tree(own_head), SELF_FILES)
    disjoint_groups({key: names for key, (_, names) in HEADS.items()})
    combined: dict[str, tuple[str, str, str]] = {}
    for name, (head, expected) in HEADS.items():
        git("merge-base", "--is-ancestor", BASE, head)
        candidate = tree(head)
        added_only(baseline, candidate, expected)
        for path in sorted(expected):
            combined[path] = candidate[path]
            print("ATTEST", name, head, candidate[path][2], path, flush=True)
    return combined


def runner_temp() -> Path:
    if os.environ.get("CI") != "true" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise Hold("No non-CI execution: GitHub Actions disposable runner only")
    raw = os.environ.get("RUNNER_TEMP", "")
    if not raw or not Path(raw).is_absolute():
        raise Hold("Missing absolute RUNNER_TEMP")
    parent = Path(raw)
    if parent.is_symlink() or not parent.is_dir():
        raise Hold("Refuse redirected/non-directory runner root")
    return parent


def test_union(blobs: dict[str, tuple[str, str, str]]) -> None:
    parent = runner_temp()
    with tempfile.TemporaryDirectory(prefix="tihulu-coord-ci01-", dir=parent) as tmp:
        lab = Path(tmp)
        if stat.S_IMODE(lab.stat().st_mode) != 0o700:
            raise Hold("Lab directory must be owner-private")
        for path, (mode, kind, blob_sha) in blobs.items():
            if not path.startswith("qa/phase6/") or not path.endswith(".py"):
                continue  # Attest workflow/docs, but never execute them.
            dst = lab / path
            dst.parent.mkdir(parents=True, exist_ok=True)
            if dst.exists() or dst.is_symlink():
                raise Hold("Unexpected duplicate fixture path")
            data = git("cat-file", "blob", blob_sha)
            with dst.open("xb") as output:
                output.write(data)
            dst.chmod(0o600)
        for start, expected in (("qa/phase6/installer", 10), ("qa/phase6/runtime", 49)):
            suite = lab / start
            if not suite.is_dir():
                raise Hold("Missing fixture group")
            env = dict(os.environ, HOME=str(lab), PYTHONDONTWRITEBYTECODE="1")
            cmd = (sys.executable, "-I", "-m", "unittest", "discover", "-s",
                   str(suite), "-p", "test_*.py", "-v")
            run = subprocess.run(cmd, cwd=lab, env=env, capture_output=True,
                                 text=True, timeout=100, check=False)
            print(run.stdout[-5000:], run.stderr[-12000:], flush=True)
            observed = re.search(r"Ran (\d+) tests", run.stderr)
            if run.returncode or not observed or int(observed.group(1)) != expected:
                raise Hold(f"Unexpected {start} test result; expected {expected} passing")
        print("SCOPED PASS: 59/59 synthetic source tests; NO live IPC/real inference/install")


def main() -> int:
    try:
        snapshots = attest()
        test_union(snapshots)
        return 0
    except (Hold, OSError, subprocess.TimeoutExpired) as error:
        print("COORD-CI-01 HOLD:", error, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
