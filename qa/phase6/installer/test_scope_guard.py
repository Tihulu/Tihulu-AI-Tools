"""Synthetic failure and path isolation checks. No installer or cleanup executed."""
from __future__ import annotations

import os
import tempfile
import unittest
from pathlib import Path

from scope_guard import LabHold, inspect_pristine_lab


class Ai09ScopePreflight(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="ai09-tests-")
        self.addCleanup(self.temp.cleanup)
        self.parent = Path(self.temp.name) / "runner"
        self.parent.mkdir(mode=0o700)
        self.lab = Path(tempfile.mkdtemp(prefix="ai09-", dir=self.parent))
        self.env = {"CI": "true", "GITHUB_ACTIONS": "true"}

    def check(self, lab: Path | None = None, parent: Path | None = None,
              env: dict[str, str] | None = None) -> tuple[int, int]:
        return inspect_pristine_lab(
            lab if lab is not None else self.lab,
            parent if parent is not None else self.parent,
            env if env is not None else self.env,
        )

    def test_01_virgin_runner_laboratory_passes(self) -> None:
        device, inode = self.check()
        self.assertEqual((device, inode), (self.lab.stat().st_dev, self.lab.stat().st_ino))

    def test_02_no_ci_or_actions_environment_rejected(self) -> None:
        for key in ("CI", "GITHUB_ACTIONS"):
            bad = dict(self.env)
            bad[key] = "false"
            with self.subTest(key=key), self.assertRaises(LabHold):
                self.check(env=bad)

    def test_03_relative_and_foreign_locations_rejected(self) -> None:
        with self.assertRaises(LabHold):
            self.check(lab=Path("ai09-unsafepath"))
        foreign = Path(tempfile.mkdtemp(prefix="ai09-", dir=self.temp.name))
        with self.assertRaises(LabHold):
            self.check(lab=foreign)

    def test_04_lab_symlink_to_another_directory_rejected(self) -> None:
        external = Path(tempfile.mkdtemp(prefix="ai09-", dir=self.temp.name))
        replacement = self.parent / "ai09-symlinklab"
        replacement.symlink_to(external, target_is_directory=True)
        with self.assertRaises(LabHold):
            self.check(lab=replacement)

    def test_05_foreign_file_is_not_deleted(self) -> None:
        sentinel = self.lab / "foreign-owner-file"
        sentinel.write_text("must survive", encoding="utf-8")
        with self.assertRaises(LabHold):
            self.check()
        self.assertEqual(sentinel.read_text(encoding="utf-8"), "must survive")

    def test_06_permissive_lab_mode_rejected(self) -> None:
        self.lab.chmod(0o755)
        with self.assertRaises(LabHold):
            self.check()

    def test_07_fresh_preflight_does_not_authenticate_prior_root(self) -> None:
        original_inode = self.lab.stat().st_ino
        original = self.parent / "ai09-originalbak"
        self.lab.rename(original)
        self.lab.mkdir(mode=0o700)
        self.assertNotEqual(self.lab.stat().st_ino, original_inode)
        self.assertEqual(self.check()[1], self.lab.stat().st_ino)
        # Preflight of a newly named pristine root cannot attest a prior
        # transaction's identity: a real installer must pin a long-lived FD.
        self.assertTrue(original.is_dir())

    def test_08_nonmatching_lab_namespace_rejected(self) -> None:
        other = self.parent / "other-install"
        other.mkdir(mode=0o700)
        with self.assertRaises(LabHold):
            self.check(lab=other)

    def test_09_runner_temp_symlink_rejected(self) -> None:
        alias = Path(self.temp.name) / "alias"
        alias.symlink_to(self.parent, target_is_directory=True)
        with self.assertRaises(LabHold):
            self.check(lab=alias / self.lab.name, parent=alias)

    def test_10_foreign_directory_is_preserved(self) -> None:
        foreign = self.lab / "unknown"
        foreign.mkdir()
        with self.assertRaises(LabHold):
            self.check()
        self.assertTrue(foreign.is_dir())


if __name__ == "__main__":
    unittest.main()
