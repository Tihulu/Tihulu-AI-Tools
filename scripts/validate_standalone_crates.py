#!/usr/bin/env python3
"""Print or opt-in execute native checks for five independently owned Rust crates.

No checkout, install, device probe or service change. Cargo network is opt-in.
A manifest must already exist in a coordinator-prepared checkout/other agent branch.
"""
import argparse
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tomllib


INTEGRATED_MEMBERS = (
    "crates/tihulu-ai-core",
    "apps/tihulu-ai-cli",
    "crates/tihulu-ai-registry",
    "crates/tihulu-ai-market",
    "crates/tihulu-ai-transport",
    "crates/tihulu-ai-integrations",
)
DESKTOP_DIR = "apps/tihulu-ai-desktop/src-tauri"


CRATES = {
    "registry": "crates/tihulu-ai-registry/Cargo.toml",
    "market": "crates/tihulu-ai-market/Cargo.toml",
    "transport": "crates/tihulu-ai-transport/Cargo.toml",
    "integrations": "crates/tihulu-ai-integrations/Cargo.toml",
    "desktop": "apps/tihulu-ai-desktop/src-tauri/Cargo.toml",
}


def commands(manifest, allow_network=False):
    """Never use root workspace's Cargo manifest for an isolated crate."""
    path = str(manifest)
    offline = [] if allow_network else ["--offline"]
    return [
        ["cargo", "fmt", "--manifest-path", path, "--all", "--", "--check"],
        ["cargo", "test", "--manifest-path", path, "--all-targets", *offline],
        ["cargo", "clippy", "--manifest-path", path, "--all-targets", *offline,
         "--", "-D", "warnings"],
    ]


def integrated_commands(allow_network=False):
    """Coordinator sandbox workspace: excludes native Tauri desktop by contract."""
    offline = [] if allow_network else ["--offline"]
    return [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "test", "--workspace", "--all-targets", *offline],
        ["cargo", "clippy", "--workspace", "--all-targets", *offline,
         "--", "-D", "warnings"],
    ]


def inspect_integrated_workspace(root):
    """Validate actual root membership before claiming root tests cover agents."""
    root = root.resolve(strict=True)
    manifest = root / "Cargo.toml"
    if not manifest.is_file() or manifest.is_symlink():
        raise ValueError("root Cargo.toml missing or symlinked")
    try:
        with manifest.open("rb") as stream:
            workspace = tomllib.load(stream)["workspace"]
    except (KeyError, tomllib.TOMLDecodeError) as exc:
        raise ValueError(f"root Cargo.toml lacks valid [workspace]: {exc}") from exc
    members = set(workspace.get("members", ()))
    missing_members = set(INTEGRATED_MEMBERS) - members
    if missing_members:
        raise ValueError("root workspace excludes required members: " +
                         ", ".join(sorted(missing_members)))
    if DESKTOP_DIR in members or DESKTOP_DIR not in workspace.get("exclude", ()):
        raise ValueError("Tauri desktop must remain explicitly excluded from root CI")
    missing = []
    for member in INTEGRATED_MEMBERS:
        path = root / member / "Cargo.toml"
        if path.is_symlink() or not path.resolve().is_relative_to(root):
            raise ValueError(f"unsafe integrated manifest path: {member}")
        if not path.is_file():
            missing.append(member)
    return missing


def inspect_manifests(root, selected):
    root = root.resolve(strict=True)
    if not (root / "Cargo.toml").is_file():
        raise ValueError("repo root lacks Cargo.toml")
    found, missing = [], []
    for name in selected:
        relpath = CRATES[name]
        path = root / relpath
        if path.is_symlink() or not path.resolve().is_relative_to(root):
            raise ValueError(f"unsafe manifest path: {relpath}")
        if path.is_file():
            found.append((name, relpath))
        else:
            missing.append((name, relpath))
    return found, missing


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path,
                        default=Path(__file__).resolve().parents[1])
    parser.add_argument("--only", action="append", choices=sorted(CRATES),
                        help="Select a crate on its own agent branch; repeatable")
    parser.add_argument("--integrated", action="store_true",
                        help="Verify and check coordinator root workspace (Tauri separate)")
    parser.add_argument("--execute", action="store_true",
                        help="Execute native Cargo commands; default is read-only plan")
    parser.add_argument("--allow-network", action="store_true",
                        help="Permit Cargo dependency retrieval (explicit opt-in)")
    parser.add_argument("--timeout-seconds", type=int, default=900,
                        help="Per Cargo command timeout, default 900 seconds")
    args = parser.parse_args(argv)
    if args.timeout_seconds < 1:
        parser.error("--timeout-seconds must be positive")
    if args.integrated and args.only:
        parser.error("--integrated cannot be combined with --only; desktop is separate")
    root = args.repo_root.resolve()
    try:
        if args.integrated:
            missing_workspace = inspect_integrated_workspace(root)
            found = [("integrated", "Cargo.toml")]
            missing = [("integrated", item + "/Cargo.toml")
                       for item in missing_workspace]
        else:
            selected = list(dict.fromkeys(args.only or CRATES.keys()))
            found, missing = inspect_manifests(root, selected)
    except (ValueError, OSError) as exc:
        print(f"INVALID CHECKOUT: {exc}", file=sys.stderr)
        return 2

    print(f"Host: {platform.system()} {platform.machine()} | root: {root}", flush=True)
    if args.integrated:
        print("INTEGRATED: root core, CLI, registry, market, transport, integrations; "
              "Tauri 2 desktop explicitly excluded")
    for name, path in missing:
        print(f"NOT PRESENT [{name}]: {path} (no source checkout or merge performed)")
    for name, path in found:
        print(f"PRESENT [{name}]: {path}")
        plan = integrated_commands(args.allow_network) if args.integrated else commands(path, args.allow_network)
        for cmd in plan:
            print("  " + subprocess.list2cmdline(cmd), flush=True)

    if not args.execute:
        print("PLAN ONLY: no Cargo commands were run.")
        return 0
    if missing:
        print("INCOMPLETE: missing manifest(s), no Cargo commands executed.", file=sys.stderr)
        return 3
    if shutil.which("cargo") is None:
        print("UNAVAILABLE: cargo executable not installed.", file=sys.stderr)
        return 2

    env = os.environ.copy()
    if not args.allow_network:
        env["CARGO_NET_OFFLINE"] = "true"
    for name, path in found:
        plan = integrated_commands(args.allow_network) if args.integrated else commands(path, args.allow_network)
        for cmd in plan:
            print(f"EXECUTE [{name}]: {subprocess.list2cmdline(cmd)}", flush=True)
            try:
                completed = subprocess.run(cmd, cwd=root, env=env,
                                           timeout=args.timeout_seconds, check=False)
            except subprocess.TimeoutExpired:
                print(f"TIMEOUT [{name}]: command exceeded bound; inspect child processes",
                      file=sys.stderr)
                return 124
            if completed.returncode != 0:
                print(f"FAILED [{name}]: exit {completed.returncode}", file=sys.stderr)
                return completed.returncode or 1
    scope = "integrated Rust workspace" if args.integrated else f"{len(found)} selected manifest(s)"
    print(f"PASS: all commands completed for {scope}; "
          "this is NOT Tauri packaging or physical hardware acceptance.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
