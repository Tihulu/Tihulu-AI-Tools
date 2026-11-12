// Pure, dependency-free simulator. NO system processes, adapters, models or network are accessed.
export const SHUTDOWN = Object.freeze({
  STOP_AI: "stop-ai",
  APP_QUIT: "app-quit",
  HARD_STOP: "hard-stop",
  QUIT_ALL: "quit-all",
});

const demoResources = [
  { id: "advisor", name: "AI Advisor · demo session", scope: "ai", ownership: "ui", owner: "AI Tools UI", status: "active", ram: 410, vram: 256 },
  { id: "agent", name: "App Control Agent · demo session", scope: "ai", ownership: "ui", owner: "AI Tools UI", status: "active", ram: 390, vram: 256 },
  { id: "model", name: "Shared local model context · demo", scope: "ai", ownership: "ui", owner: "AI Tools UI", status: "active", ram: 650, vram: 768 },
  { id: "host-lease", name: "UI lease on paired GPU host · demo", scope: "ai", ownership: "ui", owner: "AI Tools UI", status: "active", ram: 32, vram: 0 },
  { id: "sdr", name: "Native RF/SDR capture handle · demo", scope: "device", ownership: "ui", owner: "AI Tools UI", status: "active", ram: 64, vram: 0 },
  { id: "host", name: "Independent TIHULU GPU host · demo", scope: "service", ownership: "independent", owner: "TIHULU independent host", status: "active", ram: 1024, vram: 2048, clients: 2 },
  { id: "jan", name: "Jan Desktop · external demo fixture", scope: "external", ownership: "borrowed", owner: "External app", status: "active", ram: null, vram: null },
  { id: "readsb", name: "readsb/tar1090 · external demo fixture", scope: "external", ownership: "borrowed", owner: "External app", status: "active", ram: null, vram: null },
];

export const ACTION_DETAILS = Object.freeze({
  [SHUTDOWN.STOP_AI]: { title: "Stop AI", description: "Release this UI's Advisor, Agent, model and remote request leases; leave its other devices and independent services alone." },
  [SHUTDOWN.APP_QUIT]: { title: "App Quit", description: "Cancel all this UI session's owned jobs, model contexts and RF handles. An independently enabled host remains for its other clients." },
  [SHUTDOWN.HARD_STOP]: { title: "Global Hard Stop", description: "Escalate cleanup of every resource owned by this UI session only. Do not terminate external or independently owned services." },
  [SHUTDOWN.QUIT_ALL]: { title: "Quit All TIHULU-Owned Services", description: "Also stop the independently managed TIHULU host, after warning affected authorized clients. Never stop Jan, Ollama or readsb." },
});

export function createDemoState({ independentHost = false } = {}) {
  return {
    resources: demoResources.map((r) => r.id === "host" ?
      { ...r, status: independentHost ? "active" : "released", clients: independentHost ? 2 : 0 } :
      r.id === "host-lease" ? { ...r, status: independentHost ? "active" : "released" } : { ...r }),
    lifecycle: "running",
    serviceMode: independentHost ? "independent" : "disabled",
    history: [],
    progress: [],
    activeTab: "overview",
    language: "en",
    profile: "auto",
    marketQuery: "",
    background: false,
    keepWarm: false,
    model: { advisor: "sample-local", agent: "sample-local" },
    appModel: {},
    appHost: {},
    compute: { advisor: "local", agent: "local" },
    chat: { advisor: [], agent: [] },
    janStep: -1,
    janStatus: "not_configured",
  };
}

export function getResource(state, id) {
  return state.resources.find((resource) => resource.id === id);
}

export function getTotals(state) {
  const owned = state.resources.filter((r) => r.ownership === "ui" && r.status === "active");
  return {
    activeOwned: owned.length,
    mockRamMiB: owned.reduce((sum, r) => sum + (r.ram || 0), 0),
    mockVramMiB: owned.reduce((sum, r) => sum + (r.vram || 0), 0),
  };
}

function isActionTarget(resource, action) {
  if (resource.status !== "active") return false;
  if (action === SHUTDOWN.QUIT_ALL) return resource.ownership === "ui" || resource.ownership === "independent";
  if (resource.ownership !== "ui") return false;
  if (action === SHUTDOWN.STOP_AI) return resource.scope === "ai";
  return action === SHUTDOWN.APP_QUIT || action === SHUTDOWN.HARD_STOP;
}

export function previewShutdown(state, action) {
  if (!ACTION_DETAILS[action]) throw new Error("Unknown shutdown action");
  const affected = state.resources.filter((r) => isActionTarget(r, action));
  const host = getResource(state, "host");
  const independentHostKept = action !== SHUTDOWN.QUIT_ALL && host.status === "active";
  const warnings = [];
  if (action === SHUTDOWN.QUIT_ALL && affected.some((r) => r.id === "host")) {
    warnings.push("Two simulated authorized host clients would lose access. Real clients must be notified and drained first.");
  }
  if (independentHostKept && action !== SHUTDOWN.STOP_AI) {
    warnings.push("Independent GPU host stays active for other authorized clients; its RAM/VRAM is not UI-owned.");
  }
  warnings.push("DEMO ONLY: no process, model, host, receiver or device is actually stopped.");
  return {
    ...ACTION_DETAILS[action],
    affected: affected.map((r) => r.id),
    preserved: state.resources.filter((r) => r.status === "active" && !isActionTarget(r, action)).map((r) => r.id),
    warnings,
  };
}

export function applyShutdown(state, action) {
  const preview = previewShutdown(state, action);
  const resources = state.resources.map((r) => {
    if (!preview.affected.includes(r.id)) return { ...r };
    return { ...r, status: "released", ...(r.id === "host" ? { clients: 0 } : {}) };
  });
  return {
    ...state,
    resources,
    serviceMode: action === SHUTDOWN.QUIT_ALL ? "disabled" : state.serviceMode,
    lifecycle: action === SHUTDOWN.APP_QUIT || action === SHUTDOWN.HARD_STOP || action === SHUTDOWN.QUIT_ALL ? "stopped" : state.lifecycle,
    progress: ["Reject new demo work", "Cancel owned demo requests", "Release simulated contexts / handles", "Verify mock ownership", "Simulation complete — physical state unverified"],
    history: [{ action, affected: preview.affected, preserved: preview.preserved, demo: true }, ...state.history].slice(0, 12),
  };
}

export function closeChat(state, chatId) {
  if (chatId !== "advisor" && chatId !== "agent") throw new Error("Unknown chat");
  const resources = state.resources.map((r) => r.id === chatId ? { ...r, status: "released" } : { ...r });
  if (!resources.some((r) => (r.id === "advisor" || r.id === "agent") && r.status === "active")) {
    const model = resources.find((r) => r.id === "model");
    if (model) model.status = "released";
  }
  return { ...state, resources, history: [{ action: "close-" + chatId, affected: [chatId], demo: true }, ...state.history] };
}

export function sendMockMessage(state, chatId, message) {
  if (chatId !== "advisor" && chatId !== "agent") throw new Error("Unknown chat");
  if (state.lifecycle !== "running" || getResource(state, chatId).status !== "active") return state;
  const value = String(message).trim().slice(0, 500);
  if (!value) return state;
  return {
    ...state,
    chat: {
      ...state.chat,
      [chatId]: [
        ...state.chat[chatId],
        { role: "user", text: value },
        { role: "system", text: "Demo only. No model, tool or inference provider is connected; no request was sent." },
      ],
    },
  };
}
