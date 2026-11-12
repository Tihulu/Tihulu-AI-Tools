import test from "node:test";
import assert from "node:assert/strict";
import { EXPECTED_MODULES, validBackendStatus, validNativeModules, liveReadonlyProbe } from "./live-ipc-probe.mjs";

const backend = () => ({
  schema_version: 1, source: "demo_fixture", transport: "tauri_ipc",
  ui_bridge_ready: true, backend_verified: false,
  hardware_active: false, cleanup_verified: false,
});
const modules = () => ({
  schema_version: 1, source: "native_in_memory_read_only",
  linked_modules: [...EXPECTED_MODULES], modules_compiled: true,
  registry_records: 0, gui_owner_registered: false, approved_source_scans: 0,
  backend_verified: false, jan_connected: false, gpu_connected: false,
  cleanup_verified: false,
});

test("validates the exact read-only native DTO contracts", async () => {
  assert.equal(validBackendStatus(backend()), true);
  assert.equal(validNativeModules(modules()), true);
  let calls = [];
  assert.equal(await liveReadonlyProbe(async (command) => {
    calls.push(command);
    if (command === "ui_backend_status") return backend();
    if (command === "ui_native_module_status") return modules();
    throw new Error("unexpected command");
  }), true);
  assert.deepEqual(calls, ["ui_backend_status", "ui_native_module_status"]);
});

test("no native invoke means fail, even if browser UI renders", async () => {
  assert.equal(await liveReadonlyProbe(undefined), false);
  assert.equal(await liveReadonlyProbe(null), false);
});

test("rejects forged hardware and backend ready claims", async () => {
  for (const key of ["backend_verified", "hardware_active", "cleanup_verified"]) {
    assert.equal(validBackendStatus({ ...backend(), [key]: true }), false);
  }
});

test("rejects wrong transport/source/schema and unsolicited keys", () => {
  assert.equal(validBackendStatus({ ...backend(), transport: "fake_ipc" }), false);
  assert.equal(validBackendStatus({ ...backend(), source: "native_verified" }), false);
  assert.equal(validBackendStatus({ ...backend(), schema_version: 2 }), false);
  assert.equal(validBackendStatus({ ...backend(), secret: "value" }), false);
});

test("rejects real model scans, owners, services and GPU claims", () => {
  for (const [key, value] of [
    ["registry_records", 1], ["gui_owner_registered", true],
    ["approved_source_scans", 1], ["backend_verified", true],
    ["jan_connected", true], ["gpu_connected", true],
    ["cleanup_verified", true], ["modules_compiled", false],
  ]) {
    assert.equal(validNativeModules({ ...modules(), [key]: value }), false, key);
  }
});

test("rejects duplicate, reordered, missing and forged linked modules", () => {
  for (const linked_modules of [
    [], EXPECTED_MODULES.slice(1),
    [...EXPECTED_MODULES].reverse(),
    [...EXPECTED_MODULES, "secret"],
  ]) {
    assert.equal(validNativeModules({ ...modules(), linked_modules }), false);
  }
});

test("native errors remain HOLD; never become a synthetic PASS", async () => {
  assert.equal(await liveReadonlyProbe(async () => { throw new Error("offline"); }), false);
  assert.equal(await liveReadonlyProbe(async (cmd) => cmd === "ui_backend_status" ? backend() : null), false);
});
