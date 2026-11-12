"""Phase5 platform smoke contracts — no installation and no real Cargo actions."""
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

MODULE_FILE = Path(__file__).resolve().parents[1] / 'phase5_platform_smoke.py'
SPEC = importlib.util.spec_from_file_location('phase5_platform_smoke', MODULE_FILE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class CheckoutFixture(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.mk_checkout()

    def write(self, rel, data):
        path = self.root / rel
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(data, encoding='utf-8')

    def mk_checkout(self):
        members = '\n'.join(f'"{m}",' for m in sorted(MODULE.EXPECTED_MEMBERS))
        self.write('Cargo.toml', '[workspace]\nmembers=[' + members + ']\nexclude=["apps/tihulu-ai-desktop/src-tauri"]\n')
        for path in MODULE.EXPECTED_MEMBERS:
            self.write(path + '/Cargo.toml', '[package]\nname="fixture"\n')
        self.write(str(MODULE.NATIVE_MANIFEST), '[package]\nname="tihulu-ai-desktop"\n')
        self.write(str(MODULE.NATIVE_CONFIG), json.dumps({'bundle': {'active': False}, 'build': {'frontendDist': '../../tihulu-ai-web'}}))
        self.write('apps/tihulu-ai-web/package.json', '{"private":true}')
        self.write('apps/tihulu-ai-web/tests/mock.test.mjs', 'import test from "node:test";test("fixture",()=>{});')


class PlatformSmokeTests(CheckoutFixture):
    def test_native_targets_match_real_arches(self):
        self.assertEqual(len(MODULE.TARGETS), 5)
        self.assertTrue(MODULE.host_matches('windows-arm64', 'Windows', 'ARM64'))
        self.assertFalse(MODULE.host_matches('windows-arm64', 'Windows', 'AMD64'))
        self.assertTrue(MODULE.host_matches('linux-arm64', 'Linux', 'aarch64'))
        self.assertTrue(MODULE.host_matches('macos-arm64', 'Darwin', 'arm64'))

    def test_safe_plan_requires_tauri_excluded_and_bundling_off(self):
        report = MODULE.inspect_checkout(self.root)
        self.assertTrue(report['bundle_disabled'])
        self.assertEqual(len(report['web_tests']), 1)
        commands = MODULE.native_build_commands(report['web_tests'], self.root)
        self.assertEqual([c[1] for c in commands[:4]], ['fmt', 'test', 'clippy', 'build'])
        self.assertTrue(all('bundle' not in c for c in commands))
        self.assertEqual(commands[-1][0:2], ['node', '--test'])
        self.assertIn('--offline', commands[1])
        self.assertIn('--offline', commands[3])
        self.assertNotIn('--offline', commands[0])

    def test_plan_does_not_execute_any_command(self):
        with patch.object(MODULE.subprocess, 'run') as mock:
            result = MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64'])
        self.assertEqual(result, 0)
        mock.assert_not_called()

    def test_unexpected_bundle_activation_rejected_even_for_plan(self):
        self.write(str(MODULE.NATIVE_CONFIG), json.dumps({'bundle': {'active': True}, 'build': {'frontendDist': '../../tihulu-ai-web'}}))
        with patch.object(MODULE.subprocess, 'run') as mock:
            self.assertEqual(MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64']), 2)
        mock.assert_not_called()

    def test_incomplete_membership_rejected(self):
        self.write('Cargo.toml', '[workspace]\nmembers=[]\nexclude=["apps/tihulu-ai-desktop/src-tauri"]\n')
        with patch.object(MODULE.subprocess, 'run') as mock:
            self.assertEqual(MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64']), 2)
        mock.assert_not_called()

    def test_no_bundle_manifest_or_symlink_escape(self):
        manifest = self.root / MODULE.NATIVE_MANIFEST
        manifest.unlink()
        with tempfile.TemporaryDirectory() as tmp:
            outside = Path(tmp) / 'Cargo.toml'
            outside.write_text('[package]\nname="tihulu-ai-desktop"\n')
            try:
                manifest.symlink_to(outside)
            except OSError:
                self.skipTest('Symlinks unavailable on this host')
            with self.assertRaises(ValueError):
                MODULE.inspect_checkout(self.root)

    def test_execute_requires_native_machine_before_attempting_cargo(self):
        with patch.object(MODULE, 'host_matches', return_value=False), \
             patch.object(MODULE.subprocess, 'run') as mock:
            result = MODULE.main(['--repo-root', str(self.root), '--target', 'windows-arm64', '--execute-build'])
        self.assertEqual(result, 3)
        mock.assert_not_called()

    def test_execute_requires_cargo_and_node(self):
        with patch.object(MODULE, 'host_matches', return_value=True), \
             patch.object(MODULE.shutil, 'which', return_value=None), \
             patch.object(MODULE.subprocess, 'run') as mock:
            result = MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64', '--execute-build'])
        self.assertEqual(result, 4)
        mock.assert_not_called()

    def test_mock_executed_build_stops_after_first_failure(self):
        with patch.object(MODULE, 'host_matches', return_value=True), \
             patch.object(MODULE.shutil, 'which', return_value='/mock/cargo'), \
             patch.object(MODULE.subprocess, 'run', return_value=subprocess.CompletedProcess([], 8)) as mock:
            result = MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64', '--execute-build'])
        self.assertEqual(result, 8)
        mock.assert_called_once()
        self.assertEqual(mock.call_args.args[0][1], 'fmt')

    def test_mock_executed_build_runs_five_checks_only(self):
        with patch.object(MODULE, 'host_matches', return_value=True), \
             patch.object(MODULE.shutil, 'which', return_value='/mock/cargo'), \
             patch.object(MODULE.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0)) as mock:
            result = MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64', '--execute-build'])
        self.assertEqual(result, 0)
        self.assertEqual(mock.call_count, 5)
        self.assertTrue(all(call.kwargs['env']['CARGO_NET_OFFLINE'] == 'true' for call in mock.call_args_list))
        self.assertTrue(all(call.kwargs['cwd'] == self.root for call in mock.call_args_list))

    def test_network_is_separate_explicit_optin(self):
        with self.assertRaises(SystemExit):
            MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64', '--allow-network'])
        commands = MODULE.native_build_commands([], self.root, allow_network=True)
        self.assertNotIn('--offline', commands[1])
        self.assertNotIn('--offline', commands[3])

    def test_timeout_does_not_claim_build_passed(self):
        with patch.object(MODULE, 'host_matches', return_value=True), \
             patch.object(MODULE.shutil, 'which', return_value='/mock/cargo'), \
             patch.object(MODULE.subprocess, 'run', side_effect=subprocess.TimeoutExpired(['cargo'], 1)):
            self.assertEqual(MODULE.main(['--repo-root', str(self.root), '--target', 'linux-x64', '--execute-build', '--timeout-seconds', '1']), 124)

    def test_package_hash_never_executes_or_verifies_signature(self):
        path = self.root / 'mock.msi'
        path.write_bytes(b'not an actual package')
        with patch.object(MODULE.subprocess, 'run') as mock:
            report = MODULE.artifact_report(path)
            result = MODULE.main(['--target', 'windows-x64', '--inspect-artifact', str(path)])
        self.assertEqual(result, 0)
        self.assertFalse(report['signature_verified'])
        self.assertFalse(report['installer_executed'])
        self.assertFalse(report['launch_tested'])
        self.assertEqual(len(report['sha256']), 64)
        mock.assert_not_called()

    def test_package_rejects_symlinks_empty_and_unrecognized(self):
        path = self.root / 'empty.msi'
        path.write_bytes(b'')
        with self.assertRaises(ValueError): MODULE.artifact_report(path)
        path = self.root / 'installer.sh'
        path.write_bytes(b'#!/bin/sh\necho hi')
        with self.assertRaises(ValueError): MODULE.artifact_report(path)
        target = self.root / 'artifact.pkg'
        target.write_bytes(b'fake')
        symlink = self.root / 'linked.pkg'
        try:
            symlink.symlink_to(target)
        except OSError:
            self.skipTest('Symlinks unavailable')
        with self.assertRaises(ValueError): MODULE.artifact_report(symlink)


if __name__ == '__main__':
    unittest.main()


class MacBundleReadOnlyTests(unittest.TestCase):
    def test_minimal_app_tree_report_unverified(self):
        import plistlib
        with tempfile.TemporaryDirectory() as d:
            app = Path(d) / 'Safe.app'
            contents = app / 'Contents'
            (contents / 'MacOS').mkdir(parents=True)
            (contents / 'Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable': 'safe'}))
            (contents / 'MacOS' / 'safe').write_bytes(b'not a program')
            report = MODULE.artifact_report(app)
            self.assertEqual(report['kind'], 'app_directory')
            self.assertEqual(report['files'], 2)
            self.assertFalse(report['signature_verified'])
            self.assertFalse(report['launch_tested'])

    def test_app_escape_symlink_fails_closed(self):
        import plistlib
        with tempfile.TemporaryDirectory() as d:
            app = Path(d) / 'Safe.app'
            contents = app / 'Contents'
            (contents / 'MacOS').mkdir(parents=True)
            (contents / 'Info.plist').write_bytes(plistlib.dumps({'CFBundleExecutable': 'safe'}))
            (contents / 'MacOS' / 'safe').write_bytes(b'dummy')
            link = contents / 'Escape'
            try: link.symlink_to(Path(d) / 'outside')
            except OSError: self.skipTest('No symlink privileges')
            with self.assertRaises(ValueError): MODULE.artifact_report(app)
