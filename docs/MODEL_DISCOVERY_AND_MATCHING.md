# Universal Model Discovery & Application Matching — v0.6 design

> **Status:** Planning-only. Requirements confirmed by user on 2026-11-12; discovery, matching, automation and source-specific adapters remain **unimplemented proposals**. Never scan, move, modify or execute another application's files without user consent.

## 1. Scope

The **Model Market** is also a **cross-application model registry**. Given (a) an application name, (b) an application installation/data directory, (c) a model directory or file, or (d) a known inference server, TIHULU AI Tools should *ask permission*, discover available compatible model artifacts, and show them as **Already installed — externally managed** without downloading or relocating them.

A user can then select an AI task in any independently installed TIHULU application (or a separately authorized third-party app), view compatible existing models, read evidence-backed suggestions, and explicitly **assign** a model or serving endpoint. Nothing silently edits the foreign application.

### Core distinction

- **Model artifact:** e.g. GGUF, MLX bundle, CTranslate2 model, ONNX/OpenVINO export, Hailo HEF+required assets. File discovery does **not** prove a usable runtime.
- **Model owner:** Jan, Ollama, LM Studio, Hugging Face cache, TIHULU, another named application, or an explicit custom path.
- **Inference endpoint:** Jan/OpenAI-compatible endpoint, Ollama API, TIHULU GPU host, etc. A provider may expose model **IDs**, not accessible on-disk weights.
- **Consumer capability:** e.g. Media Center live ASR, Text Center offline translation, Survival Kit chat/RAG, Agent tool calling, Satellite image analysis. Must be backed by implemented application adapters, not descriptions alone.
- **Ready to use** requires verified file/endpoint, compatible runtime, actual target hardware and authorized consumer contract.

## 2. Explicit discovery wizard (planned)

1. **Choose source:** app name (from searchable catalog), known installed app, manually chosen folder, a model file, or a local/API endpoint.
2. **Preview scope:** show exactly which directories or endpoints will be inspected. Ask for read-only permission. Default to conservative directories and avoid recursive whole-disk scans.
3. **Detect:** identify active app data path (may be customized), engine, owned metadata, model files and optional server model list. Do not read chat histories, private documents, API keys, passwords or unrelated content.
4. **Inventory safely:** normalize format, file size, content hash when reasonable, source URL/license when known, architecture, context size, quantization, language/task tags and runtime hints. Treat metadata from external apps as untrusted.
5. **Deduplicate by provenance + digest**, not filename. Preserve distinct quantizations and formats.
6. **Register an external reference**, not a TIHULU download. Clearly show “Stored in Jan (external)”, “Known local artifact”, “Server-served only” and “Available for mapping”.
7. **Run compatibility checks** (artifact format × runtime × CPU/GPU/NPU × available memory × task modality × app adapter); optional opt-in test inference.
8. **Offer matching** into eligible TIHULU apps and approved third-party apps. Generate a diff/config preview, check scope, request confirmation, atomically persist mapping with backup and rollback.

### Source adapters

| Source | Candidate discovery approach | Constraints |
| --- | --- | --- |
| **Jan Desktop** | OS Jan data folder or user-defined data folder; inspect `llamacpp/models` and `mlx/models` and model metadata; optionally connect to Jan Local API | Read-only unless separate user-approved import/link operation; desktop and standalone Jan Agent are separate products |
| **Ollama** | Local API model listing; filesystem detection only with explicit grant | Store ownership belongs to Ollama; its blob layout is not a generic GGUF directory |
| **LM Studio / other desktop AI** | Documented server API or selected model path | Do not assume layouts or access privileges |
| **Hugging Face cache** | Standard user-scoped cache paths or explicitly selected HF_HOME | Snapshot/links can share blobs; preserve cache ownership and snapshot layout |
| **whisper.cpp / Faster-Whisper / Argos** | File path, package/runtime discovery, current TIHULU application mapping | Separate ASR and translation engines; one file does not run in every backend |
| **Hailo AI HAT+ 2** | Supported Hailo runtime and required model assets | ARM64/Hailo compilation alone is insufficient; HEF cannot be executed by NVIDIA CUDA without a separate compatible export |
| **Custom application / directory** | File chooser plus owner-assigned label and optional adapter | Require a defensible type detector; unknown files remain “unverified” |

Paths must be normalized, user-approved and safeguarded against symlink escapes, network mount surprises and traversal beyond declared scope. Avoid automatically opening arbitrary models as executable code.

## 3. Model registry contract — example fields (draft, not a deployed schema)

```json
{
  "schema_version": "0.6-draft",
  "model_id": "sha256:example-placeholder",
  "display_name": "Example Jan GGUF",
  "ownership": "external",
  "source_application": "jan-desktop",
  "storage_mode": "reference-in-place",
  "source_path": "<user-approved Jan data path>/llamacpp/models/.../model.gguf",
  "model_format": "GGUF",
  "task_capabilities": ["text-generation", "chat"],
  "inference_endpoints": [],
  "runtime_compatibility": ["llama.cpp:pending-validation"],
  "known_installed": true,
  "runtime_verified": false,
  "referenced_by": [],
  "integrity": "unverified",
  "license_status": "unknown"
}
```

