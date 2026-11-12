"""CI-only, non-mutating preflight for an AI-09 disposable installation lab.

This is a test harness boundary, NOT a safe production installer or cleanup tool.
No creation, deletion, chmod, application launch or network access occurs here.
"""
from __future__ import annotations

import os
import re
import stat
from pathlib import Path
from typing import Mapping


class LabHold(ValueError):
    """An installer laboratory precondition is not met."""


def inspect_pristine_lab(
    lab: Path, runner_temp: Path, env: Mapping[str, str] | None = None
) -> tuple[int, int]:
    """Inspect a new private runner-owned directory, fail closed if not pristine.

    Return its observed (device, inode), NOT permission to delete it later.
    """
    if env is None:
        env = os.environ
    if env.get("CI") != "true" or env.get("GITHUB_ACTIONS") != "true":
        raise LabHold("GitHub CI-only lab; never run against a user workstation")

    lab, runner_temp = Path(lab), Path(runner_temp)
    if not lab.is_absolute() or not runner_temp.is_absolute():
        raise LabHold("Absolute lab and RUNNER_TEMP paths required")
    if lab.parent != runner_temp:
        raise LabHold("Lab must be an immediate child of RUNNER_TEMP")
    if not re.fullmatch(r"ai09-[A-Za-z0-9_-]{6,70}", lab.name):
        raise LabHold("Unexpected lab namespace")

    try:
        parent_stat = runner_temp.lstat()
        pre_stat = lab.lstat()
    except OSError as exc:
        raise LabHold("Lab or runner root unavailable") from exc

    if (
        not stat.S_ISDIR(parent_stat.st_mode)
        or parent_stat.st_uid != os.getuid()
        or (stat.S_IMODE(parent_stat.st_mode) & 0o022)
    ):
        raise LabHold("Unsafe RUNNER_TEMP identity or mode")
    if (
        not stat.S_ISDIR(pre_stat.st_mode)
        or pre_stat.st_uid != os.getuid()
        or stat.S_IMODE(pre_stat.st_mode) != 0o700
    ):
        raise LabHold("Lab must be a private 0700 directory owned by the runner")

    flags = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
    try:
        fd = os.open(lab, flags)
    except OSError as exc:
        raise LabHold("Unable to pin private lab directory") from exc
    try:
        pinned_stat = os.fstat(fd)
        if (pinned_stat.st_dev, pinned_stat.st_ino) != (
            pre_stat.st_dev, pre_stat.st_ino
        ):
            raise LabHold("Lab changed during initial pin")
        with os.scandir(fd) as entries:
            if next(entries, None) is not None:
                raise LabHold("Foreign or existing lab contents: refuse to proceed")
        after_stat = lab.lstat()
        if (after_stat.st_dev, after_stat.st_ino) != (
            pinned_stat.st_dev, pinned_stat.st_ino
        ):
            raise LabHold("Lab name was replaced during preflight")
        return pinned_stat.st_dev, pinned_stat.st_ino
    except OSError as exc:
        raise LabHold("Lab changed during inspection") from exc
    finally:
        os.close(fd)
