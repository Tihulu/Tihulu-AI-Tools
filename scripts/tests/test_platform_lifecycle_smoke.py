"""Synthetic tests of the developer-only platform supervisor (not real AI)."""
import importlib.util
import os
import sys
import threading
from pathlib import Path
import subprocess
import unittest


MODULE_PATH = Path(__file__).resolve().parents[1] / "platform_lifecycle_smoke.py"
SPEC = importlib.util.spec_from_file_location("platform_lifecycle_smoke", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class FakeChild:
    pid = 424242

    def __init__(self, stubborn=False):
        self.stubborn = stubborn
        self.exited = False
        self.wait_count = 0

    def poll(self):
        return 0 if self.exited else None

    def wait(self, timeout):
        self.wait_count += 1
        if self.stubborn and self.wait_count == 1:
            raise subprocess.TimeoutExpired(cmd="fake-owned-only", timeout=timeout)
        self.exited = True
        return 0


class SpySupervisor(MODULE.OwnedDummySupervisor):
    def __init__(self, fake):
        super().__init__()
        self._owned = fake
        self.events = []

    def _graceful(self, proc):
        self.events.append(("graceful", proc.pid))

    def _force(self, proc):
        self.events.append(("force", proc.pid))


class SyntheticLifecycleTests(unittest.TestCase):
    def test_l01_owned_reaped_borrowed_preserved(self):
        child = FakeChild()
        s = SpySupervisor(child)
        s.record_borrowed_label("Jan")
        s.record_borrowed_label("readsb")
        report = s.stop()
        self.assertTrue(report["owned_reaped"])
        self.assertEqual(s.events, [("graceful", child.pid)])
        self.assertEqual(report["borrowed_labels_untouched"], ("Jan", "readsb"))

    def test_l08_non_cooperative_escalates_owned_only(self):
        child = FakeChild(stubborn=True)
        s = SpySupervisor(child)
        s.record_borrowed_label("Ollama")
        result = s.stop(grace_seconds=0)
        self.assertTrue(result["escalated"])
        self.assertTrue(result["owned_reaped"])
        self.assertEqual(s.events, [("graceful", child.pid), ("force", child.pid)])

    def test_l11_shutdown_idempotent_refuses_restart(self):
        s = SpySupervisor(FakeChild())
        first = s.stop()
        second = s.stop()
        self.assertEqual(first, second)
        self.assertEqual(len(s.events), 1)
        with self.assertRaises(RuntimeError):
            s.spawn_owned_dummy()

    def test_negative_grace_fails_before_state_change(self):
        s = SpySupervisor(FakeChild())
        with self.assertRaises(ValueError):
            s.stop(-1)
        self.assertEqual(s.state, "running")

    def test_cleanup_failure_requires_retry_and_is_never_reported_stopped(self):
        class RecoverableChild(FakeChild):
            def wait(self, timeout):
                self.wait_count += 1
                if self.wait_count <= 2:
                    raise subprocess.TimeoutExpired(cmd="fake-owned", timeout=timeout)
                self.exited = True
                return 0

        s = SpySupervisor(RecoverableChild())
        first = s.stop(grace_seconds=0)
        self.assertFalse(first["owned_reaped"])
        self.assertEqual(s.state, "cleanup_failed")
        with self.assertRaises(RuntimeError):
            s.spawn_owned_dummy()
        second = s.stop(grace_seconds=0)
        self.assertTrue(second["owned_reaped"])
        self.assertEqual(s.state, "stopped")

    def test_owned_process_is_isolated_group_on_posix(self):
        if os.name == "nt":
            self.skipTest("Windows direct handle does not establish POSIX process group")
        s = MODULE.OwnedDummySupervisor()
        try:
            pid = s.spawn_owned_dummy()
            self.assertEqual(os.getpgid(pid), pid)
        finally:
            self.assertTrue(s.stop()["owned_reaped"])

    def test_two_simultaneous_stop_requests_are_idempotent(self):
        s = SpySupervisor(FakeChild())
        results = []
        threads = [threading.Thread(target=lambda: results.append(s.stop()))
                   for _ in range(2)]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(len(results), 2)
        self.assertEqual(results[0], results[1])
        self.assertEqual(len(s.events), 1)

    def test_stopping_owned_dummy_does_not_stop_untracked_dummy(self):
        import subprocess
        foreign_test_dummy = subprocess.Popen(
            [sys.executable, "-c", "import time; time.sleep(15)"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            stdin=subprocess.DEVNULL)
        supervisor = MODULE.OwnedDummySupervisor()
        try:
            supervisor.spawn_owned_dummy()
            self.assertTrue(supervisor.stop()["owned_reaped"])
            self.assertIsNone(foreign_test_dummy.poll())
        finally:
            supervisor.stop()
            if foreign_test_dummy.poll() is None:
                foreign_test_dummy.terminate()
            try:
                foreign_test_dummy.wait(timeout=2)
            except subprocess.TimeoutExpired:
                foreign_test_dummy.kill()
                foreign_test_dummy.wait(timeout=2)

    def test_l14_os_owned_dummy_reaped(self):
        s = MODULE.OwnedDummySupervisor()
        s.record_borrowed_label("fake-independent-host")
        try:
            s.spawn_owned_dummy()
            result = s.stop(grace_seconds=2)
            self.assertTrue(result["owned_reaped"])
            self.assertEqual(result["borrowed_labels_untouched"],
                             ("fake-independent-host",))
        finally:
            s.stop()


if __name__ == "__main__":
    unittest.main()
