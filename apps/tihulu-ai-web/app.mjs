import { SHUTDOWN, createDemoState, getResource, getTotals } from "./state.mjs";
import { createMockUiAdapter, uiRequest, UI_COMMANDS, SERVICE_MODES, MODEL_FIXTURES } from "./ui-contract.mjs";
import { translate } from "./i18n.mjs";

// No fetch, WebUSB, Tauri invoke, localStorage, process calls or backend adapters.
let state = createDemoState();
const uiAdapter = createMockUiAdapter({ readState: () => state, writeState: (next) => { state = next; } });
const typedRead = (kind, payload = {}) => uiAdapter.request(uiRequest(kind, payload)).data;
let pendingAction = null;
const root = document.querySelector("#app");
const modal = document.querySelector("#confirmation");
const modalBody = document.querySelector("#dialog-body");
const t = (key) => translate(state.language, key);
const esc = (value) => String(value ?? "").replace(/[&<>"']/g, (ch) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[ch]);

const tabs = [
  ["overview", "◫"], ["market", "▦"], ["models", "▤"], ["apps", "⬡"],
  ["devices", "⌘"], ["host", "◇"], ["advisor", "✦"], ["agent", "✧"],
  ["jan", "⟁"], ["settings", "⚙"],
];
const applications = ["Media Center", "Text Center", "Satellite Center", "Flight Radar", "RF Geolocation", "Survival Kit"];
const profiles = [
  ["auto", "autodetect"], ["regular", "regular"], ["performance", "performance"],
  ["pi", "pi"], ["hailo", "hailo"], ["custom", "custom"],
];
const models = MODEL_FIXTURES.map((model) => ({
  id: model.model_id, name: model.name, task: model.modality,
  format: model.format.toUpperCase(), owner: model.owner_type,
  state: model.availability === "external" ? "external" : "unverified",
}));

function chip(label, tone = "neutral") { return '<span class="chip ' + tone + '">' + esc(label) + "</span>"; }
function panel(title, subtitle, body) {
  return '<section class="panel"><div class="head-row"><div><h2>' + esc(title) + '</h2><p class="hint">' + esc(subtitle) + "</p></div></div>" + body + "</section>";
}
function fieldSelect(key, options, selected, attr) {
  return '<label class="field">' + esc(t(key)) + '<select ' + attr + '>' +
    options.map(([value, label]) => '<option value="' + esc(value) + '"' + (selected === value ? " selected" : "") + ">" + esc(label) + "</option>").join("") +
    "</select></label>";
}
function pageHeading(title, description) {
  return '<div class="page-heading"><div class="eyebrow">TIHULU AI Tools / ' + esc(t("future")) + '</div><h1>' + esc(title) + "</h1><p>" + esc(description) + "</p></div>";
}
function resourceTable(resources) {
  return '<div class="table-scroll"><table><thead><tr><th>' + t("resource") + '</th><th>' + t("owner") + '</th><th>' + t("status") + '</th><th>RAM / VRAM</th></tr></thead><tbody>' +
    resources.map((r) => '<tr><td><div class="resource-name">' + esc(r.name) + '</div><div class="resource-meta">' + esc(r.scope) + ' · ' + esc(r.ownership) +
      '</div></td><td>' + esc(r.owner) + '</td><td><span class="resource-status ' + (r.status === "active" ? "" : "off") + '">' + t(r.status === "active" ? "active" : "released") +
      '</span></td><td>' + (r.ram == null ? "—" : r.ram + " MiB") + ' / ' + (r.vram == null ? "—" : r.vram + " MiB") + "</td></tr>").join("") +
    "</tbody></table></div>";
}
function shutdownControls() {
  const specs = [
    [SHUTDOWN.STOP_AI, "stopAi", "secondary"],
    [SHUTDOWN.APP_QUIT, "appQuit", "secondary"],
    [SHUTDOWN.HARD_STOP, "hardStop", "danger"],
    [SHUTDOWN.QUIT_ALL, "quitAll", "danger"],
  ];
  return '<div class="btn-grid">' + specs.map(([action, text, tone]) =>
    '<button class="btn ' + tone + '" type="button" data-shutdown="' + action + '">' + esc(t(text)) + "</button>"
  ).join("") + '</div><p class="hint">' + esc(t("shutdownNote")) + "</p>";
}
function overview() {
  const totals = typedRead(UI_COMMANDS.OVERVIEW).synthetic_totals;
  const stats = [
    [t("uiState"), t(state.lifecycle === "running" ? "running" : "stopped"), t("source")],
    [t("modelsActive"), String(totals.activeOwned), t("source")],
    [t("ram"), totals.mockRamMiB + " MiB", t("resourceSubtitle")],
    [t("vram"), totals.mockVramMiB + " MiB", t("resourceSubtitle")],
  ];
  return pageHeading(t("welcome"), t("intro")) +
    '<div class="cards">' + stats.slice(0, 3).map(([label, value, hint]) =>
      '<section class="card"><div class="micro">' + esc(label) + '</div><div class="value">' + esc(value) + '</div><div class="muted">' + esc(hint) + "</div></section>"
    ).join("") + '</div><div class="two-col"><div>' +
    panel(t("resourceTitle"), t("resourceSubtitle"), resourceTable(state.resources)) +
    '</div><div class="stack">' +
    panel(t("shutdown"), t("warning"), shutdownControls()) +
    panel(t("systemPolicy"), t("noBackground"),
      '<div class="stack">' + chip(t("onDemand")) + chip(t("backgroundOff")) +
      '<div class="hint">' + esc(t("vram")) + ': ' + esc(stats[3][1]) + " — " + esc(t("source")) + "</div></div>") +
    panel(t("log"), t("complete"),
      state.progress.length ? state.progress.map((step) => '<div class="mini-log">' + esc(step) + '</div>').join("") :
      '<p class="hint">' + t("noLog") + '</p>') +
    '<button type="button" class="btn ghost" data-reset>' + t("reset") + "</button></div></div>";
}
function market() {
  const available = typedRead(UI_COMMANDS.MODELS).models;
  const matching = available.map((model) => ({ id: model.model_id, name: model.name, task: model.modality, format: model.format.toUpperCase(), owner: model.owner_type, state: model.availability === "external" ? "external" : "unverified" })).filter((m) => (m.name + " " + m.task + " " + m.format).toLowerCase().includes(state.marketQuery.toLowerCase()));
  return pageHeading(t("browse"), t("marketHint")) +
    panel(t("market"), t("modelNames"), '<label class="field">' + t("search") +
      '<input id="market-query" type="search" maxlength="80" value="' + esc(state.marketQuery) + '" placeholder="' + esc(t("search")) +
      '"></label><div class="market-grid">' +
      (matching.length ? matching.map((m) =>
        '<article class="market-card">' + chip(t(m.state), m.state === "external" ? "neutral" : "warning") +
        '<h3>' + esc(m.name) + '</h3><p>' + esc(m.owner) + '</p><div class="meta-grid"><span>' + esc(m.task) + '</span><span>' + esc(m.format) +
        '</span><span>' + t("connectionNotReady") + '</span></div></article>'
      ).join("") : '<p class="hint">No matching demo candidates.</p>') + "</div>");
}
function myModels() {
  return pageHeading(t("models"), t("modelNames")) +
    panel(t("models"), t("marketHint"), resourceTable(state.resources.filter((r) => r.scope === "ai" || r.id === "jan")) +
      '<p class="hint">' + esc(t("connectionNotReady")) + "</p>");
}
function apps() {
  return pageHeading(t("applications"), t("appsHint")) +
    panel(t("apps"), t("none"),
      '<div class="table-scroll"><table><thead><tr><th>' + t("appName") + '</th><th>' + t("selection") + '</th><th>' + t("selectedHost") + '</th><th>' + t("permission") + "</th></tr></thead><tbody>" +
      applications.map((name, i) => '<tr><td><strong>' + esc(name) + '</strong><div class="resource-meta">' + t("connectionNotReady") + "</div></td><td>" +
        '<select aria-label="' + esc(name + " model") + '" data-app-model="' + i + '"><option value="none">None / demo</option>' +
        models.map((m) => '<option value="' + esc(m.id) + '"' + (state.appModel[i] === m.id ? ' selected' : '') + '>' + esc(m.name) + '</option>').join("") + '</select></td><td>' +
        '<select aria-label="' + esc(name + " host") + '" data-app-host="' + i + '"><option value="local"' + (state.appHost[i] !== "paired" ? ' selected' : '') + '>' + t("local") +
        '</option><option value="paired"' + (state.appHost[i] === "paired" ? ' selected' : '') + '>' + t("paired") + '</option></select></td><td>' +
        '<button type="button" class="btn secondary compact" data-explain="scope">' + t("review") + "</button></td></tr>"
      ).join("") + "</tbody></table></div>") +
    panel(t("permission"), t("readOnly"), '<p>' + esc(t("approve")) + " " + esc(t("noShell")) + "</p>");
}
function devices() {
  return pageHeading(t("deviceHeading"), t("deviceHint")) +
    '<div class="two-col"><div>' +
    panel(t("hardware"), t("future"), fieldSelect("profile", profiles.map(([value, key]) => [value, t(key)]), state.profile, 'id="hardware-profile"') +
      '<p class="hint">' + t("connectionNotReady") + "</p>") +
    '</div><div>' +
    panel(t("resourceTitle"), t("source"), resourceTable(state.resources.filter((r) => r.scope === "device" || r.scope === "external"))) +
    "</div></div>";
}
function gpuHost() {
  const host = typedRead(UI_COMMANDS.HOST);
  return pageHeading(t("hostHeading"), t("hostHint")) +
    '<div class="two-col"><div>' +
    panel(t("hostIndependent"), t("connectionNotReady"),
      '<div class="cards"><div class="card"><div class="micro">' + t("status") +
      '</div><div class="value">' + t(host.service_phase === "active" ? "active" : "released") +
      '</div></div><div class="card"><div class="micro">' + t("hostClients") + '</div><div class="value">' +
      Number(host.active_other_clients || 0) + '</div></div></div><button type="button" class="btn secondary" data-explain="pairing">' +
      t("pairing") + "</button>" +
      '<div class="actions"><span class="chip neutral">' + esc(t("serviceMode")) + ": " + esc(t(host.service_mode === SERVICE_MODES.INDEPENDENT ? "serviceIndependent" : "serviceDisabled")) + '</span></div>' +
      '<button type="button" class="btn ghost" data-service-mode="' + (host.service_mode === SERVICE_MODES.INDEPENDENT ? "disabled" : "independent") + '">' + t(host.service_mode === SERVICE_MODES.INDEPENDENT ? "disableService" : "enableService") + '</button>' +
      '<p class="hint">' + esc(t("serviceModeDemo")) + '</p>') + '</div><div>' +
    panel(t("permission"), t("readOnly"), '<p>' + t("approve") + '</p>' + chip(t("unsupported"), "warning")) +
    "</div></div>";
}
function chat(type) {
  const isAdvisor = type === "advisor";
  const title = t(isAdvisor ? "advisorHeading" : "agentHeading");
  const transcript = typedRead(UI_COMMANDS.CHAT, { agent: type }).messages;
  const enabled = state.lifecycle === "running" && getResource(state, type).status === "active";
  return pageHeading(title, t("chatInfo")) +
    '<div class="two-col"><div>' +
    panel(t(type), t("privacy"), '<div class="chat-window" aria-live="polite" aria-label="' + esc(t(type)) + ' transcript">' +
      (transcript.length ? transcript.map((m) => '<div class="bubble ' + esc(m.role) + '"><strong>' + esc(m.role) +
        '</strong><div>' + esc(m.text) + '</div></div>').join("") :
        '<p class="hint">' + t("chatInfo") + '</p>') +
      '</div><form class="chat-composer" data-chat="' + type + '"><input name="message" maxlength="500" required autocomplete="off" placeholder="' +
      esc(t("chatPlaceholder")) + '" aria-label="' + esc(t("chatPlaceholder")) + '"' + (enabled ? "" : " disabled") +
      '><button class="btn" type="submit"' + (enabled ? "" : " disabled") + '>' + t("send") + '</button></form>' +
      '<button class="btn ghost" type="button" data-close-chat="' + type + '"' + (enabled ? "" : " disabled") + '>' +
      t("closeChat") + '</button>') + '</div><div>' +
    panel(t("selection"), t("connectionNotReady"),
      fieldSelect("selection", models.map((m) => [m.id, m.name]), state.model[type], 'data-model="' + type + '"') +
      '<div></div>' +
      fieldSelect("selectedHost", [["local", t("local")], ["paired", t("paired")]], state.compute[type], 'data-compute="' + type + '"')) +
    panel(t("permission"), t("readOnly"),
      '<p>' + t("approve") + '</p><p class="hint">' + t("noShell") + '</p>' + chip(t("future"), "warning")) +
    "</div></div>";
}
function jan() {
  const stages = ["janStage1", "janStage2", "janStage3", "janStage4"];
  const janStatus = typedRead(UI_COMMANDS.JAN);
  return pageHeading(t("janHeading"), t("janHint")) +
    '<div class="two-col"><div>' +
    panel(t("howJan"), t("connectionNotReady"),
      '<ol class="steps">' + [1, 2, 3, 4].map((i) => '<li>' + esc(t("janGuide" + i)) + '</li>').join("") + '</ol>') +
    '</div><div>' +
    panel(t("setupJan"), t("janHint"), chip(janStatus.verified_handshake ? t("connected") : t("disconnected"), "warning") + '<p class="hint">MCP / STDIO: ' +
      t("connectionNotReady") + '</p>' +
      (state.janStep < 0 ? '<button type="button" class="btn" data-jan-start>' + t("setupJan") + '</button>' :
      '<div><h3>' + t("wizard") + ' · ' + (state.janStep + 1) + '/4</h3><p>' + esc(t(stages[state.janStep])) + '</p><div class="actions">' +
      '<button type="button" class="btn secondary" data-jan-prev>' + t("prev") + '</button>' +
      '<button type="button" class="btn" data-jan-next>' + t(state.janStep === 3 ? "finish" : "next") +
      '</button></div></div>')) +
    panel(t("permission"), t("readOnly"), '<p>' + t("approve") + '</p>' + chip(t("noShell"), "neutral")) +
    "</div></div>";
}
function settings() {
  return pageHeading(t("settingsHeading"), t("settingsHint")) +
    '<div class="two-col"><div>' +
    panel(t("systemPolicy"), t("noBackground"),
      '<label class="checkbox"><input type="checkbox" id="background-flag"' + (state.background ? " checked" : "") +
      ">" + t("background") + '</label><label class="checkbox"><input type="checkbox" id="keep-warm-flag"' +
      (state.keepWarm ? " checked" : "") + ">" + t("keepWarm") + "</label>" +
      '<p class="hint">' + t("noBackground") + "</p>") +
    '</div><div>' +
    panel(t("language"), t("future"),
      fieldSelect("language", [["en", "English"], ["tr", "Türkçe"], ["de", "Deutsch"]], state.language, 'id="language-settings"')) +
    panel(t("docs"), t("source"), '<p class="hint">' + t("demoDetail") + "</p>") +
    "</div></div>";
}
function screen() {
  switch (state.activeTab) {
    case "market": return market();
    case "models": return myModels();
    case "apps": return apps();
    case "devices": return devices();
    case "host": return gpuHost();
    case "advisor": return chat("advisor");
    case "agent": return chat("agent");
    case "jan": return jan();
    case "settings": return settings();
    default: return overview();
  }
}
function render() {
  document.documentElement.lang = state.language;
  root.innerHTML =
    '<div class="app"><aside class="sidebar"><div class="brand"><div class="brand-mark" aria-hidden="true">T</div><div>' +
    '<div class="brand-title">TIHULU AI Tools</div><div class="micro">' + esc(t("subtitle")) + "</div></div></div>" +
    '<div><div class="micro">' + esc(t("navigation")) + '</div><nav class="nav" aria-label="' + esc(t("navigation")) + '">' +
    tabs.map(([id, icon]) => '<button type="button" class="nav-btn ' + (state.activeTab === id ? "active" : "") +
      '" data-go="' + id + '" ' + (state.activeTab === id ? 'aria-current="page"' : "") +
      '><span class="nav-icon" aria-hidden="true">' + icon + '</span>' + esc(t(id)) + "</button>").join("") +
    '</nav></div><div class="sidebar-foot"><span class="dot"></span>' + esc(t("onDemand")) + " · " +
    esc(t("backgroundOff")) + '<div class="micro">' + esc(t("connectionNotReady")) + "</div></div></aside>" +
    '<main class="content"><header class="topbar"><div class="top-actions">' + chip(t("demo"), "warning") +
    chip(t("connection") + ": " + t("disconnected")) + '</div><div class="top-actions">' +
    '<label class="micro" for="language-top">' + esc(t("language")) + '</label><select id="language-top" aria-label="' + esc(t("language")) +
    '"><option value="en"' + (state.language === "en" ? " selected" : "") + '>EN</option><option value="tr"' +
    (state.language === "tr" ? " selected" : "") + '>TR</option><option value="de"' +
    (state.language === "de" ? " selected" : "") + '>DE</option></select></div></header>' +
    '<div class="demo-banner"><strong>' + esc(t("demo")) + '</strong> — ' + esc(t("demoDetail")) + "</div>" +
    screen() + '<footer class="footer">TIHULU AI Tools · Rust-first/Tauri 2 UI concept · EN / TR / DE · ' +
    esc(t("source")) + "</footer></main></div>";
}
function showModal(title, detail, warnings, onConfirm = null) {
  pendingAction = onConfirm;
  modalBody.innerHTML = '<h2 id="dialog-title">' + esc(title) + '</h2><p id="dialog-detail">' + esc(detail) + '</p>' +
    '<ul>' + warnings.map((warning) => '<li>' + esc(warning) + '</li>').join("") + "</ul>";
  document.querySelector("#dialog-confirm").hidden = !onConfirm;
  document.querySelector("#dialog-confirm").textContent = t("confirm");
  document.querySelector("#dialog-cancel").textContent = t("cancel");
  modal.showModal();
}
root.addEventListener("click", (event) => {
  const button = event.target.closest("button");
  if (!button) return;
  if (button.dataset.go) { state = { ...state, activeTab: button.dataset.go }; render(); return; }
  if (button.dataset.reset !== undefined) { state = createDemoState(); render(); return; }
  if (button.dataset.shutdown) {
    const action = button.dataset.shutdown;
    const preview = typedRead(UI_COMMANDS.PREVIEW_STOP, { action });
    const affected = preview.affected.map((id) => getResource(state, id).name);
    showModal(preview.title + " — " + t("detailPanel"),
      preview.description + (affected.length ? "" : " " + t("nothingActive")),
      [t("affected") + ": " + (affected.join(", ") || "None"), t("preserved") + ": " +
        (preview.preserved.map((id) => getResource(state, id).name).join(", ") || "None"), ...preview.warnings],
      () => { uiAdapter.request(uiRequest(UI_COMMANDS.EXECUTE_STOP, { action, confirmed: true })); state.activeTab = "overview"; render(); });
    return;
  }
  if (button.dataset.serviceMode) {
    const mode = button.dataset.serviceMode;
    const shuttingDown = mode === SERVICE_MODES.DISABLED;
    const host = typedRead(UI_COMMANDS.HOST);
    showModal(t("serviceMode"), t("serviceModeDemo"),
      [t("demoDetail"), shuttingDown ? t("serviceClientWarning") + " (" + host.active_other_clients + ")" : t("serviceOptInWarning"),
       t("borrowedPreserved")],
      () => { uiAdapter.request(uiRequest(UI_COMMANDS.SERVICE_MODE, { mode, confirmed: true })); render(); });
    return;
  }
  if (button.dataset.closeChat) {
    const id = button.dataset.closeChat;
    showModal(t("closeChat"), t("chatInfo"), [t("warning"), "DEMO ONLY: no actual model or client is released."],
      () => { uiAdapter.closeChat(id); render(); });
    return;
  }
  if (button.dataset.explain) {
    showModal(t("permission"), t("connectionNotReady"),
      [t("readOnly"), t("approve"), t("noShell"), t("demoDetail")]);
    return;
  }
  if (button.dataset.janStart !== undefined) { state = { ...state, janStep: 0 }; render(); return; }
  if (button.dataset.janPrev !== undefined) { state = { ...state, janStep: Math.max(0, state.janStep - 1) }; render(); return; }
  if (button.dataset.janNext !== undefined) {
    state = { ...state, janStep: state.janStep === 3 ? -1 : state.janStep + 1 };
    render();
  }
});
root.addEventListener("submit", (event) => {
  const form = event.target.closest("form[data-chat]");
  if (!form) return;
  event.preventDefault();
  uiAdapter.sendChat(form.dataset.chat, new FormData(form).get("message"));
  render();
});
root.addEventListener("change", (event) => {
  const el = event.target;
  if (el.id === "language-top" || el.id === "language-settings") state = { ...state, language: el.value };
  else if (el.id === "hardware-profile") state = { ...state, profile: el.value };
  else if (el.id === "background-flag") state = { ...state, background: el.checked };
  else if (el.id === "keep-warm-flag") state = { ...state, keepWarm: el.checked };
  else if (el.id === "market-query") state = { ...state, marketQuery: el.value.slice(0, 80) };
  else if (el.dataset.model) state = { ...state, model: { ...state.model, [el.dataset.model]: el.value } };
  else if (el.dataset.compute) state = { ...state, compute: { ...state.compute, [el.dataset.compute]: el.value } };
  else if (el.dataset.appModel !== undefined) state = { ...state, appModel: { ...state.appModel, [el.dataset.appModel]: el.value } };
  else if (el.dataset.appHost !== undefined) state = { ...state, appHost: { ...state.appHost, [el.dataset.appHost]: el.value } };
  // Mock app selectors persist only in this browser's volatile simulator.
  render();
});
document.querySelector("#dialog-cancel").addEventListener("click", () => { pendingAction = null; modal.close(); });
document.querySelector("#dialog-confirm").addEventListener("click", () => {
  const action = pendingAction; pendingAction = null; modal.close();
  if (action) action();
});
modal.addEventListener("close", () => { pendingAction = null; });
render();
