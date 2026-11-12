/**
 * TIHULU UI ⇄ Rust boundary proposal, schema v1.
 * This module is a MOCK TRANSPORT ONLY: no Tauri invoke(), fetch(), WebUSB or filesystem calls.
 * Rust is the eventual authority for owner/lease and backend verification.
 *
 * @typedef {"demo_fixture"|"backend_verified"} EvidenceSource
 * @typedef {"disabled"|"independent"} ServiceMode
 * @typedef {"running"|"closed"} GuiPhase
 * @typedef {"unknown"|"candidate"|"installed"|"external"} ModelAvailability
 * @typedef {"unverified"|"incompatible"|"verified"} CompatibilityVerdict
 * @typedef {"read_overview"|"read_models"|"read_compatibility"|"read_gpu_host"|"read_chat"|"read_jan"|"set_jan_step"|"send_chat"|"close_chat"|"preview_stop"|"execute_stop"|"set_service_mode"} UiRequestKind
 *
 * @typedef {{schema_version:1, source:EvidenceSource, request_id:string, kind:string, data:object}} UiResponse
 * @typedef {{schema_version:1, request_id:string, kind:UiRequestKind, payload:object}} UiRequest
 */
import { SHUTDOWN, applyShutdown, closeChat, getResource, getTotals, previewShutdown, sendMockMessage } from "./state.mjs";

export const UI_SCHEMA_VERSION = 1;
export const EVIDENCE = Object.freeze({ DEMO: "demo_fixture", VERIFIED: "backend_verified" });
export const UI_COMMANDS = Object.freeze({
  OVERVIEW: "read_overview", MODELS: "read_models", COMPATIBILITY: "read_compatibility",
  HOST: "read_gpu_host", CHAT: "read_chat", JAN: "read_jan", JAN_STEP: "set_jan_step",
  SEND_CHAT: "send_chat", CLOSE_CHAT: "close_chat",
  PREVIEW_STOP: "preview_stop", EXECUTE_STOP: "execute_stop", SERVICE_MODE: "set_service_mode",
});
export const SERVICE_MODES = Object.freeze({ DISABLED: "disabled", INDEPENDENT: "independent" });
export const MODEL_FIXTURES = Object.freeze([
  { model_id: "demo:ggml-speech", name: "Speech candidate · demo", format: "ggml", modality: "speech", availability: "candidate", owner_type: "mock", selected_runtime: null },
  { model_id: "demo:external-gguf", name: "External GGUF candidate · demo", format: "gguf", modality: "text", availability: "external", owner_type: "borrowed_external", selected_runtime: null },
  { model_id: "demo:hailo-hef", name: "Hailo artifact candidate · demo", format: "hef", modality: "vision", availability: "candidate", owner_type: "mock", selected_runtime: null },
  { model_id: "demo:onnx", name: "ONNX candidate · demo", format: "onnx", modality: "vision", availability: "candidate", owner_type: "mock", selected_runtime: null },
]);

const requestKinds = new Set(Object.values(UI_COMMANDS));
const stopActions = new Set(Object.values(SHUTDOWN));
const opaqueIdentity = (id) => typeof id === "string" && id.length <= 128 &&
  /^[A-Za-z0-9][A-Za-z0-9._:-]*$/.test(id) && !id.includes("..");
const exactKeys = (payload, keys) => Object.keys(payload).length === keys.length &&
  keys.every((key) => Object.hasOwn(payload, key));
// JavaScript must match Rust's byte-based 500-byte chat limit.
const utf8Bytes = (value) => unescape(encodeURIComponent(value)).length;

