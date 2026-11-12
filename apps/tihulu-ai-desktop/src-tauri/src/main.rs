#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Tauri 2 UI-only host. There are intentionally no commands, hardware APIs,
/// model runtimes, background workers, filesystem grants or remote listeners.
/// A future Rust supervisor must own real handles and verify shutdown before
/// the UI may claim model/context memory or SDR capture was released.
#[allow(dead_code)] // DTOs are not exposed as native commands until the Rust supervisor is verified.
mod ui_contract;

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to launch the TIHULU AI Tools UI demo");
}
