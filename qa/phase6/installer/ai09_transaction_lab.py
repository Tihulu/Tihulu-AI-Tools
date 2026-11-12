"""DISPOSABLE CI-ONLY synthetic installation transaction oracle.

This is NOT an AI Tools installer. It never writes to HOME/XDG or any actual
installation, never deletes versions, and cannot attest hostile same-UID safety.
All mutations are descriptor-relative inside a new private GitHub runner lab.
"""
from __future__ import annotations

import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import uuid

APP = "tihulu-ai-tools"
MARKER = "AI09_DISPOSABLE_TEST_ONLY_v1"
BASE_SHA = "283edc8a1101f63e08f12c172d6b2d6688b9a5ca"
DIR_FLAGS = os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC
READ_FLAGS = os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC
NEW_FLAGS = os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW | os.O_CLOEXEC
ALLOWED = frozenset({".ai09-owner", ".lock", "versions", "journal-history", "current", "journal"})


class Hold(RuntimeError):
    """A safety or evidence condition is not met; refuse changes."""


def _verify_regular(fd: int, *, mode: int = 0o600) -> None:
    st = os.fstat(fd)
    if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != mode:
        raise Hold("Unsafe file identity, ownership or permissions")


def _read(fd: int, name: str, *, limit: int = 1_048_576) -> bytes:
    try:
        f = os.open(name, READ_FLAGS, dir_fd=fd)
    except FileNotFoundError:
        raise Hold("Required owned file absent") from None
    except OSError as e:
        raise Hold("Unsafe file open") from e
    try:
        _verify_regular(f)
        data = os.read(f, limit + 1)
        if len(data) > limit:
            raise Hold("File too large")
        return data
    finally:
        os.close(f)


def _put_exclusive(fd: int, name: str, content: bytes) -> None:
    f = os.open(name, NEW_FLAGS, 0o600, dir_fd=fd)
    try:
        _verify_regular(f)
        mv = memoryview(content)
        while mv:
            mv = mv[os.write(f, mv):]
        os.fsync(f)
    finally:
        os.close(f)
    os.fsync(fd)


def _json(content: bytes) -> dict:
    try:
        value = json.loads(content)
    except (ValueError, UnicodeDecodeError) as e:
        raise Hold("Invalid transaction JSON") from e
    if not isinstance(value, dict):
        raise Hold("Invalid transaction object")
    return value