/** @param {unknown} value @returns {value is UiRequest} */
export function isUiRequest(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return false;
  const v = /** @type {Record<string, unknown>} */ (value);
  if (v.schema_version !== 1 || !exactKeys(v, ["schema_version", "request_id", "kind", "payload"]) || typeof v.request_id !== "string" ||
      !/^[A-Za-z0-9:_-]{1,64}$/.test(v.request_id) ||
      !requestKinds.has(v.kind) || !v.payload || typeof v.payload !== "object" || Array.isArray(v.payload)) return false;
  const p = /** @type {Record<string, unknown>} */ (v.payload);
  switch (v.kind) {
    case UI_COMMANDS.COMPATIBILITY: return exactKeys(p, ["model_id", "task_id", "host_id"]) && [p.model_id, p.task_id, p.host_id].every(opaqueIdentity);
    case UI_COMMANDS.CHAT:
    case UI_COMMANDS.CLOSE_CHAT: return exactKeys(p, ["agent"]) && (p.agent === "advisor" || p.agent === "agent");
    case UI_COMMANDS.SEND_CHAT: return exactKeys(p, ["agent", "message"]) && (p.agent === "advisor" || p.agent === "agent") && typeof p.message === "string" && p.message.trim().length > 0 && utf8Bytes(p.message) <= 500;
    case UI_COMMANDS.JAN_STEP: return exactKeys(p, ["step"]) && Number.isInteger(p.step) && p.step >= -1 && p.step <= 3;
    case UI_COMMANDS.PREVIEW_STOP: return exactKeys(p, ["action"]) && stopActions.has(p.action);
    case UI_COMMANDS.EXECUTE_STOP: return exactKeys(p, ["action", "confirmed"]) && stopActions.has(p.action) && p.confirmed === true;
    case UI_COMMANDS.SERVICE_MODE: return exactKeys(p, ["mode", "confirmed"]) && (p.mode === SERVICE_MODES.DISABLED || p.mode === SERVICE_MODES.INDEPENDENT) && p.confirmed === true;
    default: return Object.keys(p).length === 0;
  }
}

/** @param {UiRequestKind} kind @param {object} payload @param {string} requestId */
export function uiRequest(kind, payload = {}, requestId = "ui-demo-01") {
  const request = { schema_version: UI_SCHEMA_VERSION, request_id: requestId, kind, payload };
  if (!isUiRequest(request)) throw new TypeError("Invalid typed UI request");
  return request;
}

function respond(request, data) {
  return Object.freeze({
    schema_version: UI_SCHEMA_VERSION,
    source: EVIDENCE.DEMO,
    request_id: request.request_id,
    kind: request.kind,
    data,
  });
}

/**
 * Typed mocked adapter. All values are synthetic, even when resource state says active/released.
 * The Rust authoritative ledger must eventually supply snapshots and acknowledge actual cleanup.
 *
 * @param {{readState:()=>any, writeState:(state:any)=>void}} bindings
 */
