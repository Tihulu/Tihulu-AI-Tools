"""COORD-CI-02 QA ONLY — pinned-source suites on independent hosted runner jobs.

This runner is not a production installer, security sandbox for arbitrary
Python, model runtime, provider connector or a substitute for required CI.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest

# Explicit pinned historical #44 source; never import from a moving Git ref.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from cross_pr_attestor import Hold, added_only, disjoint_groups, git, tree

FROZEN = "283edc8a1101f63e08f12c172d6b2d6688b9a5ca"
PREVIOUS = "127b5331236cc9d5adb37122d8ab52f7492203c5"
PREVIOUS_FILES = frozenset({
    ".github/workflows/phase6-coord-cross-pr-attestation.yml",
    "docs/qa/COORD_CI_01.md",
    "qa/coord/cross_pr_attestor.py",
    "qa/coord/test_cross_pr_attestor.py",
})
OWN_FILES = frozenset({
    ".github/workflows/phase6-coord-ci02-isolated.yml",
    "docs/qa/COORD_CI_02.md",
    "qa/coord/ci02_runner.py",
    "qa/coord/test_ci02_runner.py",
})
SOURCES = {
    "ai09_preflight": {
        "head": "ea6667b72d01fadf222c1c50363e4df7a2a3b7f3",
        "expected": 10,
        "paths": frozenset({
            ".github/workflows/phase6-ai09-lab-preflight.yml",
            "docs/qa/installer/AI09_INITIAL_PREFLIGHT.md",
            "qa/phase6/installer/scope_guard.py",
            "qa/phase6/installer/test_scope_guard.py",
        }),
        "python": ("qa/phase6/installer/scope_guard.py",
                   "qa/phase6/installer/test_scope_guard.py"),
    },
    "ai09_transaction": {
        "head": "c11d2bb38f04d0ac4c03529a70284c136e58c254",
        "expected": 18,
        "paths": frozenset({
            ".github/workflows/phase6-ai09-transaction-lab.yml",
            "docs/qa/installer/AI09_DISPOSABLE_TRANSACTION_ACCEPTANCE.md",
            "qa/phase6/installer/ai09_transaction_lab.py",
            "qa/phase6/installer/test_ai09_transaction_lab.py",
        }),
        "python": ("qa/phase6/installer/ai09_transaction_lab.py",
                   "qa/phase6/installer/test_ai09_transaction_lab.py"),
    },
    "ai10_policy": {
        "head": "b28a4b1b1869289fd77392f8e1985f9b3b00f3b5",
        "expected": 15,
        "paths": frozenset({
            ".github/workflows/phase6-ai10-runtime-policy.yml",
            "docs/qa/model-runtime/AI10_POLICY_ORACLE.md",
            "qa/phase6/runtime/runtime_policy.py",
            "qa/phase6/runtime/test_runtime_policy.py",
        }),
        "python": ("qa/phase6/runtime/runtime_policy.py",
                   "qa/phase6/runtime/test_runtime_policy.py"),
    },
    "ai10_sessions": {
        "head": "c817eed1a1088fa642102fefce6cc57b6ac981a6",
        "expected": 34,
        "paths": frozenset({
            ".github/workflows/phase6-ai10-provider-sessions.yml",
            "docs/qa/model-runtime/AI10_PROVIDER_SESSION_ACCEPTANCE.md",
            "qa/phase6/runtime/ai10_synthetic_sessions.py",
            "qa/phase6/runtime/test_ai10_synthetic_sessions.py",
        }),
        "python": ("qa/phase6/runtime/ai10_synthetic_sessions.py",
                   "qa/phase6/runtime/test_ai10_synthetic_sessions.py"),
    },
}


def verify_attestation() -> dict[str, dict[str, tuple[str, str, str]]]:
    """No old source changes; exactly four new paths per immutable source."""
    base = tree(FROZEN)
    prior = tree(PREVIOUS)
    git("merge-base", "--is-ancestor", FROZEN, PREVIOUS)
    added_only(base, prior, PREVIOUS_FILES)
    current = git("rev-parse", "HEAD").decode("ascii").strip()
    git("merge-base", "--is-ancestor", PREVIOUS, current)
    added_only(prior, tree(current), OWN_FILES)
    disjoint_groups({name: info["paths"] for name, info in SOURCES.items()})
    snapshots = {}
    for name, info in SOURCES.items():
        head = info["head"]
        git("merge-base", "--is-ancestor", FROZEN, head)
        candidate = tree(head)
        added_only(base, candidate, info["paths"])
        for path in info["python"]:
            if path not in info["paths"]:
                raise Hold("Unauthorized executable fixture: " + path)
        snapshots[name] = candidate
        print("PINNED SOURCE PASS:", name, head, flush=True)
    print("IMMUTABLE GIT TREE/BLOB/MODE CHECK: PASS (frozen #34 + #44 preserved)")
    return snapshots


def hosted_runner_root(environ: dict[str, str]) -> Path:
    """Require a GitHub-hosted runner and reject root/temp alias shortcuts."""
    if any(environ.get(k) != v for k, v in {
        "CI": "true", "GITHUB_ACTIONS": "true",
        "RUNNER_ENVIRONMENT": "github-hosted",
    }.items()):
        raise Hold("Missing genuine hosted-runner environment markers")
    raw = environ.get("RUNNER_TEMP", "")
    if not raw or not os.path.isabs(raw):
        raise Hold("Missing absolute temporary runner root")
    path = Path(raw)
    if path in (Path("/"), Path("/tmp"), Path("/var/tmp")):
        raise Hold("Refuse arbitrary shared/global temp directory")
    workspace = environ.get("GITHUB_WORKSPACE", "")
    if not workspace or not Path(workspace).is_absolute():
        raise Hold("Missing GitHub workspace boundary")
    w = Path(workspace)
    if path == w or path.is_relative_to(w):
        raise Hold("Runner temp cannot be inside the Git checkout")
    for entry in (path, *path.parents):
        try:
            info = entry.lstat()
        except OSError as exc:
            raise Hold("Missing runner temp ancestor") from exc
        if stat.S_ISLNK(info.st_mode) or not stat.S_ISDIR(info.st_mode):
            raise Hold("Symlink or non-directory runner temp ancestor")
    if path.stat().st_uid != os.getuid():
        raise Hold("Runner temp root not owned by executing UID")
    return path


def safe_fixture_digest(path: Path) -> str:
    """Fail closed on rewritten/symlinked files; no real-path trust claims."""
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise Hold("Fixture is symlink, nonregular or multiply linked")
    if (stat.S_IMODE(info.st_mode) != 0o600 or
            info.st_uid != os.getuid()):
        raise Hold("Fixture owner/mode drift")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run_unittest_suite(directory: Path, expected: int) -> tuple[int, int]:
    """Use actual TestResult.testsRun, never grep/parsing fixture stderr."""
    loader = unittest.TestLoader()
    suite = loader.discover(start_dir=str(directory), pattern="test_*.py",
                            top_level_dir=str(directory))
    discovered = suite.countTestCases()
    result = unittest.TextTestRunner(verbosity=2, stream=sys.stderr).run(suite)
    if (not result.wasSuccessful() or result.testsRun != expected or
            discovered != expected):
        raise Hold(f"Observed unittest count {result.testsRun}/{discovered}, "
                   f"expected {expected}")
    return discovered, result.testsRun


def test_one(name: str, snapshots: dict) -> None:
    """Each caller is a SEPARATE GitHub-hosted runner matrix job."""
    if name not in SOURCES:
        raise Hold("Unknown suite")
    parent = hosted_runner_root(os.environ)
    info = SOURCES[name]
    tree_entries = snapshots[name]
    with tempfile.TemporaryDirectory(prefix=f"tihulu-ci02-{name}-", dir=parent) as d:
        lab = Path(d)
        if (stat.S_IMODE(lab.lstat().st_mode) != 0o700 or
                lab.lstat().st_uid != os.getuid()):
            raise Hold("Ephemeral suite lab not private")
        digests: dict[str, str] = {}
        for path in info["python"]:
            mode, kind, oid = tree_entries[path]
            if mode != "100644" or kind != "blob":
                raise Hold("Source is not pinned regular Git blob")
            data = git("cat-file", "blob", oid)
            calculated = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") +
                                      b"\0" + data).hexdigest()
            if calculated != oid:
                raise Hold("Git blob content does not match pinned SHA")
            dest = lab / Path(path).name
            if dest.exists() or dest.is_symlink():
                raise Hold("Duplicate/symlinked suite file")
            with dest.open("xb") as stream:
                stream.write(data)
            dest.chmod(0o600)
            digests[dest.name] = safe_fixture_digest(dest)
        observed = run_unittest_suite(lab, info["expected"])
        for name_path, original_sha in digests.items():
            if safe_fixture_digest(lab / name_path) != original_sha:
                raise Hold("Python test fixture modified during suite execution")
        present = {p.name for p in lab.iterdir()}
        if present != set(digests):
            raise Hold("Suite introduced foreign files or hidden artifacts")
        print(f"SCOPED SUITE PASS: {name} {observed[1]}/{info['expected']} "
              "actual unittest TestResult tests on independent hosted runner")
        print("NOT approved: adversarial arbitrary-Python sandbox; required checks; "
              "installed package; real model/Jan/GPU; release.")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("attest", "suite"))
    parser.add_argument("suite_name", nargs="?", choices=tuple(SOURCES))
    args = parser.parse_args()
    try:
        snapshots = verify_attestation()
        if args.command == "suite":
            if args.suite_name is None:
                raise Hold("Suite name missing")
            test_one(args.suite_name, snapshots)
        else:
            if args.suite_name:
                raise Hold("Unexpected suite name for attestation")
        return 0
    except (Hold, OSError, ValueError) as exc:
        print("COORD-CI-02 HOLD:", exc, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
