import test from "node:test";
import assert from "node:assert/strict";
import { getGlobalNativeInvoke, NATIVE_BRIDGE_PHASE, probeNativeBridge } from "../native-bridge.mjs";

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
