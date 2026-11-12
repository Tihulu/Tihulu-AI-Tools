import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { translations, translate } from "../i18n.mjs";

const files = {
  css: readFileSync(new URL("../styles.css", import.meta.url), "utf8"),
  html: readFileSync(new URL("../index.html", import.meta.url), "utf8"),
  app: readFileSync(new URL("../app.mjs", import.meta.url), "utf8"),
};
const controls = [
  "overview", "market", "models", "apps", "devices", "host", "settings",
  "advisor", "agent", "jan", "stopAi", "appQuit", "hardStop", "quitAll",
  "hardware", "autodetect", "regular", "performance", "pi", "hailo", "custom",
  "setupJan", "howJan", "serviceMode", "enableService", "disableService",
  "serviceModeDemo", "serviceClientWarning", "serviceOptInWarning",
  "stopAiDesc", "appQuitDesc", "hardStopDesc", "quitAllDesc", "hostSurvives",
];

test("English default, Turkish and German supply all critical controls without fallback", () => {
  for (const lang of ["en", "tr", "de"]) {
    for (const key of controls) {
      assert.equal(typeof translations[lang][key], "string", lang + "/" + key);
      assert.ok(translations[lang][key].trim(), lang + "/" + key);
      assert.equal(translate(lang, key), translations[lang][key], lang + "/" + key);
    }
    assert.notEqual(translations[lang].stopAi, translations[lang].appQuit);
    assert.notEqual(translations[lang].hardStop, translations[lang].quitAll);
    assert.notEqual(translations[lang].enableService, translations[lang].disableService);
  }
  assert.equal(translate("en", "appQuit"), "App Quit");
  const englishKeys = Object.keys(translations.en).sort();
  for (const lang of ["tr", "de"]) assert.deepEqual(Object.keys(translations[lang]).sort(), englishKeys);
});

test("Responsive source includes desktop, tablet and narrow-screen patterns", () => {
  assert.match(files.html, /name="viewport" content="width=device-width, initial-scale=1"/);
  assert.match(files.css, /@media \(max-width: 1080px\)/);
  assert.match(files.css, /@media \(max-width: 760px\)/);
  assert.match(files.css, /\.nav\s*\{[^}]*overflow-x:\s*auto/s);
  assert.match(files.css, /\.table-scroll\s*\{[^}]*overflow-x:\s*auto/s);
  assert.match(files.css, /:focus-visible/);
  assert.match(files.css, /prefers-reduced-motion/);
  assert.match(files.html, /<dialog id="confirmation"/);
  assert.match(files.app, /aria-live="polite"/);
});

test("Web UI has no mandatory WebUSB, native calls, embedded remote fetch or service listener", () => {
  assert.doesNotMatch(files.app, /navigator\.usb\.|\bfetch\s*\(/);
  assert.doesNotMatch(files.app, /\binvoke\s*\(/);
  assert.match(files.html, /src="\.\/app\.mjs"/);
});

test("Rendered mock navigation contains all workspaces and localized shutdown & host controls", async () => {
  const listeners = new Map();
  const root = { innerHTML: "", addEventListener: (event, fn) => { listeners.set(event, fn); } };
  const noop = { textContent: "", hidden: false, addEventListener() {} };
  const modalBody = { innerHTML: "" };
  const dialog = { showModal() {}, close() {}, addEventListener() {} };
  const original = globalThis.document;
  globalThis.document = {
    documentElement: { lang: "" },
    querySelector(selector) {
      if (selector === "#app") return root;
      if (selector === "#confirmation") return dialog;
      if (selector === "#dialog-body") return modalBody;
      return noop;
    },
  };
  try {
    await import("../app.mjs?responsive-test");
    for (const key of ["overview", "market", "models", "apps", "devices", "host", "settings", "advisor", "agent", "jan"]) {
      assert.match(root.innerHTML, new RegExp('data-go="' + key + '"'));
    }
    assert.match(root.innerHTML, /Stop AI/);
    listeners.get("change")({ target: { id: "language-top", value: "tr" } });
    assert.equal(document.documentElement.lang, "tr");
    assert.ok(root.innerHTML.includes(translations.tr.appQuit));
    listeners.get("change")({ target: { id: "language-top", value: "de" } });
    assert.equal(document.documentElement.lang, "de");
    assert.ok(root.innerHTML.includes(translations.de.quitAll));
    listeners.get("click")({ target: { closest: () => ({ dataset: { shutdown: "app-quit" } }) } });
    assert.ok(modalBody.innerHTML.includes(translations.de.appQuitDesc));
    listeners.get("click")({ target: { closest: () => ({ dataset: { go: "host" } }) } });
    assert.ok(root.innerHTML.includes(translations.de.serviceDisabled));
    assert.ok(root.innerHTML.includes('data-service-mode="independent"'));
    assert.ok(root.innerHTML.includes(translations.de.serviceModeDemo));
  } finally {
    globalThis.document = original;
  }
});