export function createMockUiAdapter({ readState, writeState }) {
  if (typeof readState !== "function" || typeof writeState !== "function") throw new TypeError("State bindings required");
  return Object.freeze({
    request(request) {
      if (!isUiRequest(request)) throw new TypeError("Invalid typed UI request");
      const state = readState();
      const totals = getTotals(state);
      switch (request.kind) {
        case UI_COMMANDS.OVERVIEW:
          return respond(request, {
            gui: { session_id: "demo-gui-session", phase: state.lifecycle === "running" ? "running" : "closed" },
            policy: { execution: "on_demand", background_enabled: state.background, keep_warm: state.keepWarm },
            resources: state.resources.map(({ id, name, owner, ownership, scope, status, ram, vram }) => ({
              resource_id: "demo:" + id, label: name, owner_id: owner, provenance: ownership,
              resource_kind: scope, mock_phase: status, estimated_ram_mib: ram, estimated_vram_mib: vram,
              backend_verified: false,
            })),
            synthetic_totals: totals,
          });
        case UI_COMMANDS.MODELS:
          return respond(request, { models: MODEL_FIXTURES.map((m) => ({ ...m })) });
        case UI_COMMANDS.COMPATIBILITY:
          return respond(request, {
            model_id: request.payload.model_id,
            task_id: request.payload.task_id,
            host_id: request.payload.host_id,
            verdict: "unverified", reasons: ["No approved runtime, driver or actual task probe has been executed."],
          });
        case UI_COMMANDS.HOST: {
          const host = getResource(state, "host");
          return respond(request, {
            host_id: "demo:independent-pc",
            service_mode: state.serviceMode,
            gui_phase: state.lifecycle === "running" ? "running" : "closed",
            service_phase: state.serviceMode === SERVICE_MODES.INDEPENDENT ? host.status : "stopped",
            active_other_clients: state.serviceMode === SERVICE_MODES.INDEPENDENT ? host.clients : 0,
            connection: "not_connected", scope: "independent_tihulu_host",
            backend_verified: false,
          });
        }
        case UI_COMMANDS.CHAT:
          return respond(request, {
            agent: request.payload.agent, messages: state.chat[request.payload.agent].map((m) => ({ ...m })),
            inference_connected: false, permission_mode: "read_only", available_tools: [],
          });
        case UI_COMMANDS.JAN:
          return respond(request, {
            connection: "not_configured", wizard_step: state.janStep,
            registered: false, verified_handshake: false, executable_available: false,
          });
        case UI_COMMANDS.JAN_STEP: {
          if (state.lifecycle !== "running") throw new Error("GUI session closed");
          writeState({ ...state, janStep: request.payload.step });
          return respond(request, { outcome: "simulated_only", wizard_step: request.payload.step, connection: "not_configured", backend_verified: false });
        }
        case UI_COMMANDS.SEND_CHAT: {
          if (state.lifecycle !== "running") throw new Error("GUI session closed");
          const next = sendMockMessage(state, request.payload.agent, request.payload.message);
          writeState(next);
          return respond(request, { outcome: "simulated_only", accepted: next !== state, inference_connected: false });
        }
        case UI_COMMANDS.CLOSE_CHAT: {
          if (state.lifecycle !== "running") throw new Error("GUI session closed");
          writeState(closeChat(state, request.payload.agent));
          return respond(request, { outcome: "simulated_only", backend_verified: false });
        }
        case UI_COMMANDS.PREVIEW_STOP:
          return respond(request, { ...previewShutdown(state, request.payload.action), backend_verified: false });
        case UI_COMMANDS.EXECUTE_STOP: {
          const next = applyShutdown(state, request.payload.action);
          writeState(next);
          return respond(request, {
            outcome: "simulated_only",
            gui_phase: next.lifecycle === "running" ? "running" : "closed",
            service_mode: next.serviceMode,
            simulated_stages: next.progress.slice(),
            backend_verified: false,
          });
        }
        case UI_COMMANDS.SERVICE_MODE: {
          if (state.lifecycle !== "running") throw new Error("GUI session closed");
          const enabled = request.payload.mode === SERVICE_MODES.INDEPENDENT;
          const next = {
            ...state,
            serviceMode: request.payload.mode,
            resources: state.resources.map((r) => r.id === "host" ?
              { ...r, status: enabled ? "active" : "released", clients: enabled ? 2 : 0 } : { ...r }),
          };
          writeState(next);
          return respond(request, {
            outcome: "simulated_only", service_mode: next.serviceMode,
            connected_clients: enabled ? 2 : 0, backend_verified: false,
          });
        }
        default:
          throw new TypeError("Unsupported mock UI request");
      }
    },
    /** Synthetic chat event; no remote or native provider is invoked. */
    sendChat(agent, message) {
      const response = this.request(uiRequest(UI_COMMANDS.SEND_CHAT, { agent, message }));
      return { ...response.data, source: response.source };
    },
    closeChat(agent) {
      const response = this.request(uiRequest(UI_COMMANDS.CLOSE_CHAT, { agent }));
      return { ...response.data, source: response.source };
    },
  });
}
