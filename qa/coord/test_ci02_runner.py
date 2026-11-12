"""Negative controls for CI-02: no provider, model, hardware or real installer."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat
import sys
import tempfile
import unittest

from ci02_runner import Hold, hosted_runner_root, run_unittest_suite, safe_fixture_digest


class Ci02AdversarialContract(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="ci02-negative-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.runner = self.root / "runner"
        self.runner.mkdir(mode=0o700)
        self.workspace = self.root / "workspace"
        self.workspace.mkdir(mode=0o700)
        self.env = {
            "CI": "true", "GITHUB_ACTIONS": "true",
            "RUNNER_ENVIRONMENT": "github-hosted",
            "RUNNER_TEMP": str(self.runner),
            "GITHUB_WORKSPACE": str(self.workspace),
        }

    def test_01_valid_private_mock_boundary(self):
        self.assertEqual(hosted_runner_root(self.env), self.runner)

    def test_02_missing_ci_is_hold(self):
        for key in ("CI", "GITHUB_ACTIONS", "RUNNER_ENVIRONMENT"):
            mutated = dict(self.env, **{key: "false"})
            with self.subTest(key=key), self.assertRaises(Hold):
                hosted_runner_root(mutated)

    def test_03_global_root_tmp_var_tmp_rejected(self):
        for bad in ("/", "/tmp", "/var/tmp"):
            with self.subTest(root=bad), self.assertRaises(Hold):
                hosted_runner_root(dict(self.env, RUNNER_TEMP=bad))

    def test_04_runner_temp_cannot_reside_in_checkout(self):
        with self.assertRaises(Hold):
            hosted_runner_root(dict(self.env, RUNNER_TEMP=str(self.workspace)))

    def test_05_missing_workspace_or_relative_root_rejected(self):
        with self.assertRaises(Hold):
            hosted_runner_root(dict(self.env, GITHUB_WORKSPACE=""))
        with self.assertRaises(Hold):
            hosted_runner_root(dict(self.env, RUNNER_TEMP="./runner"))

    def test_06_symlinked_runner_root_rejected(self):
        link = self.root / "alias"
        link.symlink_to(self.runner, target_is_directory=True)
        with self.assertRaises(Hold):
            hosted_runner_root(dict(self.env, RUNNER_TEMP=str(link)))

    def test_07_non_directory_runner_root_rejected(self):
        regular = self.root / "regular"
        regular.write_text("foreign", encoding="utf-8")
        with self.assertRaises(Hold):
            hosted_runner_root(dict(self.env, RUNNER_TEMP=str(regular)))

    def test_08_0600_regular_fixture_digest_passes(self):
        p = self.root / "fixture.py"
        p.write_text("x = 1", encoding="utf-8")
        p.chmod(0o600)
        self.assertEqual(safe_fixture_digest(p),
                         hashlib.sha256(b"x = 1").hexdigest())

    def test_09_symlink_fixture_refused(self):
        p = self.root / "foreign"
        p.write_text("unchanged", encoding="utf-8")
        link = self.root / "alias.py"
        link.symlink_to(p)
        with self.assertRaises(Hold):
            safe_fixture_digest(link)
        self.assertEqual(p.read_text(), "unchanged")

    def test_10_hardlink_fixture_refused(self):
        p = self.root / "fixture.py"
        p.write_bytes(b"source")
        p.chmod(0o600)
        (self.root / "other.py").hardlink_to(p)
        with self.assertRaises(Hold):
            safe_fixture_digest(p)

    def test_11_mode_drift_refused(self):
        p = self.root / "fixture.py"
        p.write_bytes(b"source")
        p.chmod(0o644)
        with self.assertRaises(Hold):
            safe_fixture_digest(p)

    def test_12_rewritten_file_changes_digest(self):
        p = self.root / "fixture.py"
        p.write_bytes(b"original")
        p.chmod(0o600)
        first = safe_fixture_digest(p)
        p.write_bytes(b"substituted")
        self.assertNotEqual(safe_fixture_digest(p), first)

    def create_unittest(self, filename: str, payload: str) -> Path:
        directory = self.root / filename
        directory.mkdir()
        (directory / "test_fixture.py").write_text(payload, encoding="utf-8")
        return directory

    def test_13_single_actual_test_with_forged_28_receipt_is_hold(self):
        directory = self.create_unittest(
            "fake28",
            "import sys, unittest\n"
            "class T(unittest.TestCase):\n"
            "    def test_only(self):\n"
            "        print('Ran 28 tests', file=sys.stderr)\n"
        )
        with self.assertRaises(Hold):
            run_unittest_suite(directory, expected=28)

    def test_14_single_actual_test_with_forged_49_receipt_is_hold(self):
        directory = self.create_unittest(
            "fake49",
            "import sys, unittest\n"
            "class T(unittest.TestCase):\n"
            "    def test_only(self):\n"
            "        print('Ran 49 tests', file=sys.stderr)\n"
        )
        with self.assertRaises(Hold):
            run_unittest_suite(directory, expected=49)

    def test_15_actual_test_counter_passes_expected_one(self):
        directory = self.create_unittest(
            "valid1",
            "import unittest\n"
            "class T(unittest.TestCase):\n"
            "    def test_real(self): self.assertTrue(True)\n"
        )
        self.assertEqual(run_unittest_suite(directory, expected=1), (1, 1))


if __name__ == "__main__":
    unittest.main()
