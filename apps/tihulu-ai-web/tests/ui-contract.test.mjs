import test from "node:test";
import assert from "node:assert/strict";
import { applyShutdown, createDemoState, getResource, SHUTDOWN } from "../state.mjs";
import { createMockUiAdapter, EVIDENCE, isUiRequest, MODEL_FIXTURES, SERVICE_MODES, UI_COMMANDS, uiRequest } from "../ui-contract.mjs";

const fixture = (state = createDemoState()) => {
  let current = state;
  const adapter = createMockUiAdapter({ readState: () => current, writeState: (next) => { current = next; } });
  return { adapter, state: () => current };
};
const read = (adapter, kind, payload = {}) => adapter.request(uiRequest(kind, payload));

test("Versioned typed UI request rejects unknown commands, wrong schema and unconfirmed effects", () => {
  assert.ok(isUiRequest(uiRequest(UI_COMMANDS.OVERVIEW)));
  assert.equal(isUiRequest({ schema_version: 2, request_id: "test", kind: "read_overview", payload: {} }), false);
  assert.equal(isUiRequest({ schema_version: 1, request_id: "test", kind: "shell", payload: {} }), false);
  assert.equal(isUiRequest({ schema_version: 1, request_id: "test", kind: "execute_stop", payload: { action: "hard-stop", confirmed: false } }), false);
  assert.equal(isUiRequest({ schema_version: 1, request_id: "test", kind: "set_service_mode", payload: { mode: "independent", confirmed: false } }), false);
  assert.throws(() => uiRequest(UI_COMMANDS.EXECUTE_STOP, { action: SHUTDOWN.QUIT_ALL, confirmed: false }), TypeError);
});

test("Model availability is distinct from runtime readiness; no fake verification", () => {
  const { adapter } = fixture();
  const models = read(adapter, UI_COMMANDS.MODELS);
  assert.equal(models.source, EVIDENCE.DEMO);
  assert.deepEqual(models.data.models, MODEL_FIXTURES.map((m) => ({ ...m })));
  const compat = read(adapter, UI_COMMANDS.COMPATIBILITY, {
    model_id: "demo:external-gguf", task_id: "chat", host_id: "paired:unknown",
  });
  assert.equal(compat.data.verdict, "unverified");
  assert.equal(compat.source, EVIDENCE.DEMO);
  assert.ok(compat.data.reasons.length);
});

test("GUI closure does not stop opted-in independent GPU service or authorized other clients", () => {
  const { adapter, state } = fixture(createDemoState({ independentHost: true }));
  assert.equal(read(adapter, UI_COMMANDS.HOST).data.service_mode, SERVICE_MODES.INDEPENDENT);
  read(adapter, UI_COMMANDS.EXECUTE_STOP, { action: SHUTDOWN.APP_QUIT, confirmed: true });
  const host = read(adapter, UI_COMMANDS.HOST).data;
  assert.equal(host.gui_phase, "closed");
  assert.equal(host.service_phase, "active");
  assert.equal(host.active_other_clients, 2);
  assert.equal(host.backend_verified, false);
  assert.equal(getResource(state(), "readsb").status, "active");
});

test("Host service mode starts OFF and requires explicit review for demo transitions", () => {
  const { adapter } = fixture();
  assert.equal(read(adapter, UI_COMMANDS.HOST).data.service_mode, "disabled");
  assert.equal(read(adapter, UI_COMMANDS.HOST).data.service_phase, "stopped");
  const enabled = read(adapter, UI_COMMANDS.SERVICE_MODE, { mode: "independent", confirmed: true });
  assert.equal(enabled.data.outcome, "simulated_only");
  assert.equal(read(adapter, UI_COMMANDS.HOST).data.active_other_clients, 2);
  read(adapter, UI_COMMANDS.SERVICE_MODE, { mode: "disabled", confirmed: true });
  assert.equal(read(adapter, UI_COMMANDS.HOST).data.service_phase, "stopped");
});

test("STOP, Hard Stop and Quit All are typed, require confirmation and never claim cleanup verification", () => {
  for (const action of Object.values(SHUTDOWN)) {
    const { adapter } = fixture(createDemoState({ independentHost: true }));
    const preview = read(adapter, UI_COMMANDS.PREVIEW_STOP, { action });
    assert.equal(preview.data.backend_verified, false);
    assert.ok(preview.data.warnings.some((text) => text.includes("DEMO")));
    const executed = read(adapter, UI_COMMANDS.EXECUTE_STOP, { action, confirmed: true });
    assert.equal(executed.data.outcome, "simulated_only");
    assert.equal(executed.data.backend_verified, false);
  }
});

test("Jan wizard state is not a verified handshake or installed MCP executable", () => {
  const { adapter } = fixture();
  const jan = read(adapter, UI_COMMANDS.JAN);
  assert.equal(jan.source, EVIDENCE.DEMO);
  assert.equal(jan.data.connection, "not_configured");
  assert.equal(jan.data.verified_handshake, false);
  assert.equal(jan.data.registered, false);
  assert.equal(jan.data.executable_available, false);
});

