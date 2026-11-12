import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { INVENTORY_TEXT, inventoryT } from "../native-inventory-i18n.mjs";
import { createInventoryFixture, createInventoryViewState, reduceInventoryView } from "../native-inventory-model.mjs";
import { mountNativeInventoryWorkspace, renderNativeInventoryWorkspace } from "../native-inventory-view.mjs";

const styles = readFileSync(new URL("../native-inventory-view.css", import.meta.url), "utf8");
const source = readFileSync(new URL("../native-inventory-view.mjs", import.meta.url), "utf8");
const html = (tab, locale = "en") => {
  let state = createInventoryViewState();
  state = reduceInventoryView(state, { type: "locale", value: locale });
  state = reduceInventoryView(state, { type: "tab", value: tab });
  return renderNativeInventoryWorkspace(createInventoryFixture(), state);
};

test("all inventory UI strings exist in English, Turkish and German", () => {
  const keys = Object.keys(INVENTORY_TEXT.en).sort();
  assert.ok(keys.length >= 60);
  for (const locale of ["en", "tr", "de"]) {
    assert.deepEqual(Object.keys(INVENTORY_TEXT[locale]).sort(), keys);
    for (const key of keys) {
      assert.equal(typeof inventoryT(locale, key), "string");
      assert.ok(inventoryT(locale, key).trim(), locale + "/" + key);
    }
  }
  assert.equal(inventoryT("invalid", "modelTab"), INVENTORY_TEXT.en.modelTab);
});

test("inventory table shows only sample origin, format, owner, verification and no runtime proof", () => {
  const result = html("models");
  for (const value of ["DEMO", "Example Jan GGUF", "GGML", "GGUF",
    "Hugging Face", "Borrowed external", "Unverified", "Not checked", "demo://"]) {
    assert.ok(result.includes(value), value);
  }
  assert.match(result, /data-inv-screen="models"/);
  assert.match(result, /Native backend/);
  assert.match(result, /Unavailable/);
  assert.match(result, /Verified models/);
  assert.match(result, /Verified cleanups/);
  assert.doesNotMatch(result, />Connected</);
});

test("app details display installation location, active model root and unchanged legacy locations separately", () => {
  const result = html("apps");
  for (const name of ["Media Center", "Text Center", "Satellite Center", "Flight Radar",
    "RF Geolocation", "Survival Kit"]) assert.ok(result.includes(name), name);
  assert.match(result, /data-inv-path-type="installation"/);
  assert.match(result, /data-inv-path-type="active"/);
  assert.match(result, /demo:\/\/install\/tihulu-media-center/);
  assert.match(result, /demo:\/\/models\/apps\/tihulu-media-center/);
  assert.match(result, /demo:\/\/legacy\/tihulu-media-center\/models/);
  assert.match(result, /No path change, copy, move or migration/);
});

test("folder picker and authorization remain disabled even after the user reviews the demo scope", () => {
  const fixture = createInventoryFixture();
  let state = reduceInventoryView(createInventoryViewState(), { type: "tab", value: "sources" });
  const before = renderNativeInventoryWorkspace(fixture, state);
  assert.match(before, /Select actual folder \(native backend needed\)<\/button>/);
  assert.match(before, /type="button" disabled/);
  assert.match(before, /No real folder selected/);
  state = reduceInventoryView(state, { type: "review_scope" });
  const after = renderNativeInventoryWorkspace(fixture, state);
  assert.match(after, /type="checkbox" disabled/);
  assert.match(after, /Approve native scan \(unavailable\)<\/button>/);
  assert.match(after, /View-only preview/);
  assert.match(after, /No folder picker, access token, scan, grant or consent/);
  assert.equal(fixture.native_grants, 0);
  assert.equal(fixture.native_scans_completed, 0);
});

test("Jan, Ollama and GPU Host status remains unknown, without claiming detection", () => {
  const result = html("providers");
  for (const value of ["Jan", "Ollama", "GPU Host", "Not checked", "Not performed",
    "OFF by default", "GUI closure"]) assert.ok(result.includes(value), value);
  assert.doesNotMatch(result, /Connected \(verified\)/);
});

test("resource indicators do not claim live memory or verified cleanup", () => {
  const result = html("ownership");
  assert.match(result, /GUI session/);
  assert.match(result, /Independent host/);
  assert.match(result, /External borrowed services/);
  assert.match(result, /Not executed/);
  assert.match(result, /Never terminate/);
  assert.match(result, /Unknown/);
  assert.doesNotMatch(result, /Released \(verified\)/);
});

test("render fails closed on forged real/discovery/cleanup status and escapes HTML", () => {
  const forged = createInventoryFixture();
  forged.native_cleanup_verified = true;
  assert.throws(() => renderNativeInventoryWorkspace(forged), /Unsafe/);
  const unsafe = createInventoryFixture();
  unsafe.models[0].name = '<img src=x onerror=alert(1)>';
  const result = renderNativeInventoryWorkspace(unsafe);
  assert.doesNotMatch(result, /<img /);
  assert.match(result, /&lt;img/);
});

test("controller mount, language change and tab controls use only in-memory state", () => {
  const handlers = new Map();
  const root = {
    innerHTML: "",
    addEventListener(name, cb) { handlers.set(name, cb); },
    removeEventListener(name, cb) { if (handlers.get(name) === cb) handlers.delete(name); },
  };
  const controller = mountNativeInventoryWorkspace(root);
  assert.match(root.innerHTML, /data-inv-screen="models"/);
  handlers.get("change")({ target: { value: "tr", dataset: { invLocale: "" } } });
  assert.equal(controller.state().locale, "tr");
  assert.match(root.innerHTML, /Yerel model envanteri/);
  handlers.get("click")({ target: { closest: () => ({ dataset: { invTab: "sources" } }) } });
  assert.match(root.innerHTML, /data-inv-screen="sources"/);
  handlers.get("click")({ target: { closest: () => ({ dataset: { invReview: "" } }) } });
  assert.equal(controller.state().consent_preview, "review_only");
  assert.match(root.innerHTML, /Sadece önizleme/);
  handlers.get("change")({ target: { value: "de", dataset: { invLocale: "" } } });
  assert.match(root.innerHTML, /Ordnerfreigabe/);
  controller.dispose();
  assert.equal(handlers.size, 0);
});

test("responsive CSS and isolation: no WebUSB, browser picker, fetch or native command", () => {
  assert.match(styles, /@media \(max-width: 950px\)/);
  assert.match(styles, /@media \(max-width: 640px\)/);
  assert.match(styles, /prefers-reduced-motion/);
  assert.match(styles, /overflow-x: auto/);
  assert.match(styles, /:focus-visible/);
  assert.doesNotMatch(source, /\bfetch\s*\(|\binvoke\s*\(|navigator\.usb|showDirectoryPicker|__TAURI__/);
  assert.doesNotMatch(source, /from ["']\.\/app\.mjs|from ["']\.\/native-bridge\.mjs/);
});
