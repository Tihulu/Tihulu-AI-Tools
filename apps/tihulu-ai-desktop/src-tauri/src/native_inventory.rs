//! Native process integration proof: in-memory Rust modules only.
//! No filesystem discovery, app scanning, model loading, network, or device
//! access occurs here. This must not be mistaken for actual backend readiness.

use serde::Serialize;
use std::sync::Mutex;
use tauri::State;
use tihulu_ai_core::{OwnerId, ResourceLedger};
use tihulu_ai_registry::ModelRegistry;

const LINKED_MODULES: [&str; 5] = [
    "core_ledger",
    "model_registry",
    "model_market_contract",
    "gpu_transport_contract",
    "jan_integration_contract",
];

#[derive(Default)]
pub struct NativeBackend {
    registry: Mutex<ModelRegistry>,
    ledger: Mutex<ResourceLedger>,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct NativeModuleStatus {
    schema_version: u8,
    source: &'static str,
    linked_modules: [&'static str; 5],
    modules_compiled: bool,
    registry_records: usize,
    gui_owner_registered: bool,
    approved_source_scans: u32,
    backend_verified: bool,
    jan_connected: bool,
    gpu_connected: bool,
    cleanup_verified: bool,
}

impl NativeBackend {
    pub fn new() -> Self {
        Self::default()
    }

    fn read_status(&self) -> Result<NativeModuleStatus, &'static str> {
        let registry = self.registry.lock().map_err(|_| "registry_unavailable")?;
        let ledger = self.ledger.lock().map_err(|_| "ledger_unavailable")?;
        // Type references force the real Market, Compute and Jan crates to
        // compile into the native shell. No runtime adapter is activated.
        let modules_compiled = [
            std::any::type_name::<tihulu_ai_market::ModelManifest>(),
            std::any::type_name::<tihulu_ai_transport::MockComputeHost>(),
            std::any::type_name::<tihulu_ai_integrations::typed::TypedAppAdapter>(),
        ]
        .iter()
        .all(|name| name.starts_with("tihulu_ai_"));

        Ok(NativeModuleStatus {
            schema_version: 1,
            source: "native_in_memory_read_only",
            linked_modules: LINKED_MODULES,
            modules_compiled,
            registry_records: registry.len(),
            gui_owner_registered: ledger.owner_phase(OwnerId(1)).is_some(),
            approved_source_scans: 0,
            backend_verified: false,
            jan_connected: false,
            gpu_connected: false,
            cleanup_verified: false,
        })
    }
}

/// Read-only Tauri command with no untrusted path, process, device or
/// network arguments. Only linked modules and a local empty registry are read.
#[tauri::command]
pub fn ui_native_module_status(
    state: State<'_, NativeBackend>,
) -> Result<NativeModuleStatus, &'static str> {
    state.read_status()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_crates_are_linked_but_never_report_inference_or_cleanup() {
        let status = NativeBackend::new().read_status().unwrap();
        assert_eq!(status.source, "native_in_memory_read_only");
        assert!(status.modules_compiled);
        assert_eq!(status.linked_modules, LINKED_MODULES);
        assert_eq!(status.registry_records, 0);
        assert!(!status.gui_owner_registered);
        assert_eq!(status.approved_source_scans, 0);
        assert!(!status.backend_verified);
        assert!(!status.jan_connected);
        assert!(!status.gpu_connected);
        assert!(!status.cleanup_verified);
    }

    #[test]
    fn no_source_scan_or_owned_model_lease_appears_between_reads() {
        let backend = NativeBackend::new();
        let first = backend.read_status().unwrap();
        let second = backend.read_status().unwrap();
        assert_eq!(first, second);
        assert_eq!(second.registry_records, 0);
    }
}
