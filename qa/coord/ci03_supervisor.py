"""COORD-CI-03: trusted parent supervising fixed, reviewed Git-pinned Python fixtures.

Security boundary: child cannot alter parent's Python globals / exit code / post-run
hash checks. This is NOT a sandbox for arbitrary adversarial Python running as the
same UID; external/unreviewed fixture code MUST NOT be accepted as trusted.
"""
from __future__ import annotations

import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
import tempfile

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
from cross_pr_attestor import Hold, added_only, git, tree
from ci02_runner import SOURCES, safe_fixture_digest

BASE = "61708417c7573e3dbacdcd61e947b76f69214a74"  # frozen #45
EXACT_NEW_PATHS = frozenset({
    ".github/workflows/phase6-coord-ci03-supervisor.yml",
    "docs/qa/COORD_CI_03.md",
    "qa/coord/ci03_supervisor.py",
    "qa/coord/test_ci03_supervisor.py",
})

WORKER = r'''
import json
from pathlib import Path
import sys
import unittest

testdir = Path(sys.argv[1])
sys.path.insert(0, str(testdir))
suite = unittest.TestLoader().discover(str(testdir), pattern="test_*.py", top_level_dir=str(testdir))

def walk(node):
    for part in node:
        if isinstance(part, unittest.TestSuite):
            yield from walk(part)
        else:
            yield part

names = sorted(case._testMethodName for case in walk(suite))
result = unittest.TextTestRunner(stream=sys.stderr, verbosity=1).run(suite)
if result.wasSuccessful() and len(names) == result.testsRun:
    print("TIHULU_CI03_CHILD_RESULT=" + json.dumps(
        {"cases": result.testsRun, "names": names, "ok": True},
        sort_keys=True), flush=True)
else:
    sys.exit(2)
'''


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def is_suspicious_fixture_source(source: bytes) -> bool:
    """Bounded precaution, NOT an arbitrary-code sandbox or malware scanner.

    Prohibit the exact known test-file attack constructs. Uses AST to avoid
    superficial whitespace tricks, but dynamic imports/eval could evade it.
    """
    syntax = ast.parse(source.decode("utf-8", "strict"))
    def name(expr):
        if isinstance(expr, ast.Name):
            return expr.id
        if isinstance(expr, ast.Attribute):
            return name(expr.value) + "." + expr.attr
        return ""
    for node in ast.walk(syntax):
        if isinstance(node, ast.Call):
            called = name(node.func)
            if called in {"os._exit", "sys.exit", "builtins.exit", "unittest.mock.patch"}:
                # sys.exit in tests may be used for normal exception handling,
                # so this is a strict reviewed-fixture policy only.
                return True
            if called == "setattr" and len(node.args) >= 2:
                if isinstance(node.args[1], ast.Constant) and node.args[1].value in (
                    "countTestCases", "run", "safe_fixture_digest", "wasSuccessful",
                    "testsRun",
                ):
                    return True
        if isinstance(node, (ast.Assign, ast.AnnAssign, ast.AugAssign)):
            targets = node.targets if isinstance(node, ast.Assign) else [node.target]
            for target in targets:
                n = name(target)
                if n.startswith(("unittest.TestSuite.", "unittest.TextTestRunner.",
                                 "__main__.safe_fixture_digest")):
                    return True
    return False


def static_test_names(source: bytes) -> list[str]:
    """Parent parses reviewed TEST source before spawning a disposable child."""
    if is_suspicious_fixture_source(source):
        raise Hold("Known hostile test monkeypatch/process-exit construct")
    module = ast.parse(source.decode("utf-8", "strict"))
    methods = []
    for cls in module.body:
        if isinstance(cls, ast.ClassDef):
            methods += [node.name for node in cls.body
                        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                        and node.name.startswith("test_")]
    if not methods or len(methods) != len(set(methods)):
        raise Hold("No unique static test methods")
    return sorted(methods)


def attest() -> None:
    parent_tree = tree(BASE)
    current = git("rev-parse", "HEAD").decode("ascii").strip()
    git("merge-base", "--is-ancestor", BASE, current)
    added_only(parent_tree, tree(current), EXACT_NEW_PATHS)
    # Keep all historical #34/#44/#45 and four external agent sources pinned.
    for info in SOURCES.values():
        obj = tree(info["head"])
        if not all(path in obj and obj[path][0] == "100644" for path in info["python"]):
            raise Hold("Frozen source fixture changed/missing")
    print("COORD-CI-03 frozen #45 source and new-path invariants PASS", flush=True)


