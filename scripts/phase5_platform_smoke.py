#!/usr/bin/env python3
"""Phase 5 non-installing, offline-first Tauri platform checks.

Default mode only validates a checkout and PRINTS commands. --execute-build is
opt-in and only builds/tests in a prepared checkout; no app is launched, bundled,
installed, uninstalled or connected to a live service/device.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]
NATIVE_MANIFEST = Path('apps/tihulu-ai-desktop/src-tauri/Cargo.toml')
NATIVE_CONFIG = Path('apps/tihulu-ai-desktop/src-tauri/tauri.conf.json')
FRONTEND = Path('apps/tihulu-ai-web')
EXPECTED_MEMBERS = frozenset({
    'crates/tihulu-ai-core', 'apps/tihulu-ai-cli',
    'crates/tihulu-ai-registry', 'crates/tihulu-ai-market',
    'crates/tihulu-ai-transport', 'crates/tihulu-ai-integrations',
})
TARGETS = {
    'linux-x64': ('Linux', 'x86_64', 'x86_64-unknown-linux-gnu'),
    'linux-arm64': ('Linux', 'aarch64', 'aarch64-unknown-linux-gnu'),
    'macos-arm64': ('Darwin', 'aarch64', 'aarch64-apple-darwin'),
    'windows-x64': ('Windows', 'x86_64', 'x86_64-pc-windows-msvc'),
    'windows-arm64': ('Windows', 'aarch64', 'aarch64-pc-windows-msvc'),
}
ALLOWED_ARTIFACT_EXTENSIONS = ('.deb', '.appimage', '.dmg', '.pkg', '.msi', '.msix', '.exe')


def normalized_arch(machine: str) -> str:
    return {'amd64': 'x86_64', 'x64': 'x86_64', 'arm64': 'aarch64',
            'aarch64': 'aarch64', 'x86_64': 'x86_64'}.get(machine.lower(), machine.lower())


def host_matches(target: str, os_name: str | None = None, machine: str | None = None) -> bool:
    os_name = os_name or platform.system()
    machine = machine or platform.machine()
    expected_os, expected_machine, _ = TARGETS[target]
    return os_name == expected_os and normalized_arch(machine) == expected_machine


def safe_file(root: Path, relative: Path) -> Path:
    if relative.is_absolute() or '..' in relative.parts:
        raise ValueError(f'Unsafe checkout path: {relative}')
    candidate = root / relative
    if candidate.is_symlink() or not candidate.resolve().is_relative_to(root.resolve()):
        raise ValueError(f'Unsafe manifest symlink/traversal: {relative}')
    if not candidate.is_file():
        raise ValueError(f'Missing required checkout file: {relative}')
    return candidate


def inspect_checkout(root: Path) -> dict:
    root = root.resolve(strict=True)
    with safe_file(root, Path('Cargo.toml')).open('rb') as file:
        workspace = tomllib.load(file)['workspace']
    members = frozenset(workspace.get('members', ()))
    if not EXPECTED_MEMBERS.issubset(members):
        raise ValueError(f'Missing integrated workspace members: {sorted(EXPECTED_MEMBERS - members)}')
    if str(NATIVE_MANIFEST.parent).replace('\\', '/') not in workspace.get('exclude', []):
        raise ValueError('Native Tauri must remain excluded from root workspace')
    for member in EXPECTED_MEMBERS:
        safe_file(root, Path(member) / 'Cargo.toml')
    with safe_file(root, NATIVE_MANIFEST).open('rb') as file:
        cargo = tomllib.load(file)
    if cargo.get('package', {}).get('name') != 'tihulu-ai-desktop':
        raise ValueError('Unexpected native package identity')
    with safe_file(root, NATIVE_CONFIG).open(encoding='utf-8') as file:
        config = json.load(file)
    if config.get('bundle', {}).get('active') is not False:
        raise ValueError('Bundle must remain disabled: no installer/package generation')
    frontend_relative = config.get('build', {}).get('frontendDist')
    if frontend_relative != '../../tihulu-ai-web':
        raise ValueError('Unexpected Tauri frontend path: review manually')
    safe_file(root, FRONTEND / 'package.json')
    web_tests = sorted((root / FRONTEND / 'tests').glob('*.test.mjs'))
    if not web_tests:
        raise ValueError('Web UI tests not found')
    for test in web_tests:
        safe_file(root, test.relative_to(root))
    return {'root': root, 'web_tests': web_tests,
            'package_name': cargo['package']['name'],
            'bundle_disabled': True, 'root_members': len(members)}


def native_build_commands(web_tests: list[Path], root: Path, allow_network: bool = False) -> list[list[str]]:
    manifest = str(NATIVE_MANIFEST).replace('\\', '/')
    offline = [] if allow_network else ['--offline']
    return [
        ['cargo', 'fmt', '--manifest-path', manifest, '--all', '--', '--check'],
        ['cargo', 'test', '--manifest-path', manifest, '--all-targets', *offline],
        ['cargo', 'clippy', '--manifest-path', manifest, '--all-targets', *offline, '--', '-D', 'warnings'],
        ['cargo', 'build', '--manifest-path', manifest, *offline],
        ['node', '--test', *(str(path.relative_to(root)) for path in web_tests)],
    ]


def prerequisite_report(target: str) -> list[str]:
    os_name = TARGETS[target][0]
    tools = ['python', 'cargo', 'rustc', 'node']
    if os_name == 'Linux':
        tools += ['cc', 'pkg-config']
    elif os_name == 'Darwin':
        tools += ['xcrun', 'xcode-select']
    else:
        tools += ['cl']
    missing = [tool for tool in tools if not shutil.which(tool) and not
               (tool == 'python' and sys.executable)]
    if os_name == 'Linux':
        if shutil.which('pkg-config'):
            for lib in ('webkit2gtk-4.1', 'gtk+-3.0', 'librsvg-2.0', 'openssl'):
                try:
                    result = subprocess.run(['pkg-config', '--exists', lib], timeout=8,
                                            check=False, capture_output=True)
                    if result.returncode:
                        missing.append('pkg-config:' + lib)
                except (OSError, subprocess.TimeoutExpired):
                    missing.append('pkg-config:' + lib)
        else:
            missing.append('pkg-config:Tauri libraries NOT VERIFIED')
    return missing


def mac_app_bundle_report(path: Path) -> dict:
    """Read-only bounded .app metadata/digest; symlinks require manual review."""
    if path.is_symlink() or not path.is_dir() or path.suffix.lower() != '.app':
        raise ValueError('Expected a non-symlink macOS .app directory')
    info = path / 'Contents' / 'Info.plist'
    if not info.is_file() or info.is_symlink():
        raise ValueError('Missing or unsafe Info.plist')
    with info.open('rb') as stream:
        metadata = plistlib.load(stream)
    executable = metadata.get('CFBundleExecutable')
    if not isinstance(executable, str) or not executable or executable in ('.', '..') or '/' in executable or "\\" in executable:
        raise ValueError('Invalid macOS app executable metadata')
    binary = path / 'Contents' / 'MacOS' / executable
    if not binary.is_file() or binary.is_symlink():
        raise ValueError('Missing or unsafe macOS app binary')
    count = 0
    total = 0
    digest = hashlib.sha256()
    for item in sorted(path.rglob('*')):
        if item.is_symlink():
            raise ValueError('App bundle includes symlink; require manual bundle review')
        if item.is_file():
            count += 1
            if count > 20000:
                raise ValueError('App bundle file-count bound exceeded')
            size = item.stat().st_size
            total += size
            if total > 4 * 1024**3:
                raise ValueError('App bundle size bound exceeded')
            digest.update(str(item.relative_to(path)).encode('utf-8'))
            with item.open('rb') as stream:
                while data := stream.read(1024 * 1024):
                    digest.update(data)
    return {'path': str(path), 'kind': 'app_directory', 'files': count,
            'bytes': total, 'sha256_tree': digest.hexdigest(),
            'signature_verified': False, 'installer_executed': False,
            'launch_tested': False, 'hardware_tested': False}


def artifact_report(path: Path) -> dict:
    """Read-only metadata/hashing; never load or execute artifact contents."""
    if path.name.lower().endswith('.app') and path.is_dir():
        return mac_app_bundle_report(path)
    if path.is_symlink() or not path.is_file():
        raise ValueError('Artifact must be a regular, non-symlink file')
    if not path.name.lower().endswith(ALLOWED_ARTIFACT_EXTENSIONS):
        raise ValueError('Unrecognized package suffix: manual security review required')
    if path.stat().st_size == 0:
        raise ValueError('Empty package file')
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        while data := stream.read(1024 * 1024):
            digest.update(data)
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest.hexdigest(),
            'signature_verified': False, 'installer_executed': False,
            'launch_tested': False, 'hardware_tested': False}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repo-root', type=Path, default=ROOT)
    parser.add_argument('--target', required=True, choices=sorted(TARGETS))
    parser.add_argument('--execute-build', action='store_true',
                        help='Opt in to running native Cargo and Node checks only')
    parser.add_argument('--allow-network', action='store_true',
                        help='Only affects Cargo dependency retrieval; never downloads models')
    parser.add_argument('--preflight', action='store_true',
                        help='Read-only native tool/development library check')
    parser.add_argument('--inspect-artifact', type=Path,
                        help='Hash an existing package; do not install or launch')
    parser.add_argument('--timeout-seconds', type=int, default=900)
    args = parser.parse_args(argv)
    if args.timeout_seconds < 1:
        parser.error('timeout must be >= 1')
    if args.allow_network and not args.execute_build:
        parser.error('--allow-network requires --execute-build')
    if args.inspect_artifact and args.execute_build:
        parser.error('Package inspection and native build must be separate operations')
    if args.inspect_artifact:
        try:
            print(json.dumps(artifact_report(args.inspect_artifact), indent=2))
        except (ValueError, OSError) as exc:
            print(f'UNVERIFIED PACKAGE: {exc}', file=sys.stderr)
            return 2
        return 0
    try:
        report = inspect_checkout(args.repo_root)
    except (ValueError, KeyError, OSError, tomllib.TOMLDecodeError, json.JSONDecodeError) as exc:
        print(f'INVALID/INCOMPLETE CHECKOUT: {exc}', file=sys.stderr)
        return 2
    root = report['root']
    print(f'Phase 5 target: {args.target} / {TARGETS[args.target][2]}')
    print('Native Tauri manifest found. Bundle disabled. Native startup NOT tested.')
    commands = native_build_commands(report['web_tests'], root, args.allow_network)
    for command in commands:
        print('PLAN: ' + subprocess.list2cmdline(command))
    if args.preflight:
        if not host_matches(args.target):
            print('BLOCKED: native target/host mismatch; cross-compiled artifact is not native acceptance')
            return 3
        missing = prerequisite_report(args.target)
        if missing:
            print('BLOCKED: missing native tools/libraries: ' + ', '.join(missing))
            return 4
        print('Preflight tools present (NOT proof of Tauri startup/installers or hardware).')
    if not args.execute_build:
        print('PLAN ONLY: no build, startup, service, installer or hardware action.')
        return 0
    if not host_matches(args.target):
        print('BLOCKED: native host target mismatch; refusing execution')
        return 3
    if not shutil.which('cargo') or not shutil.which('node'):
        print('BLOCKED: cargo and node must already be installed')
        return 4
    env = os.environ.copy()
    if not args.allow_network:
        env['CARGO_NET_OFFLINE'] = 'true'
    for command in commands:
        print('EXECUTE: ' + subprocess.list2cmdline(command), flush=True)
        try:
            completed = subprocess.run(command, cwd=root, env=env,
                                       check=False, timeout=args.timeout_seconds)
        except subprocess.TimeoutExpired:
            print('TIMEOUT: inspect/clean ONLY owned build workers; no process-name kills', file=sys.stderr)
            return 124
        except OSError as exc:
            print(f'FAILED: {exc}', file=sys.stderr)
            return 127
        if completed.returncode:
            print(f'FAILED: exit {completed.returncode}; stop (no later stage run)', file=sys.stderr)
            return completed.returncode
    print('PASS: native source fmt/test/clippy/build + web tests; NOT GUI startup, package or hardware acceptance')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
