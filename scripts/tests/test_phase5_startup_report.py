import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

FILE = Path(__file__).resolve().parents[1] / 'phase5_startup_report.py'
SPEC = importlib.util.spec_from_file_location('phase5_startup_report', FILE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class StartupEvidenceTests(unittest.TestCase):
    def example(self):
        return dict(schema_version=1, platform='linux-x64', observer='native-runner-operator',
                    gui_window_observed=True, native_ipc_observed=True,
                    external_services_untouched=True, native_process_exit_observed=True,
                    ui_native_module_status=dict(schema_version=1,
                         source='native_in_memory_read_only',
                         linked_modules=MODULE.EXPECTED_MODULES[:], modules_compiled=True,
                         registry_records=0, gui_owner_registered=False,
                         approved_source_scans=0, backend_verified=False,
                         jan_connected=False, gpu_connected=False, cleanup_verified=False))

    def test_complete_mock_report_schema(self):
        self.assertTrue(MODULE.validate_report(self.example())[0])

    def test_reject_false_cleanup_or_gpu_state(self):
        for key in ('backend_verified', 'gpu_connected', 'jan_connected', 'cleanup_verified'):
            x = self.example()
            x['ui_native_module_status'][key] = True
            self.assertFalse(MODULE.validate_report(x)[0], key)

    def test_missing_observation_rejected(self):
        for key in ('gui_window_observed', 'native_ipc_observed',
                    'external_services_untouched', 'native_process_exit_observed'):
            x = self.example()
            x[key] = False
            self.assertFalse(MODULE.validate_report(x)[0])

    def test_report_parser_does_not_launch_program(self):
        with tempfile.TemporaryDirectory() as d:
            file = Path(d) / 'report.json'
            file.write_text(json.dumps(self.example()), encoding='utf-8')
            self.assertEqual(MODULE.main(['--report', str(file)]), 0)

    def test_extra_field_rejected(self):
        x = self.example()
        x['actual_vram_released'] = True
        self.assertFalse(MODULE.validate_report(x)[0])

    def test_symlink_report_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            file = Path(d) / 'report.json'
            file.write_text(json.dumps(self.example()), encoding='utf-8')
            link = Path(d) / 'link.json'
            try: link.symlink_to(file)
            except OSError: self.skipTest('No symlink privileges')
            self.assertEqual(MODULE.main(['--report', str(link)]), 2)


if __name__ == '__main__':
    unittest.main()