test("Advisor/Agent chat snapshots and messages are never real inference", () => {
  const { adapter } = fixture();
  assert.equal(read(adapter, UI_COMMANDS.CHAT, { agent: "advisor" }).data.inference_connected, false);
  const sent = adapter.sendChat("advisor", "hello");
  assert.equal(sent.source, EVIDENCE.DEMO);
  assert.equal(sent.inference_connected, false);
  const transcript = read(adapter, UI_COMMANDS.CHAT, { agent: "advisor" }).data.messages;
  assert.equal(transcript.length, 2);
  assert.match(transcript[1].text, /no model|No model/i);
  assert.equal(read(adapter, UI_COMMANDS.CHAT, { agent: "agent" }).data.messages.length, 0);
});

test("Jan wizard is typed and remains unconfigured through every mock stage", () => {
  const { adapter } = fixture();
  for (const step of [0, 1, 2, 3, -1]) {
    const outcome = read(adapter, UI_COMMANDS.JAN_STEP, { step });
    assert.equal(outcome.data.outcome, "simulated_only");
    assert.equal(read(adapter, UI_COMMANDS.JAN).data.wizard_step, step);
    assert.equal(read(adapter, UI_COMMANDS.JAN).data.verified_handshake, false);
  }
  assert.equal(isUiRequest({ schema_version: 1, request_id: "jan", kind: "set_jan_step", payload: { step: 99 } }), false);
});

test("Closed GUI denies new mock host changes, Jan writes and chat messages", () => {
  const { adapter } = fixture(createDemoState({ independentHost: true }));
  read(adapter, UI_COMMANDS.EXECUTE_STOP, { action: SHUTDOWN.APP_QUIT, confirmed: true });
  assert.throws(() => read(adapter, UI_COMMANDS.SERVICE_MODE, { mode: "disabled", confirmed: true }), /GUI session closed/);
  assert.throws(() => read(adapter, UI_COMMANDS.JAN_STEP, { step: 0 }), /GUI session closed/);
  assert.throws(() => adapter.sendChat("advisor", "after quit"), /GUI session closed/);
  assert.equal(read(adapter, UI_COMMANDS.HOST).data.service_phase, "active");
});

test("Phase 3: envelope rejects unknown fields, paths and oversized model identities", () => {
  const bad = [
    { schema_version: 1, request_id: "test", kind: "read_overview", payload: {}, backend_verified: true },
    { schema_version: 1, request_id: "test", kind: "read_models", payload: { authenticated: true } },
    { schema_version: 1, request_id: "test", kind: "read_compatibility", payload: { model_id: "/tmp/model.gguf", task_id: "chat", host_id: "pc" } },
    { schema_version: 1, request_id: "test", kind: "read_compatibility", payload: { model_id: "a".repeat(129), task_id: "chat", host_id: "pc" } },
    { schema_version: 1, request_id: "test", kind: "read_compatibility", payload: { model_id: "demo:model", task_id: "..", host_id: "pc" } },
    { schema_version: 1, request_id: "test", kind: "execute_stop", payload: { action: SHUTDOWN.APP_QUIT, confirmed: true, owner_id: "other" } },
  ];
  for (const request of bad) assert.equal(isUiRequest(request), false, JSON.stringify(request));
  assert.equal(isUiRequest(uiRequest(UI_COMMANDS.COMPATIBILITY, { model_id: "demo:external-gguf", task_id: "chat", host_id: "paired.host-01" })), true);
});

test("Phase 3: chat limits use UTF-8 bytes and reject extra tool permissions", () => {
  assert.equal(isUiRequest({ schema_version: 1, request_id: "chat", kind: "send_chat", payload: { agent: "advisor", message: "😀".repeat(200) } }), false);
  assert.equal(isUiRequest({ schema_version: 1, request_id: "chat", kind: "send_chat", payload: { agent: "advisor", message: String.fromCharCode(0xd800) } }), false);
  assert.equal(isUiRequest({ schema_version: 1, request_id: "chat", kind: "send_chat", payload: { agent: "advisor", message: "hi", unrestricted_shell: true } }), false);
  assert.equal(isUiRequest(uiRequest(UI_COMMANDS.SEND_CHAT, { agent: "advisor", message: "Hi" })), true);
});

test("Phase 3: all mock command envelopes retain demo provenance and never claim resource cleanup", () => {
  const { adapter } = fixture(createDemoState({ independentHost: true }));
  const requests = [
    [UI_COMMANDS.OVERVIEW, {}], [UI_COMMANDS.MODELS, {}],
    [UI_COMMANDS.COMPATIBILITY, { model_id: "demo:onnx", task_id: "vision", host_id: "local" }],
    [UI_COMMANDS.HOST, {}], [UI_COMMANDS.CHAT, { agent: "advisor" }], [UI_COMMANDS.JAN, {}],
    [UI_COMMANDS.PREVIEW_STOP, { action: SHUTDOWN.APP_QUIT }],
  ];
  for (const [kind, payload] of requests) {
    const response = read(adapter, kind, payload);
    assert.equal(response.source, EVIDENCE.DEMO);
    assert.equal(response.kind, kind);
    assert.equal(response.schema_version, 1);
    assert.equal(response.request_id, "ui-demo-01");
  }
  const response = read(adapter, UI_COMMANDS.EXECUTE_STOP, { action: SHUTDOWN.APP_QUIT, confirmed: true });
  assert.equal(response.source, EVIDENCE.DEMO);
  assert.equal(response.data.outcome, "simulated_only");
  assert.equal(response.data.backend_verified, false);
});
