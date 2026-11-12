/**
 * Isolated Phase 5 UI. The coordinator may mount this AFTER Registry acceptance.
 * No imports from app.mjs/native-bridge.mjs, Tauri, WebUSB or network.
 */
import {
  createInventoryFixture, createInventoryViewState,
  reduceInventoryView, validateInventoryFixture, SOURCE_OPTIONS,
} from "./native-inventory-model.mjs";
import { inventoryT } from "./native-inventory-i18n.mjs";

const esc = (value) => String(value ?? "").replace(/[&<>"']/g, (char) => ({
  "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;",
})[char]);

const originKey = {
  tihulu_app: "sourceApp", jan: "sourceJan", ollama: "sourceOllama",
  hugging_face: "sourceHf", user_folder: "sourceUser",
};
const ownerKey = { app_managed: "ownedApp", borrowed_external: "borrowed", user_managed: "userManaged" };
const providerKey = { jan: "jan", ollama: "ollama", gpu_host: "gpuHost" };
const resourceKey = { gui_session: "ownedGui", independent_host: "ownedHost", borrowed_external: "ownedBorrowed" };
const sourceKey = {
  custom_folder: "customFolder", external_ssd: "externalSsd",
  jan: "janFolder", ollama: "ollamaFolder", hugging_face: "hfFolder",
};
const tabs = [
  ["models", "modelTab"], ["apps", "appsTab"], ["sources", "sourcesTab"],
  ["providers", "providersTab"], ["ownership", "ownershipTab"],
];
const pill = (text, tone = "muted") => `<span class="inv-pill inv-${tone}">${esc(text)}</span>`;
const heading = (title, description) =>
  `<header class="inv-heading"><h2>${esc(title)}</h2><p>${esc(description)}</p></header>`;
const tFor = (state) => (key) => inventoryT(state.locale, key);

function modelsScreen(data, t) {
  const rows = data.models.map((m) =>
    `<tr><th scope="row"><strong>${esc(m.name)}</strong><small>${esc(m.model_id)}</small></th>
      <td>${esc(t(originKey[m.origin]))}</td><td>${esc(m.format.toUpperCase())}</td>
      <td>${esc(t(ownerKey[m.ownership]))}</td>
      <td>${pill(t("unverified"), "warning")}</td>
      <td>${pill(t("notChecked"))}</td>
      <td><code>${esc(m.location)}</code></td></tr>`).join("");
  return heading(t("modelInventory"), t("modelHint")) +
    `<div class="inv-scroll"><table class="inv-table">
      <thead><tr><th scope="col">${esc(t("modelName"))}</th><th scope="col">${esc(t("origin"))}</th>
      <th scope="col">${esc(t("format"))}</th><th scope="col">${esc(t("owner"))}</th>
      <th scope="col">${esc(t("verification"))}</th><th scope="col">${esc(t("runtime"))}</th>
      <th scope="col">${esc(t("directory"))}</th></tr></thead><tbody>${rows}</tbody></table></div>`;
}

function appsScreen(data, state, t) {
  const app = data.apps.find((record) => record.app_id === state.selected_app) ?? data.apps[0];
  const selector = data.apps.map((a) =>
    `<option value="${esc(a.app_id)}"${app?.app_id === a.app_id ? " selected" : ""}>${esc(a.name)}</option>`).join("");
  const pathCard = (title, path, type) =>
    `<section class="inv-path"><h3>${esc(title)}</h3>${pill(t("demo"), "warning")}
       <code data-inv-path-type="${type}">${esc(path)}</code></section>`;
  return heading(t("appHeading"), t("appHint")) +
    `<label class="inv-field">${esc(t("selectApp"))}
       <select data-inv-app aria-label="${esc(t("selectApp"))}">${selector}</select></label>
       <p class="inv-note">${esc(t("notInstalled"))} · ${esc(t("noMigration"))}</p>
       <div class="inv-path-grid">
       ${pathCard(t("appInstall"), app.installation_path, "installation")}
       ${pathCard(t("activeRoot"), app.active_model_root, "active")}
       <section class="inv-path"><h3>${esc(t("legacyRoots"))}</h3>${pill(t("demo"), "warning")}
         <ul>${app.legacy_model_roots.map((p) => `<li><code>${esc(p)}</code></li>`).join("")}</ul>
       </section></div>`;
}

