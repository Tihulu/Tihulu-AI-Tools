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
