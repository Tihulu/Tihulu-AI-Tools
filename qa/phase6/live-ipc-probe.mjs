// CI-only frontend probe, injected ONLY into a disposable checkout before build.
// Not shipped with the frozen app. Does not call privileged commands.
export const EXPECTED_MODULES = Object.freeze([
  "core_ledger", "model_registry", "model_market_contract",
  "gpu_transport_contract", "jan_integration_contract",
]);
export const PROBE_PASS = "TIHULU_PHASE6_IPC_PASS";
export const PROBE_FAIL = "TIHULU_PHASE6_IPC_FAIL";

const exactKeys = (value, keys) =>
  value !== null && typeof value === "object" && !Array.isArray(value) &&
  Object.keys(value).length === keys.length &&
  keys.every((key) => Object.hasOwn(value, key));

export function validBackendStatus(value) {
  const keys = [
    "schema_version", "source", "transport", "ui_bridge_ready",
    "backend_verified", "hardware_active", "cleanup_verified",
  ];
  return exactKeys(value, keys) &&
    value.schema_version === 1 &&
    value.source === "demo_fixture" &&
    value.transport === "tauri_ipc" &&
    value.ui_bridge_ready === true &&
    value.backend_verified === false &&
    value.hardware_active === false &&
    value.cleanup_verified === false;
}

export function validNativeModules(value) {
  const keys = [
    "schema_version", "source", "linked_modules", "modules_compiled",
    "registry_records", "gui_owner_registered", "approved_source_scans",
    "backend_verified", "jan_connected", "gpu_connected", "cleanup_verified",
  ];
  return exactKeys(value, keys) &&
    value.schema_version === 1 &&
    value.source === "native_in_memory_read_only" &&
    Array.isArray(value.linked_modules) &&
    value.linked_modules.length === EXPECTED_MODULES.length &&
    EXPECTED_MODULES.every((name, i) => value.linked_modules[i] === name) &&
    value.modules_compiled === true &&
    value.registry_records === 0 &&
    value.gui_owner_registered === false &&
    value.approved_source_scans === 0 &&
    value.backend_verified === false &&
    value.jan_connected === false &&
    value.gpu_connected === false &&
    value.cleanup_verified === false;
}

export async function liveReadonlyProbe(invoke) {
  if (typeof invoke !== "function") return false;
  try {
    // Both responses must come from the real Tauri bridge in the live webview.
    // A response is not proof of model inference or hardware operation.
    const backend = await invoke("ui_backend_status");
    if (!validBackendStatus(backend)) return false;
    const modules = await invoke("ui_native_module_status");
    return validNativeModules(modules);
  } catch {
    return false;
  }
}

if (typeof window !== "undefined" && typeof document !== "undefined") {
  const invoke = window.__TAURI__?.core?.invoke;
  void liveReadonlyProbe(invoke).then((ok) => {
    // The test overlay exists ONLY in an ephemeral CI-injected frontend.
    // A pure WebView document.title change may not update a Tauri OS titlebar.
    // Use an opaque pixel marker that the private Xvfb observer can sample.
    const indicator = document.createElement("div");
    indicator.id = "phase6-only-live-ipc-result";
    indicator.style.position = "fixed";
    indicator.style.inset = "0";
    indicator.style.zIndex = "2147483647";
    indicator.style.backgroundColor = ok ? "rgb(0, 255, 0)" : "rgb(255, 0, 0)";
    indicator.style.pointerEvents = "none";
    document.body.append(indicator);
    document.title = ok ? PROBE_PASS : PROBE_FAIL;
  });
}
