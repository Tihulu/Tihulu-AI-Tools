/**
 * PHASE5-UI: pure, fixture-only native inventory presentation contract.
 * NO filesystem enumeration, browser folder picker, native IPC or cleanup.
 * The Registry/Native owner must replace this contract after explicit approval.
 */
export const INVENTORY_SCHEMA = 1;
export const INVENTORY_SOURCE = "demo_fixture";
export const INVENTORY_BACKEND = "unavailable";

export const SOURCE_OPTIONS = Object.freeze([
  "custom_folder", "external_ssd", "jan", "ollama", "hugging_face",
]);

const examples = Object.freeze([
  { model_id: "demo:speech-ggml", name: "Example speech model", origin: "tihulu_app", format: "ggml", ownership: "app_managed", verification: "unverified", runtime: "not_checked", location: "demo://models/media/speech" },
  { model_id: "demo:jan-gguf", name: "Example Jan GGUF", origin: "jan", format: "gguf", ownership: "borrowed_external", verification: "unverified", runtime: "not_checked", location: "demo://external/jan/models" },
  { model_id: "demo:ollama-gguf", name: "Example Ollama model", origin: "ollama", format: "gguf", ownership: "borrowed_external", verification: "unverified", runtime: "not_checked", location: "demo://external/ollama/models" },
  { model_id: "demo:hf-safetensors", name: "Example HF weight", origin: "hugging_face", format: "safetensors", ownership: "borrowed_external", verification: "unverified", runtime: "not_checked", location: "demo://external/hf/cache" },
  { model_id: "demo:hailo-hef", name: "Example AI HAT+ artifact", origin: "user_folder", format: "hef", ownership: "user_managed", verification: "unverified", runtime: "not_checked", location: "demo://user/ssd/models" },
  { model_id: "demo:vision-onnx", name: "Example vision model", origin: "tihulu_app", format: "onnx", ownership: "app_managed", verification: "unverified", runtime: "not_checked", location: "demo://models/satellite/vision" },
]);

const appFixtures = Object.freeze([
  ["tihulu-media-center", "Media Center"],
  ["tihulu-text-center", "Text Center"],
  ["tihulu-satellite-center", "Satellite Center"],
  ["tihulu-flight-radar", "Flight Radar"],
  ["tihulu-rf-geolocation", "RF Geolocation"],
  ["tihulu-survival-kit", "Survival Kit"],
]);

/** All paths are deliberately NON-FILESYSTEM display aliases. */
export function createInventoryFixture() {
  return {
    schema_version: INVENTORY_SCHEMA,
    source: INVENTORY_SOURCE,
    backend: INVENTORY_BACKEND,
    native_scans_completed: 0,
    native_grants: 0,
    native_cleanup_verified: false,
    models: examples.map((m) => ({ ...m })),
    apps: appFixtures.map(([app_id, name]) => ({
      app_id, name, install_state: "not_checked",
      installation_path: `demo://install/${app_id}`,
      active_model_root: `demo://models/apps/${app_id}`,
      legacy_model_roots: [`demo://legacy/${app_id}/models`],
      model_path_source: "illustrative_only", path_verified: false,
    })),
    providers: ["jan", "ollama", "gpu_host"].map((provider_id) => ({
      provider_id, state: "not_checked", connection_verified: false,
      note: "No native provider status check has been performed.",
    })),
    resources: [
      { owner_kind: "gui_session", lease_count: null, cleanup_state: "not_executed", verified: false },
      { owner_kind: "independent_host", lease_count: null, cleanup_state: "not_executed", verified: false },
      { owner_kind: "borrowed_external", lease_count: null, cleanup_state: "not_applicable", verified: false },
    ],
  };
}

const mockPath = (value) => typeof value === "string" &&
  /^demo:\/\/[a-z0-9/_-]+$/.test(value) && !value.includes("..");
const validId = (value) => typeof value === "string" &&
  value.length <= 128 && /^[a-z0-9][a-z0-9._:-]*$/.test(value) && !value.includes("..");
const strictly = (obj, keys) => obj !== null && typeof obj === "object" &&
  !Array.isArray(obj) && Object.keys(obj).length === keys.length &&
  keys.every((key) => Object.hasOwn(obj, key));

