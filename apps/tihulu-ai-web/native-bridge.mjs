/**
 * Read-only Tauri 2 IPC probe; the web deployment always remains offline demo.
 * No model, file, endpoint, process, remote host, or device action is exposed.
 * Never treat "Tauri IPC responded" as "model/cleanup backend verified".
 */
export const NATIVE_BRIDGE_PHASE = Object.freeze({
  WEB_PREVIEW: "web_preview",
  TAURI_DEMO: "tauri_demo_bridge",
  INVALID: "invalid_native_reply",
  FAILED: "native_ipc_failed",
});

const responseKeys = [
  "schema_version", "source", "transport", "ui_bridge_ready",
  "backend_verified", "hardware_active", "cleanup_verified",
];

const isSafeDemoStatus = (value) =>
  value !== null && typeof value === "object" && !Array.isArray(value) &&
  Object.keys(value).length === responseKeys.length &&
  responseKeys.every((key) => Object.hasOwn(value, key)) &&
  value.schema_version === 1 &&
  value.source === "demo_fixture" &&
  value.transport === "tauri_ipc" &&
  value.ui_bridge_ready === true &&
  value.backend_verified === false &&
  value.hardware_active === false &&
  value.cleanup_verified === false;

/**
 * @param {undefined | ((command:string)=>Promise<unknown>)} invoke
 * @returns {Promise<{phase:string, backend_verified:false}>}
 */
// Resolve the Tauri global only inside the isolated probe module. Web UI logic
// is still a browser-safe mock and never invokes effectful native commands.
export function getGlobalNativeInvoke() {
  return globalThis.window?.__TAURI__?.core?.invoke;
}

export async function probeNativeBridge(invoke) {
  if (typeof invoke !== "function") {
    return { phase: NATIVE_BRIDGE_PHASE.WEB_PREVIEW, backend_verified: false };
  }
  try {
    const status = await invoke("ui_backend_status");
    return {
      phase: isSafeDemoStatus(status) ?
        NATIVE_BRIDGE_PHASE.TAURI_DEMO : NATIVE_BRIDGE_PHASE.INVALID,
      backend_verified: false,
    };
  } catch {
    // Do not surface potentially sensitive local IPC error details to the UI.
    return { phase: NATIVE_BRIDGE_PHASE.FAILED, backend_verified: false };
  }
}

const NATIVE_MODULES = Object.freeze([
  "core_ledger", "model_registry", "model_market_contract",
  "gpu_transport_contract", "jan_integration_contract",
]);

export const NATIVE_MODULE_PHASE = Object.freeze({
  WEB_PREVIEW: "web_preview",
  LINKED: "native_contracts_linked",
  INVALID: "invalid_native_modules",
  FAILED: "native_module_ipc_failed",
});

const isNativeModuleStatus = (value) => {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const keys = [
    "schema_version", "source", "linked_modules", "modules_compiled",
    "registry_records", "gui_owner_registered", "approved_source_scans",
    "backend_verified", "jan_connected", "gpu_connected", "cleanup_verified",
  ];
  return Object.keys(value).length === keys.length &&
    keys.every((key) => Object.hasOwn(value, key)) &&
    value.schema_version === 1 &&
    value.source === "native_in_memory_read_only" &&
    Array.isArray(value.linked_modules) &&
    value.linked_modules.length === NATIVE_MODULES.length &&
    NATIVE_MODULES.every((name, index) => value.linked_modules[index] === name) &&
    value.modules_compiled === true &&
    // Phase 4 initializes an EMPTY in-memory Registry and has no approved
    // source scans or registration path. Even one "record" contradicts this
    // exact contract; a future consent-scoped schema needs a separate validator.
    Number.isSafeInteger(value.registry_records) && value.registry_records === 0 &&
    value.gui_owner_registered === false &&
    value.approved_source_scans === 0 &&
    value.backend_verified === false &&
    value.jan_connected === false &&
    value.gpu_connected === false &&
    value.cleanup_verified === false;
};

/** Read-only, capability-only proof; never a model or GPU connection. */
export async function probeNativeModules(invoke) {
  if (typeof invoke !== "function") {
    return { phase: NATIVE_MODULE_PHASE.WEB_PREVIEW, backend_verified: false };
  }
  try {
    const result = await invoke("ui_native_module_status");
    return isNativeModuleStatus(result) ?
      { phase: NATIVE_MODULE_PHASE.LINKED, records: result.registry_records, backend_verified: false } :
      { phase: NATIVE_MODULE_PHASE.INVALID, backend_verified: false };
  } catch {
    return { phase: NATIVE_MODULE_PHASE.FAILED, backend_verified: false };
  }
}
