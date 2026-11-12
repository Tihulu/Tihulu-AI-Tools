#!/usr/bin/env python3
"""CI-only observation of the *real* frozen Tauri demo window on private Xvfb.

This deliberately proves no model/provider/GPU, native IPC response, service
lifecycle, signed installer, or real owner-machine coexistence. The binary must
have been copied to an ephemeral runner directory, not a user installation.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import time

APP_TITLE = "TIHULU AI Tools"
WINDOW_ID = re.compile(r"^(?:0x[0-9a-fA-F]+|[0-9]+)$")


class UnsafeEnvironment(RuntimeError):
    """Fail closed instead of running on a user's real workstation."""


def runner_root(env: dict[str, str]) -> Path:
    if env.get("CI") != "true" or env.get("GITHUB_ACTIONS") != "true":
        raise UnsafeEnvironment("GitHub Actions disposable runner is required")
    raw = env.get("RUNNER_TEMP", "")
    if not raw or not Path(raw).is_absolute():
        raise UnsafeEnvironment("Absolute RUNNER_TEMP is required")
    root = Path(raw)
    if root.is_symlink() or not root.is_dir():
        raise UnsafeEnvironment("RUNNER_TEMP must be an existing real directory")
    root = root.resolve(strict=True)
    if root == Path("/") or root == Path.home().resolve():
        raise UnsafeEnvironment("RUNNER_TEMP cannot be / or user HOME")
    return root


def check_owned_path(path: Path, root: Path, *, must_exist: bool) -> Path:
    if not path.is_absolute() or ".." in path.parts:
        raise UnsafeEnvironment("Relative/traversing path refused")
    try:
        path.relative_to(root)
    except ValueError as exc:
        raise UnsafeEnvironment("Path outside disposable RUNNER_TEMP") from exc
    ancestor = path
    while ancestor != root:
        if ancestor.is_symlink():
            raise UnsafeEnvironment("Symlink in disposable path refused")
        ancestor = ancestor.parent
    if must_exist and not path.is_file():
        raise UnsafeEnvironment("Disposable binary does not exist")
    return path


def find_app_window(process: subprocess.Popen[bytes]) -> str | None:
    if process.poll() is not None:
        raise RuntimeError("Tauri native process exited before visible window")
    found = subprocess.run(
        ["xdotool", "search", "--onlyvisible", "--name", APP_TITLE],
        capture_output=True, text=True, check=False, timeout=5,
    )
    if found.returncode not in (0, 1):
        raise RuntimeError("X11 window query failed")
    for line in found.stdout.splitlines():
        window_id = line.strip()
        if not WINDOW_ID.fullmatch(window_id):
            continue
        binding = subprocess.run(
            ["xdotool", "getwindowpid", window_id],
            capture_output=True, text=True, check=False, timeout=5,
        )
        # On a private Xvfb display no foreign app should have this title.
        # If _NET_WM_PID is present it MUST match our launched process.
        if binding.returncode == 0 and binding.stdout.strip().isdigit():
            if int(binding.stdout.strip()) != process.pid:
                continue
        return window_id
    return None


def observe(binary: Path, report: Path, root: Path, timeout: int) -> dict:
    check_owned_path(binary, root, must_exist=True)
    check_owned_path(report, root, must_exist=False)
    if not os.access(binary, os.X_OK) or not os.environ.get("DISPLAY"):
        raise UnsafeEnvironment("Executable binary and private Xvfb DISPLAY required")
    if not 10 <= timeout <= 90:
        raise UnsafeEnvironment("Timeout must be between 10 and 90 seconds")

    home = root / "phase6-private-home"
    home.mkdir(mode=0o700, exist_ok=True)
    for name in ("config", "cache", "data", "state", "runtime"):
        (home / name).mkdir(mode=0o700, exist_ok=True)

    env = os.environ.copy()
    env.update({
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / "config"),
        "XDG_CACHE_HOME": str(home / "cache"),
        "XDG_DATA_HOME": str(home / "data"),
        "XDG_STATE_HOME": str(home / "state"),
        "XDG_RUNTIME_DIR": str(home / "runtime"),
        "GDK_BACKEND": "x11",
        "LIBGL_ALWAYS_SOFTWARE": "1",
        "WEBKIT_DISABLE_COMPOSITING_MODE": "1",
        "WEBKIT_DISABLE_DMABUF_RENDERER": "1",
    })

    # Never use a shell, sudo, process-name kill or service-control command.
    with (root / "phase6-native-stderr.log").open("wb") as log:
        process = subprocess.Popen([str(binary)], cwd=root, env=env,
                                   stdin=subprocess.DEVNULL,
                                   stdout=log, stderr=subprocess.STDOUT)
        window_id = None
        graceful_exit = False
        try:
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                window_id = find_app_window(process)
                if window_id:
                    break
                time.sleep(1)
            if not window_id:
                raise RuntimeError("No visible real native Tauri window observed")
            close = subprocess.run(
                ["xdotool", "windowclose", window_id],
                capture_output=True, text=True, check=False, timeout=5,
            )
            if close.returncode != 0:
                raise RuntimeError("X11 graceful window close request failed")
            try:
                process.wait(timeout=20)
                graceful_exit = True
            except subprocess.TimeoutExpired as exc:
                raise RuntimeError("GUI did not exit after windowclose") from exc
            result = {
                "schema_version": 1,
                "scope": "ubuntu-24.04-private-xvfb-observation",
                "checkout_sha": os.environ.get("PHASE6_CHECKOUT_SHA", ""),
                "window_observed": True,
                "native_process_exited": graceful_exit,
                "native_ipc_observed": False,
                "installed_package_tested": False,
                "real_models_or_providers_tested": False,
                "physical_gpu_npu_tested": False,
                "owner_machine_touched": False,
            }
            report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            return result
        finally:
            if process.poll() is None:
                # Only the exact private child PID started by this test.
                process.terminate()
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--binary", type=Path, required=True)
    ap.add_argument("--report", type=Path, required=True)
    ap.add_argument("--timeout", type=int, default=55)
    args = ap.parse_args()
    try:
        root = runner_root(os.environ)
        result = observe(args.binary, args.report, root, args.timeout)
    except (UnsafeEnvironment, RuntimeError, OSError, subprocess.SubprocessError) as exc:
        print("PHASE6 NATIVE GUI HOLD: " + str(exc), file=sys.stderr)
        return 2
    print("PHASE6 REAL GUI WINDOW AND EXIT: PASS (private Ubuntu Xvfb only)")
    print("IPC/inference/GPU/installer/owner-machine acceptance: NOT TESTED")
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