/** Rejects any allegedly verified, scanned, or authorized discovery fixture. */
export function validateInventoryFixture(data) {
  if (!strictly(data, ["schema_version", "source", "backend", "native_scans_completed",
    "native_grants", "native_cleanup_verified", "models", "apps", "providers", "resources"])) return false;
  if (data.schema_version !== 1 || data.source !== INVENTORY_SOURCE ||
      data.backend !== INVENTORY_BACKEND || data.native_scans_completed !== 0 ||
      data.native_grants !== 0 || data.native_cleanup_verified !== false) return false;
  if (!Array.isArray(data.models) || !Array.isArray(data.apps) ||
      !Array.isArray(data.providers) || !Array.isArray(data.resources)) return false;
  if (data.models.length > 100 || data.apps.length > 30 ||
      data.providers.length !== 3 || data.resources.length !== 3) return false;
  const seenModels = new Set();
  for (const m of data.models) {
    if (!strictly(m, ["model_id","name","origin","format","ownership","verification","runtime","location"]) ||
        !validId(m.model_id) || seenModels.has(m.model_id) ||
        typeof m.name !== "string" || m.name.length > 100 ||
        !["tihulu_app","jan","ollama","hugging_face","user_folder"].includes(m.origin) ||
        !["ggml","gguf","safetensors","hef","onnx"].includes(m.format) ||
        !["app_managed","borrowed_external","user_managed"].includes(m.ownership) ||
        (["jan","ollama","hugging_face"].includes(m.origin) && m.ownership !== "borrowed_external") ||
        (m.origin === "tihulu_app" && m.ownership !== "app_managed") ||
        (m.origin === "user_folder" && m.ownership !== "user_managed") ||
        m.verification !== "unverified" || m.runtime !== "not_checked" ||
        !mockPath(m.location)) return false;
    seenModels.add(m.model_id);
  }
  const seenApps = new Set();
  for (const app of data.apps) {
    if (!strictly(app, ["app_id","name","install_state","installation_path","active_model_root",
      "legacy_model_roots","model_path_source","path_verified"]) ||
      !validId(app.app_id) || seenApps.has(app.app_id) ||
      typeof app.name !== "string" || app.name.length > 100 ||
      app.install_state !== "not_checked" || app.path_verified !== false ||
      app.model_path_source !== "illustrative_only" ||
      !mockPath(app.installation_path) || !mockPath(app.active_model_root) ||
      !Array.isArray(app.legacy_model_roots) || app.legacy_model_roots.length > 10 ||
      !app.legacy_model_roots.every(mockPath)) return false;
    seenApps.add(app.app_id);
  }
  const seenProviders = new Set();
  for (const p of data.providers) {
    if (!strictly(p, ["provider_id","state","connection_verified","note"]) ||
      !["jan","ollama","gpu_host"].includes(p.provider_id) || seenProviders.has(p.provider_id) ||
      p.state !== "not_checked" || p.connection_verified !== false ||
      typeof p.note !== "string" || p.note.length > 200) return false;
    seenProviders.add(p.provider_id);
  }
  if (seenProviders.size !== 3) return false;
  const seenResources = new Set();
  for (const r of data.resources) {
    if (!strictly(r, ["owner_kind","lease_count","cleanup_state","verified"]) ||
      !["gui_session","independent_host","borrowed_external"].includes(r.owner_kind) ||
      seenResources.has(r.owner_kind) || r.lease_count !== null || r.verified !== false ||
      r.cleanup_state !== (r.owner_kind === "borrowed_external" ? "not_applicable" : "not_executed")) return false;
    seenResources.add(r.owner_kind);
  }
  return seenResources.size === 3;
}

export function createInventoryViewState() {
  return {
    locale: "en", tab: "models", selected_app: "tihulu-media-center",
    source_kind: "custom_folder", consent_preview: "idle",
  };
}

const tabs = new Set(["models", "apps", "sources", "providers", "ownership"]);
const locales = new Set(["en", "tr", "de"]);
/**
 * Preview-only state reducer; NO grant IDs, OS paths, process controls,
 * folder dialog side effects or persistent configuration are produced.
 */
export function reduceInventoryView(state, action) {
  if (!action || typeof action !== "object") return state;
  switch (action.type) {
    case "locale": return locales.has(action.value) ? { ...state, locale: action.value } : state;
    case "tab": return tabs.has(action.value) ? { ...state, tab: action.value } : state;
    case "app": return typeof action.value === "string" && appFixtures.some(([id]) => id === action.value) ?
      { ...state, selected_app: action.value } : state;
    case "source": return SOURCE_OPTIONS.includes(action.value) ?
      { ...state, source_kind: action.value, consent_preview: "idle" } : state;
    case "review_scope": return { ...state, consent_preview: "review_only" };
    case "dismiss_scope": return { ...state, consent_preview: "idle" };
    default: return state;
  }
}