def _atomic_json(fd: int, dest: str, value: dict) -> None:
    # No overwrite of pre-existing staging names; unknown content remains untouched.
    staging = ".ai09-temp-" + uuid.uuid4().hex
    payload = (json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n").encode()
    try:
        _put_exclusive(fd, staging, payload)
        try:
            st = os.stat(dest, dir_fd=fd, follow_symlinks=False)
        except FileNotFoundError:
            pass
        else:
            if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o600:
                raise Hold("Destination isn't an owned regular file")
        os.replace(staging, dest, src_dir_fd=fd, dst_dir_fd=fd)
        os.fsync(fd)
    except BaseException:
        # Do not automatically unlink a possibly substituted staging path.
        # Preserve it for forensic inspection. Production cleanup remains HOLD.
        raise


class Lab:
    """Versioned fake executable copies in an isolated runner-owned directory."""

    def __init__(self, root: Path, runner_temp: Path, *, env: dict[str, str] | None = None):
        env = dict(os.environ if env is None else env)
        root, runner_temp = Path(root), Path(runner_temp)
        if env.get("CI") != "true" or env.get("GITHUB_ACTIONS") != "true":
            raise Hold("GitHub CI-only fixture")
        if env.get("RUNNER_TEMP") != str(runner_temp):
            raise Hold("Runner root must match declared RUNNER_TEMP exactly")
        if not runner_temp.is_absolute() or not root.is_absolute() or root.parent != runner_temp:
            raise Hold("Only direct absolute child of RUNNER_TEMP")
        if not re.fullmatch(r"ai09tx-[A-Za-z0-9_-]{6,64}", root.name):
            raise Hold("Unexpected disposable namespace")
        for p, exact in ((runner_temp, False), (root, True)):
            try:
                s = p.lstat()
            except OSError as e:
                raise Hold("Missing runner lab") from e
            if not stat.S_ISDIR(s.st_mode) or s.st_uid != os.getuid():
                raise Hold("Unsafe runner path identity")
            if (stat.S_IMODE(s.st_mode) != 0o700 if exact else bool(stat.S_IMODE(s.st_mode) & 0o022)):
                raise Hold("Unsafe runner path mode")
        self.parent_fd = os.open(runner_temp, DIR_FLAGS)
        try:
            self.root_fd = os.open(root.name, DIR_FLAGS, dir_fd=self.parent_fd)
        except BaseException:
            os.close(self.parent_fd)
            raise
        self.root = root
        self.runner_temp = runner_temp
        self.root_identity = (os.fstat(self.root_fd).st_dev, os.fstat(self.root_fd).st_ino)
        self._verify_path()

    def close(self) -> None:
        if getattr(self, "root_fd", -1) >= 0:
            os.close(self.root_fd)
            self.root_fd = -1
        if getattr(self, "parent_fd", -1) >= 0:
            os.close(self.parent_fd)
            self.parent_fd = -1

    def __enter__(self) -> "Lab":
        return self

    def __exit__(self, *args: object) -> None:
        self.close()

    def _verify_path(self) -> None:
        # Revalidate both visible names against the pinned descriptors.
        parent = self.runner_temp.lstat()
        pst = os.fstat(self.parent_fd)
        if (not stat.S_ISDIR(parent.st_mode) or
                (parent.st_dev, parent.st_ino) != (pst.st_dev, pst.st_ino)):
            raise Hold("RUNNER_TEMP path substituted")
        st = os.stat(self.root.name, dir_fd=self.parent_fd, follow_symlinks=False)
        if not stat.S_ISDIR(st.st_mode) or (st.st_dev, st.st_ino) != self.root_identity:
            raise Hold("Root path substituted: refuse")
        if stat.S_IMODE(st.st_mode) != 0o700:
            raise Hold("Lab permission drift")

    def _check_contents(self) -> None:
        self._verify_path()
        names = set(os.listdir(self.root_fd))
        if names - ALLOWED:
            raise Hold(f"Unexpected foreign or forensic file(s): {sorted(names-ALLOWED)}")
        if ".ai09-owner" not in names or ".lock" not in names or "versions" not in names or "journal-history" not in names:
            raise Hold("Missing lab identity/layout")
        if _read(self.root_fd, ".ai09-owner") != (MARKER + "\n").encode():
            raise Hold("Lab marker invalid")
        for name in ("versions", "journal-history"):
            fd = os.open(name, DIR_FLAGS, dir_fd=self.root_fd)
            try:
                st = os.fstat(fd)
                if st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o700:
                    raise Hold("Unsafe owned subdirectory")
                if name == "versions":
                    for child in os.listdir(fd):
                        if not re.fullmatch(r"v[1-9][0-9]{0,5}", child):
                            raise Hold("Unknown version entry, including interrupted stage")
                        child_fd = os.open(child, DIR_FLAGS, dir_fd=fd)
                        try:
                            cs = os.fstat(child_fd)
                            if (cs.st_uid != os.getuid() or
                                    stat.S_IMODE(cs.st_mode) != 0o700 or
                                    set(os.listdir(child_fd)) != {"fake-executable", "manifest.json"}):
                                raise Hold("Unexpected version path identity/content")
                        finally:
                            os.close(child_fd)
            finally:
                os.close(fd)

    def initialize(self) -> None:
        self._verify_path()
        if os.listdir(self.root_fd):
            raise Hold("Only empty disposable lab may be initialized")
        _put_exclusive(self.root_fd, ".ai09-owner", (MARKER + "\n").encode())
        _put_exclusive(self.root_fd, ".lock", b"AI09-LAB-LOCK\n")
        os.mkdir("versions", 0o700, dir_fd=self.root_fd)
        os.mkdir("journal-history", 0o700, dir_fd=self.root_fd)
        os.fsync(self.root_fd)
        self._check_contents()

    @contextlib.contextmanager
    def locked(self):
        self._check_contents()
        lock = os.open(".lock", os.O_RDWR | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=self.root_fd)
        try:
            _verify_regular(lock)
            st = os.stat(".lock", dir_fd=self.root_fd, follow_symlinks=False)
            if (st.st_dev, st.st_ino) != (os.fstat(lock).st_dev, os.fstat(lock).st_ino):
                raise Hold("Lock identity changed")
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            except BlockingIOError as e:
                raise Hold("Overlapping transaction refused") from e
            self._check_contents()
            yield
            self._check_contents()
        finally:
            os.close(lock)

    @staticmethod
    def _version(version: str) -> None:
        if not re.fullmatch(r"v[1-9][0-9]{0,5}", version):
            raise Hold("Untrusted version name")

    def _version_dir(self, version: str) -> int:
        self._version(version)
        root_versions = os.open("versions", DIR_FLAGS, dir_fd=self.root_fd)
        try:
            f = os.open(version, DIR_FLAGS, dir_fd=root_versions)
        except OSError as e:
            os.close(root_versions)
            raise Hold("Missing or unsafe installed version") from e
        os.close(root_versions)
        return f

    def _hash(self, version: str) -> str:
        f = self._version_dir(version)
        try:
            metadata = _json(_read(f, "manifest.json"))
            if set(metadata) != {"schema", "app_id", "version", "sha256", "source_commit", "fixture_only"}:
                raise Hold("Unexpected manifest keys")
            if (metadata["schema"], metadata["app_id"], metadata["version"],
                metadata["source_commit"], metadata["fixture_only"]) != (1, APP, version, BASE_SHA, True):
                raise Hold("Manifest identity mismatch")
            sha = hashlib.sha256(_read(f, "fake-executable")).hexdigest()
            if sha != metadata["sha256"]:
                raise Hold("Executable digest mismatch")
            return sha
        finally:
            os.close(f)

    def _current(self) -> str | None:
        try:
            os.stat("current", dir_fd=self.root_fd, follow_symlinks=False)
        except FileNotFoundError:
            return None
        data = _json(_read(self.root_fd, "current"))
        if set(data) != {"version", "sha256"}:
            raise Hold("Untrusted activation record")
        if data["version"] is None:
            if data["sha256"] is not None:
                raise Hold("Malformed deactivation")
            return None
        self._version(data["version"])
        if data["sha256"] != self._hash(data["version"]):
            raise Hold("Activation does not match on-disk executable")
        return data["version"]

    def current(self) -> str | None:
        with self.locked():
            return self._current()

    def _archive_journal(self, journal: dict) -> None:
        assert set(journal) == {"schema", "from", "to", "nonce"}
        archive = os.open("journal-history", DIR_FLAGS, dir_fd=self.root_fd)
        try:
            name = "completed-" + journal["nonce"] + ".json"
            if not re.fullmatch(r"completed-[0-9a-f]{32}\.json", name):
                raise Hold("Unsafe journal nonce")
            # If the archive name already exists, refuse rather than overwriting it.
            _put_exclusive(archive, name, _read(self.root_fd, "journal"))
        finally:
            os.close(archive)
        st = os.stat("journal", dir_fd=self.root_fd, follow_symlinks=False)
        if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid():
            raise Hold("Foreign journal: no unlink")
        # We cannot guarantee unlink safety against hostile SAME-UID replacement.
        # This is only a controlled disposable CI lab, not a production mechanism.
        os.unlink("journal", dir_fd=self.root_fd)
        os.fsync(self.root_fd)

    def recover(self) -> str:
        with self.locked():
            if "journal" not in os.listdir(self.root_fd):
                return "nothing-to-recover"
            journal = _json(_read(self.root_fd, "journal"))
            if set(journal) != {"schema", "from", "to", "nonce"} or journal["schema"] != 1:
                raise Hold("Invalid transaction journal")
            for v in (journal["from"], journal["to"]):
                if v is not None:
                    self._hash(v)
            current = self._current()
            if current not in (journal["from"], journal["to"]):
                raise Hold("Unexpected current version during recovery")
            # Atomic current pointer either old or new; never delete a version.
            self._archive_journal(journal)
            return "old-preserved" if current == journal["from"] else "new-committed"

    def _create_version(self, version: str, payload: bytes) -> None:
        self._version(version)
        if not (1 <= len(payload) <= 4096):
            raise Hold("Only bounded synthetic executable fixtures allowed")
        if not payload.startswith(b"#!/bin/sh\n") or b"TIHULU_AI09_SYNTHETIC" not in payload:
            raise Hold("Only marked fake executable fixture accepted")
        versions = os.open("versions", DIR_FLAGS, dir_fd=self.root_fd)
        try:
            if version in os.listdir(versions):
                if self._hash(version) != hashlib.sha256(payload).hexdigest():
                    raise Hold("Same version name with different bytes")
                return
            stage = ".stage-" + uuid.uuid4().hex
            os.mkdir(stage, 0o700, dir_fd=versions)
            fs = os.open(stage, DIR_FLAGS, dir_fd=versions)
            try:
                _put_exclusive(fs, "fake-executable", payload)
                digest = hashlib.sha256(payload).hexdigest()
                manifest = {"schema": 1, "app_id": APP, "version": version,
                            "sha256": digest, "source_commit": BASE_SHA, "fixture_only": True}
                _put_exclusive(fs, "manifest.json", (json.dumps(manifest, sort_keys=True) + "\n").encode())
                os.fsync(fs)
            finally:
                os.close(fs)
            if version in os.listdir(versions):
                raise Hold("Version destination appeared during staging")
            os.rename(stage, version, src_dir_fd=versions, dst_dir_fd=versions)
            os.fsync(versions)
        finally:
            os.close(versions)

    def switch(self, version: str | None, payload: bytes | None = None, *, crash: str | None = None) -> None:
        """Switch fake activation; crash options only honored by special test worker."""
        with self.locked():
            if "journal" in os.listdir(self.root_fd):
                raise Hold("Unrecovered transaction exists")
            before = self._current()
            if version is not None:
                self._version(version)
                if payload is not None:
                    self._create_version(version, payload)
                digest = self._hash(version)
            else:
                if payload is not None:
                    raise Hold("Deactivation cannot accept executable data")
                digest = None
            if before == version:
                return
            journal = {"schema": 1, "from": before, "to": version, "nonce": uuid.uuid4().hex}
            _atomic_json(self.root_fd, "journal", journal)
            if crash == "prepared" and os.environ.get("AI09_TX_KILL_TEST_CHILD") == "true":
                os.kill(os.getpid(), 9)
            _atomic_json(self.root_fd, "current", {"version": version, "sha256": digest})
            if crash == "activated" and os.environ.get("AI09_TX_KILL_TEST_CHILD") == "true":
                os.kill(os.getpid(), 9)
            self._archive_journal(journal)


# Any actual SIGKILL targets exclusively a newly forked test worker which has
# AI09_TX_KILL_TEST_CHILD=true. No process discovery, providers or services.
if __name__ == "__main__":
    import argparse
    p = argparse.ArgumentParser()
    p.add_argument("--worker", action="store_true", required=True)
    p.add_argument("--lab", type=Path, required=True)
    p.add_argument("--runner-temp", type=Path, required=True)
    p.add_argument("--version", required=True)
    p.add_argument("--crash", choices=("prepared", "activated"), required=True)
    a = p.parse_args()
    if os.environ.get("AI09_TX_KILL_TEST_CHILD") != "true":
        raise SystemExit("Refusing child process without explicit synthetic kill fixture")
    with Lab(a.lab, a.runner_temp) as lab:
        lab.switch(a.version, ("#!/bin/sh\n# TIHULU_AI09_SYNTHETIC " + a.version + "\nexit 0\n").encode(), crash=a.crash)
