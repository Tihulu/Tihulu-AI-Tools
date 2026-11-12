#!/usr/bin/env python3
"""Validate a *manually observed* Tauri 2 startup evidence report, read-only.

Never launches the GUI or a GPU host. Parsing a report is not proof that a GUI
was really observed; original native process logs/screenshots are still needed.
"""
from __future__ import annotations
import argparse
import json
from pathlib import Path
import sys

EXPECTED_MODULES = [
    'core_ledger', 'model_registry', 'model_market_contract',
    'gpu_transport_contract', 'jan_integration_contract',
]


def validate_report(report: dict) -> tuple[bool, str]:
    if not isinstance(report, dict):
        return False, 'Expected JSON object'
    if set(report) != {'schema_version', 'platform', 'observer', 'gui_window_observed',
                       'native_ipc_observed', 'ui_native_module_status',
                       'external_services_untouched', 'native_process_exit_observed'}:
        return False, 'Incomplete/unknown report fields'
    if report['schema_version'] != 1 or report['platform'] not in {
            'linux-x64', 'linux-arm64', 'macos-arm64', 'windows-x64', 'windows-arm64'}:
        return False, 'Unsupported report version/platform'
    if not isinstance(report['observer'], str) or not report['observer'].strip():
        return False, 'Observer must be identified'
    for key in ('gui_window_observed', 'native_ipc_observed',
                'external_services_untouched', 'native_process_exit_observed'):
        if report[key] is not True:
            return False, f'{key} must be explicitly observed true'
    status = report['ui_native_module_status']
    if not isinstance(status, dict) or set(status) != {
            'schema_version', 'source', 'linked_modules', 'modules_compiled',
            'registry_records', 'gui_owner_registered', 'approved_source_scans',
            'backend_verified', 'jan_connected', 'gpu_connected', 'cleanup_verified'}:
        return False, 'Invalid native IPC schema'
    if (status['schema_version'] != 1 or status['source'] != 'native_in_memory_read_only'
            or status['linked_modules'] != EXPECTED_MODULES or status['modules_compiled'] is not True
            or status['registry_records'] != 0 or status['gui_owner_registered'] is not False
            or status['approved_source_scans'] != 0 or
            any(status[k] is not False for k in ('backend_verified', 'jan_connected',
                                                  'gpu_connected', 'cleanup_verified'))):
        return False, 'Native status mismatch or forged running/cleanup claims'
    return True, 'Report schema is consistent; observer evidence still unverified'


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args(argv)
    path = args.report
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 64 * 1024:
        print('REPORT REJECTED: missing, symlink or too large', file=sys.stderr)
        return 2
    try:
        report = json.loads(path.read_text(encoding='utf-8'))
    except (OSError, UnicodeError, ValueError) as exc:
        print(f'REPORT REJECTED: {exc}', file=sys.stderr)
        return 2
    valid, reason = validate_report(report)
    print(('REPORT SCHEMA OK: ' if valid else 'REPORT REJECTED: ') + reason)
    print('NO GUI started by validator; NOT proof of actual startup or RAM/VRAM teardown.')
    return 0 if valid else 2


if __name__ == '__main__':
    raise SystemExit(main())
