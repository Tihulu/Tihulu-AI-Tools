#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;

// The only native command is a read-only IPC health check. It MUST NOT be
// confused with model execution, Jan, GPU-host connectivity or RAM/VRAM cleanup.
mod native_inventory;
#[allow(dead_code)]
mod ui_contract;

#[derive(Debug, Serialize)]
struct UiBackendStatus {
    schema_version: u8,
    source: &'static str,
    transport: &'static str,
    ui_bridge_ready: bool,
    backend_verified: bool,
    hardware_active: bool,
    cleanup_verified: bool,
}

#[tauri::command]
fn ui_backend_status() -> UiBackendStatus {
    UiBackendStatus {
        schema_version: ui_contract::UI_SCHEMA_VERSION,
        source: "demo_fixture",
        transport: "tauri_ipc",
        ui_bridge_ready: true,
        backend_verified: false,
        hardware_active: false,
        cleanup_verified: false,
    }
}

fn main() {
    tauri::Builder::default()
        .manage(native_inventory::NativeBackend::new())
        .invoke_handler(tauri::generate_handler![
            ui_backend_status,
            native_inventory::ui_native_module_status
        ])
        .run(tauri::generate_context!())
        .expect("failed to launch the TIHULU AI Tools UI demo");
}

#[cfg(test)]
mod native_contract_tests {
    use super::*;

    #[test]
    fn ipc_probe_never_claims_hardware_or_resource_release() {
        let status = ui_backend_status();
        assert_eq!(status.schema_version, 1);
        assert_eq!(status.source, "demo_fixture");
        assert_eq!(status.transport, "tauri_ipc");
        assert!(status.ui_bridge_ready);
        assert!(!status.backend_verified);
        assert!(!status.hardware_active);
        assert!(!status.cleanup_verified);
    }
}