function sourcesScreen(state, t) {
  const sourceOptions = SOURCE_OPTIONS.map((source) =>
    `<option value="${source}"${state.source_kind === source ? " selected" : ""}>${esc(t(sourceKey[source]))}</option>`).join("");
  const consent = state.consent_preview === "review_only" ?
    `<section class="inv-consent" aria-live="polite">
      <h3>${esc(t("consentTitle"))}</h3>
      <ul><li>${esc(t("consentScope"))}</li><li>${esc(t("consentAllowed"))}</li>
        <li>${esc(t("consentForbidden"))}</li><li>${esc(t("consentOwnership"))}</li></ul>
      <p class="inv-note">${esc(t("consentNotice"))}</p>
      ${pill(t("reviewedOnly"), "warning")}
      <label class="inv-disabled-consent"><input type="checkbox" disabled>
        ${esc(t("consentScope"))}</label>
      <div class="inv-actions"><button type="button" disabled>${esc(t("approveDisabled"))}</button>
      <button type="button" data-inv-dismiss>${esc(t("dismiss"))}</button></div>
    </section>` : "";
  return heading(t("sourceHeading"), t("sourceHint")) +
    `<div class="inv-source-fields">
       <label class="inv-field">${esc(t("sourceType"))}
       <select data-inv-source aria-label="${esc(t("sourceType"))}">${sourceOptions}</select></label>
       <label class="inv-field">${esc(t("folderUnknown"))}<input aria-label="${esc(t("folderUnknown"))}" disabled value="${esc(t("folderUnknown"))}"></label>
     </div><div class="inv-actions">
       <button type="button" disabled title="${esc(t("unavailable"))}">${esc(t("nativePicker"))}</button>
       <button type="button" data-inv-review>${esc(t("reviewScope"))}</button>
     </div>${consent}`;
}

function providerScreen(data, t) {
  return heading(t("providersHeading"), t("providerHint")) +
    `<div class="inv-provider-grid">${data.providers.map((p) =>
      `<section class="inv-provider"><h3>${esc(t(providerKey[p.provider_id]))}</h3>
        <p>${esc(t("providerState"))}: ${pill(t("notChecked"), "warning")}</p>
        <p>${esc(t("lastProbe"))}: ${esc(t("noProbe"))}</p>
        <p>${esc(t("demo"))}</p></section>`).join("")}</div>
       <p class="inv-note">${esc(t("hostPolicy"))}: ${esc(t("hostOff"))}.
         ${esc(t("guiSeparate"))}</p>`;
}

function ownershipScreen(data, t) {
  return heading(t("ownershipHeading"), t("ownershipHint")) +
    `<div class="inv-scroll"><table class="inv-table">
       <thead><tr><th scope="col">${esc(t("owner"))}</th>
       <th scope="col">${esc(t("leaseCount"))}</th><th scope="col">${esc(t("cleanupState"))}</th></tr></thead>
       <tbody>${data.resources.map((r) =>
         `<tr><th scope="row">${esc(t(resourceKey[r.owner_kind]))}</th>
         <td>${esc(t("unknown"))}</td><td>${pill(t(r.cleanup_state === "not_applicable" ? "notApplicable" : "notExecuted"), "warning")}</td></tr>`).join("")}
       </tbody></table></div><p class="inv-note">${esc(t("noRelease"))}</p>`;
}

