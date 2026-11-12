#!/usr/bin/env python3
"""Development-only owned dummy-child cleanup harness. NOT production supervision.

Only ever spawns its own Python sleeping child, when --owned-child-smoke is passed.
Does not contact services, models, GPU, NPU or SDR devices. On Windows it validates
a direct child handle only, not Job Objects or an owned descendant process tree.
"""
import argparse
import os
import signal
import subprocess
import sys
import threading


class OwnedDummySupervisor:
    def __init__(self):
        self._lock = threading.RLock()
        self._state = "running"
        self._owned = None
        self._borrowed_names = []
        self._result = None

    @property
    def state(self):
        with self._lock:
            return self._state

    def record_borrowed_label(self, label):
        """Metadata only; never resolves or signals an external process."""
        with self._lock:
            self._borrowed_names.append(label)

    def spawn_owned_dummy(self):
        with self._lock:
            if self._state != "running" or self._owned is not None:
                raise RuntimeError("No new work after Stop, or child already owned")
            opts = {"stdin": subprocess.DEVNULL,
                    "stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL}
            if os.name == "nt":
                opts["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP
            else:
                opts["start_new_session"] = True
            self._owned = subprocess.Popen(
                [sys.executable, "-c", "import time; time.sleep(30)"], **opts
            )
            return self._owned.pid

    def _graceful(self, proc):
        if proc.poll() is not None:
            return
        if os.name == "nt":
            proc.terminate()  # Direct child only; no Job Object in this harness.
        else:
            os.killpg(proc.pid, signal.SIGTERM)  # Our newly isolated group only.

    def _force(self, proc):
        if proc.poll() is not None:
            return
        if os.name == "nt":
            proc.kill()
        else:
            os.killpg(proc.pid, signal.SIGKILL)

    def stop(self, grace_seconds=1.0):
        """Idempotent, bounded teardown of the retained owned handle only."""
        if grace_seconds < 0:
            raise ValueError("grace_seconds must be nonnegative")
        with self._lock:
            if self._state == "stopped":
                return dict(self._result)
            self._state = "stopping"
            proc = self._owned
            escalated = False
            reaped = True
            if proc is not None:
                try:
                    self._graceful(proc)
                    try:
                        proc.wait(timeout=grace_seconds)
                    except subprocess.TimeoutExpired:
                        escalated = True
                        self._force(proc)
                        proc.wait(timeout=2.0)
                except (ProcessLookupError, PermissionError):
                    # Owner identity / permissions cannot be silently guessed.
                    reaped = False
                except subprocess.TimeoutExpired:
                    reaped = False
                if proc.poll() is None:
                    reaped = False
            self._owned = None if reaped else proc
            self._result = {
                "owned_reaped": reaped,
                "escalated": escalated,
                "borrowed_labels_untouched": tuple(self._borrowed_names),
            }
            # Retain a failed handle for a safe retry; never report a leak as stopped.
            self._state = "stopped" if reaped else "cleanup_failed"
            return dict(self._result)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--owned-child-smoke", action="store_true",
                        help="Opt in to spawning/terminating one Python dummy child")
    args = parser.parse_args()
    if not args.owned_child_smoke:
        print("Dry run only. Use --owned-child-smoke for an owned dummy child.")
        return 0
    supervisor = OwnedDummySupervisor()
    supervisor.record_borrowed_label("mock-Jan-not-a-PID")
    try:
        supervisor.spawn_owned_dummy()
        report = supervisor.stop()
        print("owned dummy reaped:", report["owned_reaped"])
        print("borrowed metadata unchanged:", report["borrowed_labels_untouched"])
        return 0 if report["owned_reaped"] else 1
    finally:
        # In case of an assertion or interrupted test, retry this exact handle.
        supervisor.stop()


if __name__ == "__main__":
    sys.exit(main())
