"""Mock lifecycle acceptance. No GPU, RAM allocation, Jan or external service."""
import importlib.util
from pathlib import Path
import unittest

FILE = Path(__file__).resolve().parents[1] / 'phase5_lifecycle_contract.py'
SPEC = importlib.util.spec_from_file_location('phase5_lifecycle_contract', FILE)
MODULE = importlib.util.module_from_spec(SPEC)
import sys
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class OwnershipContractTests(unittest.TestCase):
    def make_host(self):
        host = MODULE.MockRuntime()
        host.clients.update({'gui', 'media', 'satellite'})
        host.enable_independent_host(explicit_consent=True)
        return host

    def test_background_is_off_without_explicit_host_optin(self):
        host = MODULE.MockRuntime()
        self.assertFalse(host.host_running)
        self.assertFalse(host.independent_host_opt_in)
        with self.assertRaises(PermissionError): host.enable_independent_host()
        self.assertFalse(host.host_running)

    def test_gui_quit_does_not_stop_independent_host_other_client(self):
        host = self.make_host()
        host.acquire('gui', 'llm')
        host.acquire('media', 'llm')
        host.acquire('satellite', 'speech')
        host.register_worker('gui', 'owned-gui-worker', 128, 256)
        host.register_worker('media', 'owned-media-worker', 256, 512)
        host.quit_gui()
        self.assertFalse(host.gui_running)
        self.assertTrue(host.host_running)
        self.assertEqual(host.leases['llm'], {'media'})
        self.assertEqual(host.leases['speech'], {'satellite'})
        self.assertEqual(host.released_workers, [('gui', 'owned-gui-worker')])
        self.assertEqual(host.snapshot()['mock_owner_vram_mb'], 512)
        self.assertTrue(host.snapshot()['borrowed_untouched'])

    def test_last_model_lease_unloads_once_without_killing_host(self):
        host = self.make_host()
        host.acquire('gui', 'llm')
        host.acquire('media', 'llm')
        host.quit_gui()
        self.assertNotIn('llm', host.unloaded)
        host.stop_client('media')
        host.stop_client('media')
        self.assertEqual(host.unloaded, ['llm'])
        self.assertTrue(host.host_running)

    def test_explicit_host_shutdown_requires_confirmation(self):
        host = self.make_host()
        host.acquire('media', 'llm')
        with self.assertRaises(PermissionError): host.stop_host()
        self.assertTrue(host.host_running)
        host.stop_host(consent=True)
        self.assertFalse(host.host_running)
        self.assertEqual(host.unloaded, ['llm'])
        self.assertTrue(host.snapshot()['borrowed_untouched'])

    def test_borrowed_services_never_claimed_or_terminated(self):
        host = self.make_host()
        for name in MODULE.BORROWED:
            with self.assertRaises(PermissionError): host.register_worker(name, 'foreign-handle')
        host.stop_host(consent=True)
        self.assertEqual(host.borrowed_ops, 0)
        self.assertEqual(host.borrowed, frozenset({'Jan', 'Ollama', 'readsb', 'tar1090'}))

    def test_app_stop_does_not_evict_unrelated_model(self):
        host = self.make_host()
        host.acquire('media', 'speech')
        host.acquire('satellite', 'vision')
        host.stop_client('media')
        self.assertEqual(host.unloaded, ['speech'])
        self.assertEqual(host.leases['vision'], {'satellite'})

    def test_mock_resource_cleanup_is_owner_attributed_not_global_gpu(self):
        host = self.make_host()
        host.register_worker('gui', 'worker-1', 128, 256)
        host.register_worker('media', 'worker-2', 512, 1024)
        host.quit_gui()
        self.assertEqual(host.snapshot()['mock_owner_ram_mb'], 512)
        self.assertEqual(host.snapshot()['mock_owner_vram_mb'], 1024)
        self.assertFalse(host.snapshot()['physical_gpu_verified'])
        self.assertFalse(host.snapshot()['physical_memory_verified'])

    def test_no_new_gui_job_after_exit(self):
        host = self.make_host()
        host.quit_gui()
        with self.assertRaises(RuntimeError): host.acquire('gui', 'llm')

    def test_idempotent_quit(self):
        host = self.make_host()
        host.acquire('gui', 'llm')
        host.quit_gui()
        host.quit_gui()
        self.assertEqual(host.unloaded, ['llm'])

    def test_embedded_host_not_independent_of_gui(self):
        host = MODULE.MockRuntime(host_running=True)
        host.clients.add('gui')
        host.acquire('gui', 'local')
        host.quit_gui()
        self.assertFalse(host.host_running)
        self.assertEqual(host.unloaded, ['local'])


if __name__ == '__main__':
    unittest.main()
