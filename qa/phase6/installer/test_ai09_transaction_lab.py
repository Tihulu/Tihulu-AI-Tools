"""Security-negative tests for synthetic versioned runner lab, NEVER production."""
from __future__ import annotations

import fcntl
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

from ai09_transaction_lab import Lab, Hold, MARKER

MODULE = Path(__file__).with_name("ai09_transaction_lab.py")


def fixture(version: str) -> bytes:
    return ("#!/bin/sh\n# TIHULU_AI09_SYNTHETIC " + version + "\nexit 0\n").encode()


class DisposableTransactions(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="ai09-tx-parent-")
        self.addCleanup(self.tmp.cleanup)
        self.runner = Path(self.tmp.name) / "runner"
        self.runner.mkdir(mode=0o700)
        self.root = self.runner / "ai09tx-acceptance01"
        self.root.mkdir(mode=0o700)
        self.env = {"CI": "true", "GITHUB_ACTIONS": "true", "RUNNER_TEMP": str(self.runner)}
        with self.lab() as lab:
            lab.initialize()

    def lab(self):
        return Lab(self.root, self.runner, env=self.env)

    def install(self, version: str, payload: bytes | None = None):
        with self.lab() as lab:
            lab.switch(version, payload if payload is not None else fixture(version))

    def test_01_new_install_has_exact_immutable_fixture_sha(self):
        self.install("v1")
        with self.lab() as lab:
            self.assertEqual(lab.current(), "v1")
            self.assertEqual(lab._hash("v1"), hashlib.sha256(fixture("v1")).hexdigest())
        self.assertEqual((self.root / "versions/v1/fake-executable").read_bytes(), fixture("v1"))

    def test_02_reinstall_same_bytes_idempotent(self):
        self.install("v1")
        self.install("v1")
        self.assertEqual(list((self.root / "versions").iterdir())[0].name, "v1")
        self.assertEqual(len(list((self.root / "journal-history").iterdir())), 1)

    def test_03_upgrade_rollback_and_previous_preserved(self):
        self.install("v1")
        self.install("v2")
        with self.lab() as lab:
            lab.switch("v1")
            self.assertEqual(lab.current(), "v1")
            self.assertEqual(lab._hash("v2"), hashlib.sha256(fixture("v2")).hexdigest())
        self.assertEqual(set(p.name for p in (self.root / "versions").iterdir()), {"v1", "v2"})

    def test_04_uninstall_like_deactivation_preserves_all_version_bytes(self):
        self.install("v1")
        with self.lab() as lab:
            lab.switch(None)  # logical deactivation only, NOT actual uninstall
            self.assertIsNone(lab.current())
            lab.switch("v1")
            self.assertEqual(lab.current(), "v1")
        self.assertEqual((self.root / "versions/v1/fake-executable").read_bytes(), fixture("v1"))

    def test_05_duplicate_version_different_hash_fails_closed(self):
        self.install("v1")
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v1", fixture("v1") + b"# changed\n")
        self.assertEqual((self.root / "versions/v1/fake-executable").read_bytes(), fixture("v1"))

    def test_06_foreign_root_sentinel_survives_refusal(self):
        self.install("v1")
        sentinel = self.root / "media-center-do-not-touch"
        sentinel.write_text("foreign-owned", encoding="utf-8")
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v2", fixture("v2"))
        self.assertEqual(sentinel.read_text(), "foreign-owned")
        self.assertFalse((self.root / "versions/v2").exists())
        sentinel.unlink()  # test-only fixture cleanup, never in implementation
        nested = self.root / "versions/foreign-user-content"
        nested.write_text("do not delete", encoding="utf-8")
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v2", fixture("v2"))
        self.assertEqual(nested.read_text(), "do not delete")

    def test_07_symlinked_root_and_version_fail_without_touching_target(self):
        target = Path(self.tmp.name) / "foreign"
        target.mkdir()
        (target / "sentinel").write_bytes(b"owned elsewhere")
        self.root.rename(self.runner / "retired-original")
        self.root.symlink_to(target, target_is_directory=True)
        with self.assertRaises(Hold):
            self.lab()
        self.assertEqual((target / "sentinel").read_bytes(), b"owned elsewhere")

    def test_08_symlinked_payload_and_current_cannot_be_trusted(self):
        self.install("v1")
        target = Path(self.tmp.name) / "foreign-secret"
        target.write_bytes(b"secret remains")
        real = self.root / "versions/v1/fake-executable"
        real.unlink()  # the test corrupts only the disposable fixture
        real.symlink_to(target)
        with self.assertRaises(Hold), self.lab() as lab:
            lab.current()
        self.assertEqual(target.read_bytes(), b"secret remains")

    def test_09_corruption_is_hold_and_previous_bytes_remain(self):
        self.install("v1")
        self.install("v2")
        (self.root / "versions/v2/manifest.json").write_text("{}", encoding="utf-8")
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v1")
        self.assertEqual((self.root / "versions/v1/fake-executable").read_bytes(), fixture("v1"))

    def test_10_false_runner_scope_and_permission_drift_refused(self):
        for name, value in (("CI", "false"), ("GITHUB_ACTIONS", "false"), ("RUNNER_TEMP", "/tmp/other")):
            wrong = dict(self.env, **{name: value})
            with self.subTest(name=name), self.assertRaises(Hold):
                Lab(self.root, self.runner, env=wrong)
        self.root.chmod(0o755)
        with self.assertRaises(Hold):
            self.lab()

    def test_11_second_instance_refused_on_active_lock(self):
        with self.lab() as lab:
            with lab.locked():
                with self.lab() as rival:
                    with self.assertRaises(Hold):
                        with rival.locked():
                            pass
        self.install("v1")

    def test_12_lock_inode_substitution_fails_closed_for_pinned_holder(self):
        with self.lab() as lab:
            with self.assertRaises(Hold):
                with lab.locked():
                    (self.root / ".lock").rename(self.root / ".lock-retired")
        self.assertTrue((self.root / ".lock-retired").exists())
        # A hostile same-UID process could make a fresh lock and overlap:
        # that scenario is NOT secured by this CI-only harness.

    def _crash_worker(self, crash: str) -> subprocess.CompletedProcess:
        env = dict(os.environ, **self.env, AI09_TX_KILL_TEST_CHILD="true")
        return subprocess.run([sys.executable, str(MODULE), "--worker", "--lab", str(self.root),
                               "--runner-temp", str(self.runner), "--version", "v2", "--crash", crash],
                              env=env, capture_output=True, timeout=10)

    def test_13_real_child_sigkill_before_atomic_activation_keeps_old(self):
        self.install("v1")
        child = self._crash_worker("prepared")
        self.assertEqual(child.returncode, -9, child.stderr)
        with self.lab() as lab:
            self.assertEqual(lab.current(), "v1")
            self.assertEqual(lab.recover(), "old-preserved")
            self.assertEqual(lab.current(), "v1")
            self.assertEqual(lab._hash("v2"), hashlib.sha256(fixture("v2")).hexdigest())

    def test_14_real_child_sigkill_after_atomic_activation_keeps_new(self):
        self.install("v1")
        child = self._crash_worker("activated")
        self.assertEqual(child.returncode, -9, child.stderr)
        with self.lab() as lab:
            self.assertEqual(lab.current(), "v2")
            self.assertEqual(lab.recover(), "new-committed")
            lab.switch("v1")
            self.assertEqual(lab.current(), "v1")

    def test_15_unrecovered_journal_blocks_new_install_and_deactivation(self):
        self.install("v1")
        self.assertEqual(self._crash_worker("prepared").returncode, -9)
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch(None)
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v3", fixture("v3"))
        self.assertTrue((self.root / "journal").exists())
        with self.lab() as lab:
            self.assertEqual(lab.recover(), "old-preserved")
            lab.switch(None)
        self.assertEqual((self.root / "versions/v1/fake-executable").read_bytes(), fixture("v1"))

    def test_16_journal_forgery_refused_without_automatic_deletion(self):
        self.install("v1")
        (self.root / "journal").write_text('{"fake":"pass"}', encoding="utf-8")
        with self.assertRaises(Hold), self.lab() as lab:
            lab.recover()
        self.assertTrue((self.root / "journal").exists())
        self.assertEqual((self.root / "versions/v1/fake-executable").read_bytes(), fixture("v1"))

    def test_17_root_marker_and_active_symlink_forgeries_hold(self):
        self.install("v1")
        (self.root / ".ai09-owner").write_text("foreign", encoding="utf-8")
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v2", fixture("v2"))
        (self.root / ".ai09-owner").write_text(MARKER + "\n", encoding="utf-8")
        (self.root / "current").unlink()
        target = Path(self.tmp.name) / "untouched"
        target.write_bytes(b"private")
        (self.root / "current").symlink_to(target)
        with self.assertRaises(Hold), self.lab() as lab:
            lab.current()
        self.assertEqual(target.read_bytes(), b"private")

    def test_18_bad_payload_and_version_names_rejected(self):
        for version in ("../v2", "/tmp/v9", "V1", "v01", "v9999999"):
            with self.subTest(version=version), self.assertRaises(Hold), self.lab() as lab:
                lab.switch(version, fixture("v1"))
        with self.assertRaises(Hold), self.lab() as lab:
            lab.switch("v1", b"an external executable")
        self.assertEqual(list((self.root / "versions").iterdir()), [])


if __name__ == "__main__":
    unittest.main()
