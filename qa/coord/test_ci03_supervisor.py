"""COORD-CI-03 bounded negative controls: no owner hardware/models/install."""
from __future__ import annotations

import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from ci03_supervisor import (
    Hold, private_runner_root, static_test_names, supervised_run,
    verify_child_result,
)

GOOD = b"""import unittest
class T(unittest.TestCase):
    def test_real(self):
        self.assertEqual(1 + 1, 2)
"""


class ExternalParentNegatives(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="ci03-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.runner = self.root / "runner"
        self.runner.mkdir(mode=0o700)
        self.workspace = self.root / "workspace"
        self.workspace.mkdir(mode=0o700)
        self.flags = {
            "CI": "true", "GITHUB_ACTIONS": "true",
            "RUNNER_ENVIRONMENT": "github-hosted",
            "RUNNER_TEMP": str(self.runner),
            "GITHUB_WORKSPACE": str(self.workspace),
        }

    def fixture(self, test_source=GOOD):
        return {"module.py": b"SAFE = True\n", "test_fixture.py": test_source}

    def test_01_external_parent_accepts_known_one_case(self):
        supervised_run(self.fixture(), 1, self.runner)

    def test_02_early_child_exit_zero_has_no_receipt_and_fails(self):
        with self.assertRaises(Hold):
            supervised_run(self.fixture(), 1, self.runner,
                           worker="import os; os._exit(0)")

    def test_03_child_file_mutation_caught_by_parent_post_hash(self):
        with self.assertRaises(Hold):
            supervised_run(self.fixture(), 1, self.runner,
                           worker="from pathlib import Path; Path('module.py').write_text('altered')")

    def test_04_unittest_count_monkeypatch_denied_before_execution(self):
        malicious = b"""import unittest
unittest.TestSuite.countTestCases = lambda self: 28
unittest.TextTestRunner.run = lambda self, suite: None
class T(unittest.TestCase):
    def test_fake(self):
        self.fail("should not run")
"""
        with self.assertRaises(Hold):
            static_test_names(malicious)

    def test_05_os_exit_in_test_source_denied_before_execution(self):
        malicious = b"""import unittest, os
class T(unittest.TestCase):
    def test_shortcircuit(self):
        os._exit(0)
"""
        with self.assertRaises(Hold):
            static_test_names(malicious)

    def test_06_parent_rejects_no_child_receipt(self):
        c = subprocess.CompletedProcess(args=[], returncode=0, stdout="", stderr="")
        with self.assertRaises(Hold):
            verify_child_result(c, ["test_real"])

    def test_07_parent_rejects_forged_28_case_receipt_for_one_case(self):
        c = subprocess.CompletedProcess(args=[], returncode=0,
            stdout='TIHULU_CI03_CHILD_RESULT={"cases": 28, "names": ["test_real"], "ok": true}\n',
            stderr="Ran 28 tests")
        with self.assertRaises(Hold):
            verify_child_result(c, ["test_real"])

    def test_08_parent_rejects_nonzero_exit_even_with_valid_receipt(self):
        c = subprocess.CompletedProcess(args=[], returncode=2,
            stdout='TIHULU_CI03_CHILD_RESULT={"cases": 1, "names": ["test_real"], "ok": true}\n',
            stderr="")
        with self.assertRaises(Hold):
            verify_child_result(c, ["test_real"])

    def test_09_parent_rejects_duplicate_child_receipts(self):
        receipt='TIHULU_CI03_CHILD_RESULT={"cases": 1, "names": ["test_real"], "ok": true}\n'
        c = subprocess.CompletedProcess(args=[], returncode=0,
                                        stdout=receipt * 2, stderr="")
        with self.assertRaises(Hold):
            verify_child_result(c, ["test_real"])

    def test_10_world_writable_runner_root_rejected(self):
        self.runner.chmod(0o777)
        with self.assertRaises(Hold):
            private_runner_root(self.flags)

    def test_11_symlinked_root_rejected(self):
        alias = self.root / "runner-alias"
        alias.symlink_to(self.runner, target_is_directory=True)
        with self.assertRaises(Hold):
            private_runner_root(dict(self.flags, RUNNER_TEMP=str(alias)))

    def test_12_root_and_global_temp_rejected(self):
        for raw in ("/", "/tmp", "/var/tmp"):
            with self.subTest(path=raw), self.assertRaises(Hold):
                private_runner_root(dict(self.flags, RUNNER_TEMP=raw))

    def test_13_working_repo_root_rejected(self):
        with self.assertRaises(Hold):
            private_runner_root(dict(self.flags,
                RUNNER_TEMP=str(self.workspace)))

    def test_14_world_writable_root_restored_private_accepts_syntax(self):
        self.runner.chmod(0o700)
        self.assertEqual(private_runner_root(self.flags), self.runner)

    def test_15_static_test_count_must_match_pinned_approved_count(self):
        with self.assertRaises(Hold):
            supervised_run(self.fixture(), 28, self.runner)


if __name__ == "__main__":
    unittest.main()
