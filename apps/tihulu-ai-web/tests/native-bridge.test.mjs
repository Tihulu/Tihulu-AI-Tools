import test from "node:test";
import assert from "node:assert/strict";
import { getGlobalNativeInvoke, NATIVE_BRIDGE_PHASE, probeNativeBridge,
  NATIVE_MODULE_PHASE, probeNativeModules } from "../native-bridge.mjs";

const safeFixture = Object.freeze({
  schema_version: 1,
  source: "demo_fixture",
  transport: "tauri_ipc",
  ui_bridge_ready: true,
  backend_verified: false,
  hardware_active: false,
  cleanup_verified: false,
});

test("Browser host never enables native IPC unless explicitly injected by Tauri", () => {
  assert.equal(getGlobalNativeInvoke(), undefined);
});

test("Standalone web preview never probes a native process", async () => {
  assert.deepEqual(await probeNativeBridge(undefined), {
    phase: NATIVE_BRIDGE_PHASE.WEB_PREVIEW, backend_verified: false,
  });
});

test("Read-only native Tauri IPC response is explicitly demo-only", async () => {
  const invoked = [];
  const probe = await probeNativeBridge(async (command) => {
    invoked.push(command);
    return { ...safeFixture };
  });
  assert.deepEqual(invoked, ["ui_backend_status"]);
  assert.deepEqual(probe, { phase: NATIVE_BRIDGE_PHASE.TAURI_DEMO, backend_verified: false });
});

test("Reject forged connected, GPU, cleanup, incompatible schema or extra fields", async () => {
  for (const forged of [
    { ...safeFixture, source: "backend_verified" },
    { ...safeFixture, backend_verified: true },
    { ...safeFixture, hardware_active: true },
    { ...safeFixture, cleanup_verified: true },
    { ...safeFixture, schema_version: 2 },
    { ...safeFixture, authorization: "granted" },
    { ...safeFixture, ui_bridge_ready: false },
    null, "connected",
  ]) {
    const out = await probeNativeBridge(async () => forged);
    assert.equal(out.phase, NATIVE_BRIDGE_PHASE.INVALID);
    assert.equal(out.backend_verified, false);
  }
});

test("IPC errors expose no underlying host details and never fake connection", async () => {
  const out = await probeNativeBridge(async () => {
    throw new Error("private file or endpoint must never be displayed");
  });
  assert.deepEqual(out, { phase: NATIVE_BRIDGE_PHASE.FAILED, backend_verified: false });
});

const nativeInventoryFixture = Object.freeze({
  schema_version: 1,
  source: "native_in_memory_read_only",
  linked_modules: [
    "core_ledger", "model_registry", "model_market_contract",
    "gpu_transport_contract", "jan_integration_contract",
  ],
  modules_compiled: true,
  registry_records: 0,
  gui_owner_registered: false,
  approved_source_scans: 0,
  backend_verified: false,
  jan_connected: false,
  gpu_connected: false,
  cleanup_verified: false,
});

test("Native module summary is read-only and does not scan or start models", async () => {
  let called = 0;
  const result = await probeNativeModules(async (command) => {
    called++;
    assert.equal(command, "ui_native_module_status");
    return { ...nativeInventoryFixture };
  });
  assert.equal(called, 1);
  assert.deepEqual(result, {
    phase: NATIVE_MODULE_PHASE.LINKED, records: 0, backend_verified: false,
  });
});

test("Native module summary rejects forged connected or unscoped discovery", async () => {
  for (const change of [
    { backend_verified: true },
    { jan_connected: true },
    { gpu_connected: true },
    { cleanup_verified: true },
    { approved_source_scans: 1 },
    { gui_owner_registered: true },
    { modules_compiled: false },
    { registry_records: -1 },
    { linked_modules: ["model_registry"] },
    { source: "backend_verified" },
    { untrusted: "extra" },
  ]) {
    const report = await probeNativeModules(async () => ({ ...nativeInventoryFixture, ...change }));
    assert.equal(report.phase, NATIVE_MODULE_PHASE.INVALID);
  }
});

test("Native module summary fails closed in browser and on exceptions", async () => {
  const web = await probeNativeModules(undefined);
  assert.equal(web.phase, NATIVE_MODULE_PHASE.WEB_PREVIEW);
  const failed = await probeNativeModules(async () => { throw Error("private host path"); });
  assert.deepEqual(failed, { phase: NATIVE_MODULE_PHASE.FAILED, backend_verified: false });
});


test("QA-UI-01: Phase 4 empty-registry contract rejects every positive record count", async () => {
  for (const count of [1, 2, 100, Number.MAX_SAFE_INTEGER]) {
    const report = await probeNativeModules(async (command) => {
      assert.equal(command, "ui_native_module_status");
      return { ...nativeInventoryFixture, registry_records: count, approved_source_scans: 0 };
    });
    assert.deepEqual(report, {
      phase: NATIVE_MODULE_PHASE.INVALID, backend_verified: false,
    }, `forged record count ${count} must fail closed`);
  }
  const empty = await probeNativeModules(async () => ({ ...nativeInventoryFixture, registry_records: 0 }));
  assert.deepEqual(empty, { phase: NATIVE_MODULE_PHASE.LINKED, records: 0, backend_verified: false });
});

test("Phase 4 native status rejects inference/provider claims, scans and schema expansion", async () => {
  for (const mutation of [
    { inference_connected: true },
    { inference_running: true },
    { model_loaded: true },
    { hardware_active: true },
    { gpu_connected: true },
    { jan_connected: true },
    { backend_verified: true },
    { cleanup_verified: true },
    { approved_source_scans: 1 },
    { gui_owner_registered: true },
    { registry_records: "0" },
    { registry_records: 0.5 },
    { registry_records: null },
    { registry_records: -1 },
    { source: "backend_verified" },
    { schema_version: 2 },
    { linked_modules: [...nativeInventoryFixture.linked_modules, "inference_runtime"] },
  ]) {
    const status = await probeNativeModules(async () => ({ ...nativeInventoryFixture, ...mutation }));
    assert.deepEqual(status, { phase: NATIVE_MODULE_PHASE.INVALID, backend_verified: false },
      JSON.stringify(mutation));
  }
});

test("Browser-only fallback does not invoke native commands and never reports models", async () => {
  let calls = 0;
  const noNative = await probeNativeModules(undefined);
  assert.deepEqual(noNative, { phase: NATIVE_MODULE_PHASE.WEB_PREVIEW, backend_verified: false });
  assert.equal(calls, 0);
  const unexpectedFailure = await probeNativeModules(async () => {
    calls++;
    throw Error("No native command on this platform");
  });
  assert.equal(calls, 1);
  assert.deepEqual(unexpectedFailure, { phase: NATIVE_MODULE_PHASE.FAILED, backend_verified: false });
});
