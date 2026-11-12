"""Safety negatives for CI-only Phase 6 real GUI observation."""
from pathlib import Path
import os
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from phase6_native_launch_smoke import (UnsafeEnvironment, check_owned_path,
                                        runner_root, WINDOW_ID)


class Phase6GuardTests(unittest.TestCase):
    def test_reject_owner_workstation_and_missing_ci_marker(self):
        with tempfile.TemporaryDirectory() as d:
            for env in (
                {"RUNNER_TEMP": d},
                {"CI": "true", "RUNNER_TEMP": d},
                {"GITHUB_ACTIONS": "true", "RUNNER_TEMP": d},
                {"CI": "true", "GITHUB_ACTIONS": "true"},
            ):
                with self.subTest(env=env), self.assertRaises(UnsafeEnvironment):
                    runner_root(env)

    def test_reject_relative_runner_location(self):
        with self.assertRaises(UnsafeEnvironment):
            runner_root({"CI": "true", "GITHUB_ACTIONS": "true", "RUNNER_TEMP": "."})

    def test_allow_disposable_ci_root(self):
        with tempfile.TemporaryDirectory() as d:
            root = runner_root({"CI": "true", "GITHUB_ACTIONS": "true",
                                "RUNNER_TEMP": d})
            self.assertEqual(root, Path(d).resolve())

    def test_only_absolute_owned_paths(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d).resolve()
            binary = root / "program"
            binary.write_bytes(b"not executed")
            self.assertEqual(check_owned_path(binary, root, must_exist=True), binary)
            for path in (Path("program"), root / ".." / "escape",
                         root.parent / "outside"):
                with self.subTest(path=path), self.assertRaises(UnsafeEnvironment):
                    check_owned_path(path, root, must_exist=False)

    def test_reject_symlink_binary_and_parent(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d).resolve()
            safe = root / "safe"
            safe.mkdir()
            (safe / "file").write_bytes(b"dummy")
            (root / "alias").symlink_to(safe, target_is_directory=True)
            (root / "symlink-file").symlink_to(safe / "file")
            for path in (root / "alias" / "file", root / "symlink-file"):
                with self.subTest(path=path), self.assertRaises(UnsafeEnvironment):
                    check_owned_path(path, root, must_exist=True)

    def test_missing_executable_refused(self):
        with tempfile.TemporaryDirectory() as d:
            root = Path(d).resolve()
            with self.assertRaises(UnsafeEnvironment):
                check_owned_path(root / "missing", root, must_exist=True)

    def test_reject_untrusted_x11_window_identifiers(self):
        for item in ("0x9;systemctl stop readsb", "garbage", "-1", ""):
            self.assertIsNone(WINDOW_ID.fullmatch(item))
        for item in ("0x123ab", "4194304"):
            self.assertIsNotNone(WINDOW_ID.fullmatch(item))


if __name__ == "__main__":
    unittest.main()
