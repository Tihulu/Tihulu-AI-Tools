#!/usr/bin/env python3
"""Prepare a temporary CI-only test probe in a disposable GitHub checkout.

The production Git tree remains immutable: this intentionally modifies only
uncommitted runner working-copy files before a test build, never a PR source.
"""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import subprocess
import sys

FROZEN = "48669240e4595908363f48f4229e70f55e72d7db"
INDEX = "apps/tihulu-ai-web/index.html"
TARGET = "apps/tihulu-ai-web/phase6-ipc-probe.mjs"
SOURCE = "qa/phase6/live-ipc-probe.mjs"
INJECT = '  <script type="module" src="./phase6-ipc-probe.mjs"></script>\n'


def prepare() -> None:
    if os.environ.get("CI") != "true" or os.environ.get("GITHUB_ACTIONS") != "true":
        raise ValueError("Only an ephemeral GitHub Actions checkout is allowed")
    if not os.environ.get("RUNNER_TEMP", "").startswith("/"):
        raise ValueError("Expected disposable runner temp root")
    root = Path.cwd().resolve()
    index, target, source = (root / INDEX, root / TARGET, root / SOURCE)
    if index.is_symlink() or target.exists() or target.is_symlink() or source.is_symlink():
        raise ValueError("Untrusted source/target path")
    pristine = subprocess.run(
        ["git", "show", FROZEN + ":" + INDEX],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        check=True,
    ).stdout
    actual = index.read_bytes()
    if actual != pristine:
        raise ValueError("Frontend input differs from frozen Phase 5 HTML")
    if actual.count(b"</body>") != 1 or b"phase6-ipc-probe" in actual:
        raise ValueError("Unexpected frontend entrypoint")
    # No override of production Tauri capabilities or CSP is permitted.
    shutil.copyfile(source, target)
    index.write_bytes(actual.replace(b"</body>", INJECT.encode() + b"</body>"))
    print("Injected disposable read-only WebView IPC probe into CI working copy ONLY")


if __name__ == "__main__":
    try:
        prepare()
    except (ValueError, OSError, subprocess.CalledProcessError) as exc:
        print("PHASE6 IPC FIXTURE HOLD: " + str(exc), file=sys.stderr)
        raise SystemExit(2)