/** Builds plain HTML for the approved *mock-only* fixture, escaping all displayed strings. */
export function renderNativeInventoryWorkspace(snapshot, state = createInventoryViewState()) {
  if (!validateInventoryFixture(snapshot)) throw new TypeError("Unsafe or non-mock inventory source");
  const t = tFor(state);
  const locale = ["en", "tr", "de"].includes(state.locale) ? state.locale : "en";
  const activeTab = tabs.some(([id]) => id === state.tab) ? state.tab : "models";
  let body;
  switch (activeTab) {
    case "apps": body = appsScreen(snapshot, state, t); break;
    case "sources": body = sourcesScreen(state, t); break;
    case "providers": body = providerScreen(snapshot, t); break;
    case "ownership": body = ownershipScreen(snapshot, t); break;
    default: body = modelsScreen(snapshot, t);
  }
  const nav = tabs.map(([id, label]) =>
    `<button type="button" class="inv-tab${id === activeTab ? " inv-selected" : ""}"
      data-inv-tab="${id}"${id === activeTab ? ' aria-current="page"' : ""}>${esc(t(label))}</button>`).join("");
  return `<section class="inv-workspace" lang="${locale}" aria-label="${esc(t("title"))}">
    <div class="inv-top"><div><h1>${esc(t("title"))}</h1><p>${esc(t("subtitle"))}</p></div>
      <label class="inv-field">${esc(t("language"))}<select data-inv-locale aria-label="${esc(t("language"))}">
       ${[["en","English"],["tr","Türkçe"],["de","Deutsch"]].map(([id,name]) =>
         `<option value="${id}"${locale === id ? " selected" : ""}>${name}</option>`).join("")}
       </select></label></div>
    <div class="inv-warning" role="status"><strong>${esc(t("banner"))}</strong></div>
    <p class="inv-backend">${esc(t("backend"))}: ${pill(t("unavailable"), "warning")} · ${esc(t("demo"))}</p>
    <div class="inv-metrics">
      ${[[t("exampleModels"), snapshot.models.length], [t("verifiedModels"), 0],
          [t("approvedFolders"), snapshot.native_grants], [t("cleanups"), 0]].map(([key,value]) =>
         `<div class="inv-metric"><span>${esc(key)}</span><strong>${value}</strong></div>`).join("")}
    </div>
    <nav class="inv-tabs" aria-label="${esc(t("title"))}">${nav}</nav>
    <section class="inv-main" aria-label="${esc(t("title"))}" data-inv-screen="${activeTab}">${body}</section>
    <footer class="inv-foot">${esc(t("integrationNote"))}</footer>
  </section>`;
}

/**
 * Side-effect-free mock controller; inject an element, never a native invoke.
 * Invalid/injected "real" snapshots fail closed to a fresh fixture.
 */
export function mountNativeInventoryWorkspace(root, snapshot = createInventoryFixture()) {
  if (!root || typeof root.addEventListener !== "function" || !("innerHTML" in root)) {
    throw new TypeError("A DOM mount element is required");
  }
  const data = validateInventoryFixture(snapshot) ? snapshot : createInventoryFixture();
  let state = createInventoryViewState();
  let disposed = false;
  function render() {
    if (!disposed) root.innerHTML = renderNativeInventoryWorkspace(data, state);
  }
  function onClick(event) {
    const target = event.target?.closest?.("[data-inv-tab], [data-inv-review], [data-inv-dismiss]");
    if (!target || disposed) return;
    if (target.dataset.invTab) state = reduceInventoryView(state, { type: "tab", value: target.dataset.invTab });
    if (Object.hasOwn(target.dataset, "invReview")) state = reduceInventoryView(state, { type: "review_scope" });
    if (Object.hasOwn(target.dataset, "invDismiss")) state = reduceInventoryView(state, { type: "dismiss_scope" });
    render();
  }
  function onChange(event) {
    const target = event.target;
    if (disposed || !target) return;
    if (Object.hasOwn(target.dataset ?? {}, "invLocale"))
      state = reduceInventoryView(state, { type: "locale", value: target.value });
    else if (Object.hasOwn(target.dataset ?? {}, "invApp"))
      state = reduceInventoryView(state, { type: "app", value: target.value });
    else if (Object.hasOwn(target.dataset ?? {}, "invSource"))
      state = reduceInventoryView(state, { type: "source", value: target.value });
    else return;
    render();
  }
  root.addEventListener("click", onClick);
  root.addEventListener("change", onChange);
  render();
  return Object.freeze({
    state: () => ({ ...state }),
    dispose() {
      disposed = true;
      root.removeEventListener?.("click", onClick);
      root.removeEventListener?.("change", onChange);
    },
  });
}
