#!/usr/bin/env python3
"""Opt-in Rust foundation checks. No install, service, model or device access."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]


def commands(allow_network):
    offline = [] if allow_network else ["--offline"]
    return [
        ["cargo", "fmt", "--all", "--", "--check"],
        ["cargo", "test", "--workspace", "--all-targets", *offline],
        ["cargo", "clippy", "--workspace", "--all-targets", *offline,
         "--", "-D", "warnings"],
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--execute", action="store_true",
                        help="Actually execute Rust checks; default prints plan")
    parser.add_argument("--allow-network", action="store_true",
                        help="Allow Cargo to fetch dependencies when executing")
    args = parser.parse_args()
    for command in commands(args.allow_network):
        print(" ".join(command), flush=True)
        if args.execute:
            if shutil.which("cargo") is None:
                print("cargo is not installed", file=sys.stderr)
                return 2
            result = subprocess.run(command, cwd=ROOT, check=False)
            if result.returncode:
                return result.returncode
    return 0


if __name__ == "__main__":
    sys.exit(main())
