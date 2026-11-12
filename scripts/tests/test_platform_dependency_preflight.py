"""Dependency matrix: unit tests use mocks, never install or inspect hardware."""
import importlib.util
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).resolve().parents[1] / "platform_dependency_preflight.py"
SPEC = importlib.util.spec_from_file_location("platform_dependency_preflight", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class PlatformDependencyTests(unittest.TestCase):
    def test_seven_targets_include_pi_and_arm_windows(self):
        self.assertEqual(len(MODULE.TARGETS), 7)
        self.assertIn("pi5-hailo10h", MODULE.TARGETS)
        self.assertIn("windows-arm64", MODULE.TARGETS)

    def test_macos_arm_aliases_and_native_windows_emulation_boundary(self):
        self.assertEqual(MODULE.architecture("aarch64"), "arm64")
        self.assertFalse(MODULE.host_matches("windows-arm64", "Windows", "AMD64"))
        self.assertTrue(MODULE.host_matches("windows-arm64", "Windows", "aarch64"))

    def test_plan_is_not_a_real_probe(self):
        with patch.object(MODULE, "probe") as p:
            rc = MODULE.main(["--target", "pi5-hailo10h"])
            self.assertEqual(rc, 0)
            p.assert_not_called()

    def test_mismatch_fails_without_os_commands(self):
        with patch.object(MODULE.subprocess, "run") as run:
            rc = MODULE.probe("windows-arm64", "Linux", "x86_64")
            self.assertEqual(rc, 3)
            run.assert_not_called()

    def test_linux_missing_libraries_does_not_claim_acceptance(self):
        def which(name):
            return "/fake/" + name
        with patch.object(MODULE.shutil, "which", side_effect=which), \
             patch.object(MODULE.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "missing")) as run:
            rc = MODULE.probe("linux-arm64", "Linux", "aarch64")
            self.assertEqual(rc, 4)
            self.assertEqual(run.call_count, len(MODULE.LINUX_PKGCONFIG))
            self.assertTrue(all(x.args[0][0] == "pkg-config" for x in run.call_args_list))


if __name__ == "__main__":
    unittest.main()