def private_runner_root(env: dict[str, str]) -> Path:
    if any(env.get(k) != v for k, v in {
        "CI": "true", "GITHUB_ACTIONS": "true",
        "RUNNER_ENVIRONMENT": "github-hosted",
    }.items()):
        raise Hold("GitHub-hosted environment flags missing")
    value = env.get("RUNNER_TEMP", "")
    work = env.get("GITHUB_WORKSPACE", "")
    if not value or not work or not os.path.isabs(value) or not os.path.isabs(work):
        raise Hold("Missing trusted absolute runner/workspace paths")
    path, workspace = Path(value), Path(work)
    if path in (Path("/"), Path("/tmp"), Path("/var/tmp")):
        raise Hold("Refuse global/ambiguous temporary directory")
    if path == workspace or path.is_relative_to(workspace):
        raise Hold("Runner temp must be separate from source checkout")
    for component in (path, *path.parents):
        try:
            info = component.lstat()
        except OSError as exc:
            raise Hold("Missing runner ancestor") from exc
        if not stat.S_ISDIR(info.st_mode) or stat.S_ISLNK(info.st_mode):
            raise Hold("Runner ancestor is symlink/non-directory")
    root = path.lstat()
    if (root.st_uid != os.getuid() or
            stat.S_IMODE(root.st_mode) & 0o022):
        raise Hold("Runner root is not owned/private against other users")
    # Environment marker is not proof of a genuinely hosted machine.
    return path


def verify_child_result(
    completed: subprocess.CompletedProcess[str], static_names: list[str],
) -> None:
    """Parent-owned predicate; children cannot monkeypatch it across process."""
    if completed.returncode != 0:
        raise Hold(f"Untrusted child returned nonzero ({completed.returncode})")
    lines = completed.stdout.splitlines()
    receipts = [s.split("=", 1)[1] for s in lines
                if s.startswith("TIHULU_CI03_CHILD_RESULT=")]
    if len(receipts) != 1 or not lines or not lines[-1].startswith(
        "TIHULU_CI03_CHILD_RESULT="
    ):
        raise Hold("No unique completed child receipt")
    try:
        r = json.loads(receipts[0])
    except (ValueError, TypeError) as exc:
        raise Hold("Unparseable child receipt") from exc
    if (not isinstance(r, dict) or set(r) != {"cases", "names", "ok"} or
            r["ok"] is not True or type(r["cases"]) is not int or
            r["cases"] != len(static_names) or r["names"] != static_names):
        raise Hold("Child result does not match parent static test inventory")
    # The receipt is child-generated evidence, not an unforgeable execution proof.


def child_environment(home: Path) -> dict[str, str]:
    return {
        "HOME": str(home),
        "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
        "LANG": "C.UTF-8",
        "PYTHONDONTWRITEBYTECODE": "1",
    }


def supervised_run(
    python_source: dict[str, bytes], expected: int, parent: Path,
    *, worker: str = WORKER,
) -> None:
    """Called by trusted parent; negative tests can substitute throwaway source."""
    if set(python_source) != {"module.py", "test_fixture.py"}:
        raise Hold("Wrong exactly-two-fixture inventory")
    static_names = static_test_names(python_source["test_fixture.py"])
    if len(static_names) != expected:
        raise Hold(f"Static test inventory expected {expected}, got {len(static_names)}")
    with tempfile.TemporaryDirectory(prefix="tihulu-ci03-", dir=parent) as root:
        lab = Path(root)
        if (lab.lstat().st_uid != os.getuid() or
                stat.S_IMODE(lab.lstat().st_mode) != 0o700):
            raise Hold("Suite working root is not 0700/private")
        expected_digest = {}
        for filename, data in python_source.items():
            dest = lab / filename
            with dest.open("xb") as out:
                out.write(data)
            dest.chmod(0o600)
            expected_digest[filename] = sha256_bytes(data)
            if safe_fixture_digest(dest) != expected_digest[filename]:
                raise Hold("Before-execution fixture SHA mismatch")
        worker_result = subprocess.run(
            (sys.executable, "-I", "-B", "-c", worker, str(lab)),
            cwd=lab, env=child_environment(lab),
            capture_output=True, text=True, timeout=80, check=False,
        )
        # All verifications below occur in a different, surviving OS process.
        for filename, digest in expected_digest.items():
            if safe_fixture_digest(lab / filename) != digest:
                raise Hold("Post-execution fixture SHA mismatch")
        if {p.name for p in lab.iterdir()} != set(python_source):
            raise Hold("Unexpected child-created file in private lab")
        verify_child_result(worker_result, static_names)
        print(f"COORD-CI-03 EXTERNAL-PARENT SCOPED PASS: {expected}/{expected}", flush=True)


