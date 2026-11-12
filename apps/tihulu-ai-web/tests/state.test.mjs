import test from "node:test";
import assert from "node:assert/strict";
import { SHUTDOWN, applyShutdown, closeChat, createDemoState, getResource, getTotals, previewShutdown, sendMockMessage } from "../state.mjs";

const status = (state, id) => getResource(state, id).status;

test("L-01 App Quit releases all UI-owned model RAM/VRAM fixture without deleting external models", () => {
  const initial = createDemoState();
  const after = applyShutdown(initial, SHUTDOWN.APP_QUIT);
  assert.equal(getTotals(after).mockRamMiB, 0);
  assert.equal(getTotals(after).mockVramMiB, 0);
  assert.equal(status(after, "model"), "released");
  assert.equal(status(after, "jan"), "active");
  assert.equal(status(initial, "model"), "active"); // immutable transition
});

test("L-02 closing Advisor leaves Agent and shared model context", () => {
  const after = closeChat(createDemoState(), "advisor");
  assert.equal(status(after, "advisor"), "released");
  assert.equal(status(after, "agent"), "active");
  assert.equal(status(after, "model"), "active");
  assert.equal(status(closeChat(after, "agent"), "model"), "released");
});

test("L-03 host lease cleanup keeps independent host and its other clients", () => {
  const after = applyShutdown(createDemoState(), SHUTDOWN.APP_QUIT);
  assert.equal(status(after, "host-lease"), "released");
  assert.equal(status(after, "host"), "active");
  assert.equal(getResource(after, "host").clients, 2);
});

test("L-05 Global Hard Stop releases UI-owned simulated SDR, not readsb", () => {
  const after = applyShutdown(createDemoState(), SHUTDOWN.HARD_STOP);
  assert.equal(status(after, "sdr"), "released");
  assert.equal(status(after, "readsb"), "active");
});

test("L-12 independent host survives App Quit and warning is visible", () => {
  const state = createDemoState();
  assert.ok(previewShutdown(state, SHUTDOWN.APP_QUIT).warnings.some((x) => x.includes("Independent GPU host")));
  assert.equal(status(applyShutdown(state, SHUTDOWN.APP_QUIT), "host"), "active");
});

test("L-13 Quit All explicitly warns clients and leaves borrowed external untouched", () => {
  const state = createDemoState();
  const preview = previewShutdown(state, SHUTDOWN.QUIT_ALL);
  assert.ok(preview.warnings.some((x) => x.includes("authorized host clients")));
  const after = applyShutdown(state, SHUTDOWN.QUIT_ALL);
  assert.equal(status(after, "host"), "released");
  assert.equal(getResource(after, "host").clients, 0);
  assert.equal(status(after, "jan"), "active");
  assert.equal(status(after, "readsb"), "active");
});

test("Stop AI does not release SDR; shutdown requests are idempotent", () => {
  const once = applyShutdown(createDemoState(), SHUTDOWN.STOP_AI);
  assert.equal(status(once, "sdr"), "active");
  assert.equal(status(once, "model"), "released");
  const twice = applyShutdown(once, SHUTDOWN.STOP_AI);
  assert.deepEqual(twice.resources, once.resources);
  assert.deepEqual(twice.history[0].affected, []);
});

test("default is On-Demand, Background OFF, no keep warm; no real Jan status", () => {
  const state = createDemoState();
  assert.equal(state.background, false);
  assert.equal(state.keepWarm, false);
  assert.equal(state.janStatus, "not_configured");
  assert.equal(state.chat.advisor.length, 0);
});

test("demo chat never exposes fake assistant inference and refuses after App Quit", () => {
  const state = sendMockMessage(createDemoState(), "agent", "<script>alert(1)</script>");
  assert.equal(state.chat.agent.length, 2);
  assert.match(state.chat.agent[1].text, /No model, tool or inference/);
  const afterQuit = applyShutdown(state, SHUTDOWN.APP_QUIT);
  assert.strictEqual(sendMockMessage(afterQuit, "agent", "another"), afterQuit);
});
