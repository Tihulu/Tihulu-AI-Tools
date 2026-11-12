#!/usr/bin/env python3
"""Observe a live two-command WebView→Tauri IPC result in throwaway CI only.

The IPC probe is TEST-ONLY frontend instrumentation added to a disposable
working checkout. No native commands, permission/CSP or production sources
are altered. Window-title evidence is intentionally limited to this Linux
WebKitGTK/Xvfb setup; not authenticated remote attestation.
"""
from __future__ import annotations

import argparse
import json
import re
import os
from pathlib import Path
import subprocess
import sys
import time

from phase6_native_launch_smoke import (UnsafeEnvironment, runner_root,
                                        check_owned_path, WINDOW_ID)

PASS_TITLE = "TIHULU_PHASE6_IPC_PASS"
FAIL_TITLE = "TIHULU_PHASE6_IPC_FAIL"
HEX_COLOR = re.compile(r"#[0-9A-Fa-f]{6}(?![0-9A-Fa-f])")


def sample_test_only_pixel(wid: str) -> str | None:
    """Read one private X11 window pixel; no filesystem/screenshots persisted.

    The JS probe draws a full-window green FAIL-CLOSED overlay only after it
    has successfully received and validated both unchanged Rust IPC replies.
    A failed/missing bridge produces solid red; anything else stays HOLD.
    """
    found = subprocess.run(
        ["import", "-window", wid, "-crop", "1x1+550+450", "-depth", "8", "txt:-"],
        capture_output=True, text=True, timeout=8, check=False,
    )
    if found.returncode != 0:
        return None
    values = [c.upper() for c in HEX_COLOR.findall(found.stdout)]
    if "#00FF00" in values:
        return "PASS"
    if "#FF0000" in values:
        return "FAIL"
    return None


def observed_status(process: subprocess.Popen[bytes]) -> tuple[str | None, str | None]:
    if process.poll() is not None:
        raise RuntimeError("Native process exited before live IPC observation")
    found = subprocess.run(["xdotool", "search", "--onlyvisible", "--name", "TIHULU"],
                           capture_output=True, text=True, timeout=5, check=False)
    if found.returncode not in (0, 1):
        raise RuntimeError("X11 visible-window query failed")
    for raw in found.stdout.splitlines():
        wid = raw.strip()
        if not WINDOW_ID.fullmatch(wid):
            continue
        owner = subprocess.run(["xdotool", "getwindowpid", wid],
                               capture_output=True, text=True, timeout=5, check=False)
        if owner.returncode == 0 and owner.stdout.strip().isdigit():
            if int(owner.stdout.strip()) != process.pid:
                continue
        title = subprocess.run(["xdotool", "getwindowname", wid],
                               capture_output=True, text=True, timeout=5, check=False)
        if title.returncode == 0:
            return wid, title.stdout.strip()
    return None, None


def observe(binary: Path, report: Path, root: Path, timeout: int) -> dict:
    check_owned_path(binary, root, must_exist=True)
    check_owned_path(report, root, must_exist=False)
    if not os.access(binary, os.X_OK) or not os.environ.get("DISPLAY"):
        raise UnsafeEnvironment("Private Xvfb display and executable required")
    if not 10 <= timeout <= 90:
        raise UnsafeEnvironment("Timeout out of allowed bounds")
    home = root / "phase6-ipc-private-home"
    for folder in ("", "config", "cache", "data", "state", "runtime"):
        (home / folder).mkdir(parents=True, exist_ok=True, mode=0o700)
    env = os.environ.copy()
    env.update({
        "HOME": str(home), "XDG_CONFIG_HOME": str(home / "config"),
        "XDG_CACHE_HOME": str(home / "cache"), "XDG_DATA_HOME": str(home / "data"),
        "XDG_STATE_HOME": str(home / "state"), "XDG_RUNTIME_DIR": str(home / "runtime"),
        "GDK_BACKEND": "x11", "LIBGL_ALWAYS_SOFTWARE": "1",
        "WEBKIT_DISABLE_COMPOSITING_MODE": "1",
        "WEBKIT_DISABLE_DMABUF_RENDERER": "1",
    })
    with (root / "phase6-live-ipc-stdout.log").open("wb") as log:
        process = subprocess.Popen([str(binary)], cwd=root, env=env,
                                   stdin=subprocess.DEVNULL,
                                   stdout=log, stderr=subprocess.STDOUT)
        try:
            deadline = time.monotonic() + timeout
            wid = None
            observed = ""
            marker = None
            while time.monotonic() < deadline:
                wid, observed = observed_status(process)
                if observed == FAIL_TITLE:
                    raise RuntimeError("Live WebView rejected the native IPC result")
                marker = sample_test_only_pixel(wid) if wid else None
                if marker == "FAIL":
                    raise RuntimeError("Live WebView returned a strict IPC FAIL marker")
                if marker == "PASS":
                    break
                time.sleep(1)
            if marker != "PASS" or not wid:
                raise RuntimeError("No validated WebView IPC visual PASS marker observed")
            close = subprocess.run(["xdotool", "windowclose", wid], capture_output=True,
                                   text=True, timeout=5, check=False)
            if close.returncode != 0:
                raise RuntimeError("Could not request native window close")
            try:
                exit_code = process.wait(timeout=20)
            except subprocess.TimeoutExpired as exc:
                raise RuntimeError("Native process failed to close") from exc
            if exit_code != 0:
                raise RuntimeError("Native process did not exit cleanly")
            result = {
                "schema_version": 1,
                "scope": "ubuntu-24.04-ephemeral-instrumented-webview",
                "live_webview_native_ipc_observed": True,
                "native_commands_validated": ["ui_backend_status", "ui_native_module_status"],
                "ipc_observation": "one_pixel_private_xvfb_test_overlay",
                "window_observed": True, "native_quit_observed": True,
                "original_native_source_changed": False,
                "ephemeral_test_frontend_injected": True,
                "installed_package_tested": False,
                "real_model_inference_tested": False,
                "real_gpu_npu_tested": False, "owner_machine_touched": False,
            }
            report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
            return result
        finally:
            if process.poll() is None:
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
    ap.add_argument("--timeout", type=int, default=65)
    args = ap.parse_args()
    try:
        result = observe(args.binary, args.report, runner_root(os.environ), args.timeout)
    except (UnsafeEnvironment, OSError, RuntimeError, subprocess.SubprocessError) as exc:
        print("PHASE6 LIVE IPC HOLD: " + str(exc), file=sys.stderr)
        return 2
    print("PHASE6 LIVE WEBVIEW↔RUST IPC PASS (ephemeral instrumented Ubuntu only)")
    print("No installed package, models, providers, GPU, or owner machine tested.")
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