def single_pinned_suite(name: str) -> None:
    if name not in SOURCES:
        raise Hold("Unknown fixed pinned fixture")
    parent = private_runner_root(os.environ)
    info = SOURCES[name]
    obj = tree(info["head"])
    python_source: dict[str, bytes] = {}
    for idx, path in enumerate(info["python"]):
        mode, typ, oid = obj[path]
        if (mode, typ) != ("100644", "blob"):
            raise Hold("Pin is not a regular source blob")
        data = git("cat-file", "blob", oid)
        as_oid = hashlib.sha1(b"blob " + str(len(data)).encode("ascii") +
                              b"\0" + data).hexdigest()
        if as_oid != oid:
            raise Hold("Source failed immutable git blob identity")
        python_source["module.py" if idx == 0 else "test_fixture.py"] = data
    # Preserve original Python import module names! Test suites import these.
    original_names = [Path(p).name for p in info["python"]]
    remapped = {
        "module.py": python_source["module.py"],
        "test_fixture.py": python_source["test_fixture.py"],
    }
    # Source tests import original module name; rewrite ONLY local wrapper filenames,
    # never source content. Execution uses original Git blob files unchanged.
    supervised_actual(info, original_names, remapped, parent)


def supervised_actual(info: dict, original_names: list[str],
                      fixture_bytes: dict[str, bytes], parent: Path) -> None:
    """Same external-parent checks, with unchanged real source module names."""
    source = {original_names[0]: fixture_bytes["module.py"],
              original_names[1]: fixture_bytes["test_fixture.py"]}
    static_names = static_test_names(source[original_names[1]])
    if len(static_names) != info["expected"]:
        raise Hold("Static inventory mismatched pinned expected case count")
    with tempfile.TemporaryDirectory(prefix="tihulu-ci03-", dir=parent) as d:
        lab = Path(d)
        if (stat.S_IMODE(lab.lstat().st_mode) != 0o700 or
                lab.lstat().st_uid != os.getuid()):
            raise Hold("Unowned/writable suite directory")
        hashes = {}
        for file, data in source.items():
            dest = lab / file
            with dest.open("xb") as out:
                out.write(data)
            dest.chmod(0o600)
            hashes[file] = sha256_bytes(data)
            if safe_fixture_digest(dest) != hashes[file]:
                raise Hold("Pre-run digest mismatch")
        result = subprocess.run(
            (sys.executable, "-I", "-B", "-c", WORKER, str(lab)),
            cwd=lab, env=child_environment(lab),
            capture_output=True, text=True, timeout=80, check=False,
        )
        for file, digest in hashes.items():
            if safe_fixture_digest(lab / file) != digest:
                raise Hold("Post-run digest changed by child")
        if {p.name for p in lab.iterdir()} != set(source):
            raise Hold("Foreign file created by child")
        verify_child_result(result, static_names)
        print(f"CI03 PINNED {info['head']} SCOPED PASS "
              f"{info['expected']}/{info['expected']} after external parent checks",
              flush=True)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("attest", "suite"))
    parser.add_argument("suite", nargs="?", choices=tuple(SOURCES))
    args = parser.parse_args()
    try:
        attest()
        if args.command == "suite":
            if not args.suite:
                raise Hold("Missing suite")
            single_pinned_suite(args.suite)
        elif args.suite:
            raise Hold("Unexpected suite for attest")
        return 0
    except (Hold, OSError, ValueError, SyntaxError,
            subprocess.TimeoutExpired) as exc:
        print("COORD-CI-03 HOLD:", exc, file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
