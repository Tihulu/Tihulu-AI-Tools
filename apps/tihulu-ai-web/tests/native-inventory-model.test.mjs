import test from "node:test";
import assert from "node:assert/strict";
import {
  INVENTORY_BACKEND, INVENTORY_SCHEMA, INVENTORY_SOURCE,
  SOURCE_OPTIONS, createInventoryFixture, createInventoryViewState,
  reduceInventoryView, validateInventoryFixture,
} from "../native-inventory-model.mjs";

const tamper = (change) => {
  const fixture = createInventoryFixture();
  change(fixture);
  return fixture;
};

test("fixture starts disconnected with zero native grants, scans, and cleanup claims", () => {
  const data = createInventoryFixture();
  assert.equal(data.schema_version, INVENTORY_SCHEMA);
  assert.equal(data.source, INVENTORY_SOURCE);
  assert.equal(data.backend, INVENTORY_BACKEND);
  assert.equal(data.native_scans_completed, 0);
  assert.equal(data.native_grants, 0);
  assert.equal(data.native_cleanup_verified, false);
  assert.equal(validateInventoryFixture(data), true);
  assert.equal(data.models.length, 6);
  assert.equal(data.apps.length, 6);
  assert.equal(data.providers.length, 3);
  assert.ok(data.models.every((m) => m.verification === "unverified" && m.runtime === "not_checked"));
  assert.ok(data.providers.every((p) => p.state === "not_checked" && !p.connection_verified));
});

test("native inventory can never be promoted to real or verified by a UI-only fixture", () => {
  const changes = [
    (d) => { d.source = "backend_verified"; },
    (d) => { d.backend = "connected"; },
    (d) => { d.native_scans_completed = 1; },
    (d) => { d.native_grants = 1; },
    (d) => { d.native_cleanup_verified = true; },
    (d) => { d.models[0].verification = "verified"; },
    (d) => { d.models[0].runtime = "ready"; },
    (d) => { d.apps[0].path_verified = true; },
    (d) => { d.apps[0].install_state = "installed"; },
    (d) => { d.providers[0].state = "connected"; },
    (d) => { d.providers[1].connection_verified = true; },
    (d) => { d.resources[0].cleanup_state = "released"; },
    (d) => { d.resources[1].lease_count = 0; },
    (d) => { d.verified_by = "user"; },
  ];
  for (const change of changes) assert.equal(validateInventoryFixture(tamper(change)), false);
});

test("source and model paths are example aliases rather than host filesystem paths", () => {
  const fixture = createInventoryFixture();
  assert.ok(fixture.models.every((m) => m.location.startsWith("demo://")));
  assert.ok(fixture.apps.every((a) => a.installation_path !== a.active_model_root));
  assert.ok(fixture.apps.every((a) => a.legacy_model_roots.length > 0));
  assert.equal(validateInventoryFixture(tamper((d) => { d.apps[0].active_model_root = "/home/user/private"; })), false);
  assert.equal(validateInventoryFixture(tamper((d) => { d.models[1].location = "demo://user/../secrets"; })), false);
  assert.equal(validateInventoryFixture(tamper((d) => { d.models[0].model_id = "demo/model"; })), false);
});

test("borrowed Jan/Ollama/HF models stay borrowed and cannot be presented as TIHULU-owned", () => {
  const data = createInventoryFixture();
  for (const origin of ["jan", "ollama", "hugging_face"]) {
    assert.ok(data.models.some((m) => m.origin === origin && m.ownership === "borrowed_external"));
  }
  assert.equal(validateInventoryFixture(tamper((d) => { d.models[1].ownership = "app_managed"; })), false);
});

test("UI preview state is local and cannot grant a native filesystem scope", () => {
  const initial = createInventoryViewState();
  assert.equal(initial.locale, "en");
  assert.equal(initial.tab, "models");
  let current = reduceInventoryView(initial, { type: "tab", value: "sources" });
  assert.equal(current.tab, "sources");
  for (const option of SOURCE_OPTIONS) {
    current = reduceInventoryView(current, { type: "source", value: option });
    assert.equal(current.source_kind, option);
  }
  current = reduceInventoryView(current, { type: "review_scope" });
  assert.equal(current.consent_preview, "review_only");
  assert.equal(Object.hasOwn(current, "grant_id"), false);
  assert.equal(Object.hasOwn(current, "native_permission"), false);
  current = reduceInventoryView(current, { type: "dismiss_scope" });
  assert.equal(current.consent_preview, "idle");
  assert.strictEqual(reduceInventoryView(current, { type: "native_scan", value: true }), current);
  assert.strictEqual(reduceInventoryView(current, { type: "source", value: "../../private" }), current);
  assert.equal(initial.tab, "models");
});
