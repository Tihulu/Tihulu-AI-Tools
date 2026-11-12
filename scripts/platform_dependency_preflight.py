#!/usr/bin/env python3
"""Read-only Tauri build prerequisite inventory; no install or device operations."""
import argparse
import platform
import shutil
import subprocess
import sys


TARGETS = {
    "linux-x64": ("Linux", "x86_64"),
    "linux-arm64": ("Linux", "arm64"),
    "macos-arm64": ("Darwin", "arm64"),
    "windows-x64": ("Windows", "x86_64"),
    "windows-arm64": ("Windows", "arm64"),
    "pi5-arm64": ("Linux", "arm64"),
    "pi5-hailo10h": ("Linux", "arm64"),
}

LINUX_PKGCONFIG = ("webkit2gtk-4.1", "gtk+-3.0", "librsvg-2.0", "openssl")


def architecture(machine):
    m = machine.lower()
    return {"amd64": "x86_64", "x64": "x86_64", "aarch64": "arm64", "arm64": "arm64"}.get(m, m)


def dependency_plan(target):
    os_name, arch = TARGETS[target]
    if os_name == "Linux":
        return (f"Linux {arch}: Rust toolchain, C/C++ compiler, pkg-config, WebKitGTK 4.1, "
                "GTK3, OpenSSL development files, librsvg; optional tray needs Ayatana AppIndicator. "
                "Headless CLI needs neither WebKitGTK nor a desktop session.")
    if os_name == "Darwin":
        return "Apple Silicon: native Rust toolchain, Xcode Command Line Tools or Xcode, SDK, WKWebView. Signed/notarized .app requires separate credentials and acceptance."
    return (f"Windows {arch}: Rust MSVC target, MSVC C++ build tools, Windows SDK, "
            "WebView2 runtime. Installer/signing and actual Job Object tests are separate gates.")


def host_matches(target, os_name, machine):
    expected_os, expected_arch = TARGETS[target]
    return os_name == expected_os and architecture(machine) == expected_arch


def probe(target, os_name, machine):
    """No real accelerator, model, driver or hardware access; only tools/pkg-config."""
    if not host_matches(target, os_name, machine):
        print("UNVERIFIED: host OS/arch mismatch; cannot execute native prerequisite probe")
        return 3
    expected_os, _ = TARGETS[target]
    tools = ["cargo", "rustc"]
    if expected_os == "Linux":
        tools += ["cc", "pkg-config"]
    elif expected_os == "Darwin":
        tools += ["xcode-select", "xcrun"]
    else:
        tools += ["cl"]
    missing = []
    for command in tools:
        path = shutil.which(command)
        print(f"TOOL {command}: {'FOUND ' + path if path else 'MISSING/NOT ON PATH'}")
        if path is None:
            missing.append(command)
    if expected_os == "Linux" and shutil.which("pkg-config"):
        for pkg in LINUX_PKGCONFIG:
            result = subprocess.run(["pkg-config", "--modversion", pkg],
                                    capture_output=True, text=True, timeout=10,
                                    check=False)
            present = result.returncode == 0
            print(f"PKGCONFIG {pkg}: {result.stdout.strip() if present else 'MISSING'}")
            if not present:
                missing.append(pkg)
    if target.startswith("pi5"):
        print("HARDWARE UNVERIFIED: ARM64 is not Pi 5/Hailo acceptance. No PCIe, HailoRT, SDR or NPU probe performed.")
    if missing:
        print("BLOCKED prerequisites: " + ", ".join(missing))
        return 4
    print("AVAILABLE: host tool/development-library prerequisites only; no package or hardware acceptance.")
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", required=True, choices=sorted(TARGETS))
    parser.add_argument("--check-host", action="store_true",
                        help="Explicitly run read-only host tool and pkg-config checks")
    args = parser.parse_args(argv)
    print(dependency_plan(args.target))
    print(f"Observed host: {platform.system()} {platform.machine()}")
    if not args.check_host:
        print("PLAN ONLY: no dependencies were installed, checked or changed.")
        return 0
    return probe(args.target, platform.system(), platform.machine())


if __name__ == "__main__":
    sys.exit(main())