No real paths or secrets are stored in a public repo. Sensitive local paths belong only in a user-owned private registry. API credentials must be stored separately in OS-protected storage.

## 4. Matching engine

Match **the task, not only the model name**:

- **Hard constraints:** task type (ASR / translation / LLM / VLM / embedding / OCR / classification), input/output modality, accepted client API, installed runtime, binary/endpoint compatibility, resource limits, trust/permissions.
- **Model suitability:** supported languages (EN/TR/DE for relevant apps), context length, quantization, model/tool-calling support, expected latency, accuracy metrics when meaningful, disk/VRAM budget.
- **User preference:** local-only, paired PC GPU, offline requirement, lower memory, highest quality, “already installed first”, custom prioritization.
- **Validation:** static adapter checks + read-only readiness + **optional test inference** under user consent. Model marketing claims do not count as a successful test.
- **Evidence:** show source of any fact (registry, official model card, versioned documentation, reproducible benchmark); separate unknowns from verified data.

### Example tasks

| App | Example task | Candidate only when |
| --- | --- | --- |
| Media Center | Live TV/radio transcription | ASR model + genuine audio pipeline adapter + tested runtime |
| Media / Text Center | Offline translation | Proven EN/TR/DE directional translation engine/model |
| Survival Kit | Local assistant and offline RAG | Chat model and separately compatible embedding model/API |
| Satellite Center | Satellite image classification | Supported vision model and actual implemented image-task adapter |
| Flight Radar | Optional aircraft data explanation | Explicit supported schema/API, no flight data invented |
| RF Geolocation | Optional radio signal classification | Actual tested signal model and RF adapter |
| AI Tools Advisor/Agent | Chat / tool calling | Verified LLM endpoint with enough capability; tools remain independently permissioned |

### Matching output

For each app/task show **Recommended / Candidate / Incompatible / Unknown**, **Already installed** flag, storage owner, path/host, predicted resource footprint, verification results and why. Offer **Use for this task** (approval-driven binding), **Test** (on-demand, permissions), or **Find compatible model in Market** (no auto download).

Never assert that all models in the library are compatible with all apps. Unknown models stay unknown until tested.

## 5. Experimental AI Research & Matching Advisor

**Opt-in** AI-assisted mode can research public model cards, application documentation, supported task adapters and hardware requirements on the Internet, then propose a ranked match with citations, assumptions, confidence and **local validation steps**. Internet OFF by default. It may ask clarifying questions, suggest a better existing model, or recommend a versioned Market download, but must not mistake generated model suggestions for verified compatibility.

Suggested flow:

1. User: “Which existing models in Jan can I use for Survival Kit's AI assistant?”
2. Rust scanner inventories authorized Jan models (no chat/log/secret access).
3. Deterministic compatibility filter removes impossible formats, modes and resource mismatches.
4. Optional LLM analyzes available *metadata* and online sources (not private raw model files unless explicitly approved).
5. UI presents compare cards + explanation; user selects “Use this model”.
6. Integration layer checks current app's declared capabilities and target host, previews approved local path or secure inference endpoint and config diff.
7. User explicitly confirms. Rust-side adapter writes only authorized configuration atomically, smoke tests actual task and rolls back on failure; no model moves needed.

**Model/LLM should not directly rewrite apps**. Its output is a proposal; trusted Rust code validates the mapping and the explicit approval. Do not infer an unknown application's internal writable path from web search alone.

## 6. Relocation without breakage

Default preference is **link/reference in place**, with no external owner changes. Additional user-selected actions:
- **Copy:** safe staged copy into `~/TIHULU AI Tools/Models`, hash verification, consumer opt-in, original preserved.
- **Move:** preview all consumers, check permissions and disk space, stage+verify, update only approved mappings with backup, run tests; only after success and separately approved cleanup may external original be touched. A foreign owner's native path requires *that owner's supported relocation mechanism*; never silently patch Jan internals or remove active models.
- **Serve:** use existing Jan/Ollama inference endpoint instead of file access when appropriate.
- **Repoint consumer:** change *TIHULU app's* selected mapping to reference the same existing external path without changing that path.

An owner app's model configuration and a consumer app's `aipath.txt` are two different authorities. Changes to the latter do not imply permission for the former.

## 7. UI acceptance criteria (planning)

- Search by installed **app name**, select an arbitrary directory, or enter a vetted local inference endpoint; permissions and scope visible.
- Show detected owner, path/endpoint, status, format, model provenance, duplication and compatibility information.
- List **which TIHULU app/feature can use which installed model**, with reasons and host selection.
- Allow optional AI-assisted online research with explicit network permission and source links.
- Pair an existing model with a user-approved app without copying, downloading or moving it.
- Make migration optional; preview dependent apps, ensure owner-native relocation when needed, verify and rollback.
- All functionality remains simulated in the v0.6 HTML concept. No filesystem or model-server probing is performed.

## Reference sources

- Jan Desktop data folder: https://docs.jan.ai/desktop/data-folder
- Jan Desktop model management: https://docs.jan.ai/desktop/manage-models
- Jan local inference API: https://jan.ai/docs/api-server
- Jan Desktop MCP support: https://docs.jan.ai/desktop/integrations/mcp-servers
