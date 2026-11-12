"""Synthetic validation runner behavior, without running Cargo or OS installers."""
import importlib.util
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "validate_standalone_crates.py"
SPEC = importlib.util.spec_from_file_location("validate_standalone_crates", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ManifestRunnerTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")

    def add_manifest(self, name):
        path = self.root / MODULE.CRATES[name]
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("[package]\nname = 'fixture'\n", encoding="utf-8")
        return path

    def test_all_five_are_explicit_and_standalone(self):
        self.assertEqual(set(MODULE.CRATES),
                         {"registry", "market", "transport", "integrations", "desktop"})
        for p in MODULE.CRATES.values():
            self.assertNotEqual(p, "Cargo.toml")

    def test_plan_does_not_run_cargo_even_when_manifests_missing(self):
        with patch.object(MODULE.subprocess, "run") as run:
            self.assertEqual(MODULE.main(["--repo-root", str(self.root)]), 0)
            run.assert_not_called()

    def test_execute_rejects_missing_crates_without_partial_execution(self):
        self.add_manifest("registry")
        with patch.object(MODULE.subprocess, "run") as run:
            self.assertEqual(MODULE.main(["--repo-root", str(self.root), "--execute"]), 3)
            run.assert_not_called()

    def test_selected_crate_executes_three_offline_commands(self):
        self.add_manifest("market")
        with patch.object(MODULE.shutil, "which", return_value="/mock/cargo"), \
             patch.object(MODULE.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
            rc = MODULE.main(["--repo-root", str(self.root), "--only", "market", "--execute"])
            self.assertEqual(rc, 0)
            self.assertEqual(run.call_count, 3)
            cmds = [x.args[0] for x in run.call_args_list]
            self.assertEqual([x[1] for x in cmds], ["fmt", "test", "clippy"])
            self.assertTrue(all("--manifest-path" in x for x in cmds))
            self.assertNotIn("--offline", cmds[0])
            self.assertIn("--offline", cmds[1])
            self.assertIn("--offline", cmds[2])
            self.assertTrue(run.call_args_list[1].kwargs["env"]["CARGO_NET_OFFLINE"] == "true")

    def test_network_is_only_opted_in_explicitly(self):
        for cmd in MODULE.commands(MODULE.CRATES["transport"], allow_network=True):
            self.assertNotIn("--offline", cmd)
        for cmd in MODULE.commands(MODULE.CRATES["transport"])[1:]:
            self.assertIn("--offline", cmd)

    def test_timeout_fails_and_never_claims_pass(self):
        self.add_manifest("integrations")
        with patch.object(MODULE.shutil, "which", return_value="cargo"), \
             patch.object(MODULE.subprocess, "run",
                          side_effect=subprocess.TimeoutExpired(cmd="cargo", timeout=1)):
            self.assertEqual(MODULE.main(["--repo-root", str(self.root), "--only",
                                          "integrations", "--execute", "--timeout-seconds", "1"]), 124)

    def test_failure_stops_without_running_further_commands(self):
        self.add_manifest("registry")
        with patch.object(MODULE.shutil, "which", return_value="cargo"), \
             patch.object(MODULE.subprocess, "run", return_value=subprocess.CompletedProcess([], 7)) as run:
            self.assertEqual(MODULE.main(["--repo-root", str(self.root), "--only", "registry",
                                          "--execute"]), 7)
            run.assert_called_once()

    def test_symlink_outside_repo_is_rejected(self):
        with tempfile.TemporaryDirectory() as external:
            outside = Path(external) / "Cargo.toml"
            outside.write_text("[package]\n", encoding="utf-8")
            dest = self.root / MODULE.CRATES["desktop"]
            dest.parent.mkdir(parents=True)
            dest.symlink_to(outside)
            with self.assertRaises(ValueError):
                MODULE.inspect_manifests(self.root, ["desktop"])


if __name__ == "__main__":
    unittest.main()
