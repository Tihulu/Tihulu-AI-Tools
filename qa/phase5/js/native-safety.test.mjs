// Independent QA regression tests; all data synthetic, no native processes.
import test from "node:test";
import assert from "node:assert/strict";
import {
  NATIVE_BRIDGE_PHASE, NATIVE_MODULE_PHASE, probeNativeBridge, probeNativeModules,
} from "../../../apps/tihulu-ai-web/native-bridge.mjs";
import {
  createMockUiAdapter, EVIDENCE, isUiRequest, uiRequest, UI_COMMANDS,
} from "../../../apps/tihulu-ai-web/ui-contract.mjs";
import { createDemoState, SHUTDOWN } from "../../../apps/tihulu-ai-web/state.mjs";

const native = {
  schema_version: 1, source: "native_in_memory_read_only",
  linked_modules: [
    "core_ledger", "model_registry", "model_market_contract",
    "gpu_transport_contract", "jan_integration_contract",
  ],
  modules_compiled: true, registry_records: 0, gui_owner_registered: false,
  approved_source_scans: 0, backend_verified: false, jan_connected: false,
  gpu_connected: false, cleanup_verified: false,
};

test("QA: web-only case never claims native presence", async () => {
  assert.equal((await probeNativeModules(undefined)).phase, NATIVE_MODULE_PHASE.WEB_PREVIEW);
  assert.equal((await probeNativeBridge(undefined)).phase, NATIVE_BRIDGE_PHASE.WEB_PREVIEW);
});
test("QA: native handshake never turns linked Rust crates into real inference", async () => {
  const commands = [];
  const out = await probeNativeModules(async (cmd) => {
    commands.push(cmd);
    return { ...native };
  });
  assert.deepEqual(commands, ["ui_native_module_status"]);
  assert.equal(out.phase, NATIVE_MODULE_PHASE.LINKED);
  assert.equal(out.backend_verified, false);
});
test("QA: forged provider connections, cleanup or unsolicited source scans fail closed", async () => {
  for (const mutation of [
    { backend_verified: true }, { gpu_connected: true }, { jan_connected: true },
    { cleanup_verified: true }, { approved_source_scans: 1 },
    { gui_owner_registered: true }, { source: "backend_verified" },
    { linked_modules: ["fake-module"] }, { leaked_path: "/etc/private" },
  ]) {
    const response = await probeNativeModules(async () => ({ ...native, ...mutation }));
    assert.equal(response.phase, NATIVE_MODULE_PHASE.INVALID, JSON.stringify(mutation));
    assert.equal(response.backend_verified, false);
  }
});
test("QA: an unapproved inventory count is not accepted as discovered models", async () => {
  // PR #19 reports a fresh empty in-memory registry and zero grants. Any
  // nonzero inventory in that response is inconsistent with that proof.
  const forged = await probeNativeModules(async () => ({
    ...native, registry_records: 1, approved_source_scans: 0,
  }));
  assert.equal(forged.phase, NATIVE_MODULE_PHASE.INVALID,
    "no approved scan and no registration must not yield discovered-model count");
});
test("QA: unconfirmed UI effects and payload-bearing paths are rejected", () => {
  for (const raw of [
    { schema_version: 1, request_id: "qa", kind: "execute_stop",
      payload: { action: SHUTDOWN.APP_QUIT, confirmed: false } },
    { schema_version: 1, request_id: "qa", kind: "read_models",
      payload: { root: "/etc" } },
    { schema_version: 1, request_id: "qa", kind: "shell",
      payload: { command: "whoami" } },
  ]) assert.equal(isUiRequest(raw), false);
  assert.throws(() => uiRequest(UI_COMMANDS.EXECUTE_STOP,
    { action: SHUTDOWN.APP_QUIT, confirmed: false }));
});
test("QA: stop cannot claim physical RAM/VRAM cleanup or stop independent host", () => {
  let current = createDemoState({ independentHost: true });
  const adapter = createMockUiAdapter({
    readState: () => current, writeState: (state) => { current = state; },
  });
  const execute = adapter.request(uiRequest(
    UI_COMMANDS.EXECUTE_STOP, { action: SHUTDOWN.APP_QUIT, confirmed: true },
  ));
  const host = adapter.request(uiRequest(UI_COMMANDS.HOST));
  assert.equal(execute.source, EVIDENCE.DEMO);
  assert.equal(execute.data.outcome, "simulated_only");
  assert.equal(execute.data.backend_verified, false);
  assert.equal(host.data.backend_verified, false);
  assert.equal(host.data.service_mode, "independent");
  assert.equal(host.data.active_other_clients, 2);
});
