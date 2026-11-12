# Architecture diagrams — planning baseline v0.6

**Status:** Design diagrams, not a deployed architecture. These Mermaid diagrams are the text-editable reference for future discussions and reviews. They render in GitHub Markdown.

## A. System boundaries

```mermaid
flowchart TB
  subgraph APPS["Independent clients — never require AI Tools GUI at startup"]
    MC["Media Center"]
    TC["Text Center"]
    SC["Satellite Center"]
    FR["Flight Radar"]
    GEO["RF Geolocation"]
    SK["Survival Kit (Pi 5)"]
    EXT["Authorized external apps"]
  end
  subgraph TOOLS["TIHULU AI Tools — proposed Rust control plane"]
    UI["Tauri desktop / responsive web UI"]
    REG["Model Registry + Market"]
    MAP["Client mappings / aipath.txt"]
    POLICY["Permissions / resource policies"]
    HOST["Optional secure GPU Host"]
    ADAPTER["Runtime adapters"]
  end
  subgraph STORAGE["Existing and managed storage"]
    LOCAL["~/TIHULU AI Tools/Models"]
    JAN["Jan local models"]
    OLLAMA["Ollama & other owned stores"]
    USER["Custom / external SSD"]
  end
  subgraph RUNTIMES["Verified execution runtimes"]
    CPU["CPU: x86-64 / ARM64"]
    NVIDIA["NVIDIA CUDA"]
    HAILO["Hailo-10H / AI HAT+ 2"]
    REMOTE["Paired remote host"]
  end
  APPS -->|"optional local paths / authorized APIs"| MAP
  APPS -->|"optional inference requests"| HOST
  UI --> REG
  UI --> POLICY
  REG --> STORAGE
  REG --> ADAPTER
  MAP --> STORAGE
  HOST --> ADAPTER
  ADAPTER --> RUNTIMES
  APPS -.->|"legacy fallback when manager unavailable"| STORAGE
```

## B. Hardware-aware model discovery and installation

```mermaid
flowchart TD
  A["Auto Detect or selected hardware profile"] --> B["Inventory CPU / RAM / GPU / NPU / disk / OS"]
  B --> C["Scan known model directories with permission"]
  C --> D{"Compatible model already installed?"}
  D -->|Yes| E["Register external path or reuse serving API"]
  D -->|No| F["Market: show compatible model variants"]
  F --> G["Review source, license, resources, storage and version"]
  G --> H{"User approves download?"}
  H -->|No| Z["No changes"]
  H -->|Yes| I["Staged download + integrity check"]
  I --> J["Register model version"]
  E --> K["Test runtime + model + hardware"]
  J --> K
  K --> L{"Verified inference?"}
  L -->|Yes| M["Available for eligible apps"]
  L -->|No| N["Keep original model safe; report error"]
```

## C. GPU Server: two distinct modes

```mermaid
flowchart LR
  subgraph PI["Raspberry Pi 5 + optional AI HAT+ 2"]
    PIM["Pi-stored models / data"]
    SURV["Survival Kit / browser"]
  end
  subgraph PC["Paired x86-64 GPU host"]
    STORE["PC model library"]
    CACHE["Approved staging cache"]
    RTX["Verified RTX/CUDA runtime"]
    API["Authenticated inference API"]
  end
  PHONE["Phone browser / client"] -->|"HTTPS requests"| API
  SURV -->|"Mode A: PC-hosted model"| API
  STORE --> RTX
  PIM -->|"Mode B: explicit transfer when compatible"| CACHE
  CACHE --> RTX
  API --> RTX
  API -->|"Results"| SURV
  API -->|"Results"| PHONE
```

A phone opening the web UI is **a client**, not automatically a GPU server. A Pi model cannot run on NVIDIA simply because it exists: the model artifact must be supported by the PC runtime. Hailo HEF and CUDA models are not interchangeable.

## D. Application startup: GUI must not be required

```mermaid
flowchart TD
  OPEN["Open integrated TIHULU app"] --> CFG{"Valid aipath.txt / integration manifest?"}
  CFG -->|Yes| PATH["Resolve approved local model path"]
  CFG -->|No| LEGACY["Use unchanged legacy settings"]
  PATH --> LOCAL{"Local runtime/model ready?"}
  LOCAL -->|Yes| USE["Use model without launching AI Tools GUI"]
  LOCAL -->|No| HOST{"Remote host explicitly configured and reachable?"}
  HOST -->|Yes| RUSE["Use authorized remote inference service"]
  HOST -->|No| LEGACY
  LEGACY --> END["Run all independent features; clearly report unavailable AI task"]
```

## E. Experimental LLM modules and permissions

```mermaid
flowchart TB
  USER["User"] --> ADVISOR["AI Advisor (experimental)"]
  USER --> AGENT["App Control Agent (experimental)"]
  ADVISOR --> MODELS["Model Registry + Market"]
  ADVISOR --> GUIDE["Explain / suggest integration / draft plan"]
  AGENT --> PLAN["Plan task; list affected apps and actions"]
  PLAN --> APPROVAL{"Scoped permission and user approval?"}
  APPROVAL -->|No| STOP["No effect"]
  APPROVAL -->|Yes| TOOLS["Supported app adapters / permissioned tools"]
  TOOLS --> AUDIT["Action log + cancellation / rollback where supported"]
  MODELS --> PROVIDERS["Local Jan / Ollama / compatible hosted LLM provider"]
  AGENT --> PROVIDERS
```

**Security baseline:** no Internet access or unscoped agent execution by default; credentials only in OS secret storage; no tokens in `aipath.txt`. Raw llama.cpp RPC is not an acceptable unauthenticated public endpoint.

## F. Storage and path portability

```mermaid
flowchart TD
  FOUND["Found existing Jan / Ollama / custom model"] --> REGISTER["Read-only inventory + owner & hash"]
  REGISTER --> CHOOSE{"User selects action"}
  CHOOSE -->|Link in place| LINK["Registry reference; do not modify owner"]
  CHOOSE -->|Copy| COPY["Copy to managed library; verify checksum"]
  CHOOSE -->|Move| STAGE["Plan + backup + stage + integrity check"]
  STAGE --> TEST["Repoint allowed client mappings and test"]
  TEST --> OK{"All dependent clients confirmed?"}
  OK -->|Yes| CLEAN["Commit mapping; optional approved cleanup"]
  OK -->|No| ROLLBACK["Restore old mappings and files"]
```

## G. Raspberry Pi AI Tools uses the paired PC GPU

```mermaid
flowchart TB
  subgraph PI["Raspberry Pi 5 · Rust AI Tools client"]
    CHAT["AI Advisor / Agent chats"]
    APPS["Survival Kit & independent TIHULU apps"]
    ROUTE["Rust task routing and permission policy"]
    MAP["Per-app aipath.txt + paired host registry"]
    CHAT --> ROUTE
    APPS --> MAP
    MAP --> ROUTE
  end
  subgraph PC["PC · Rust AI Tools GPU host"]
    HOST["Authenticated Rust host service"]
    MODELS["PC model registry (Jan / Ollama / managed)"]
    GPU["Verified CUDA / CPU inference adapter"]
    HOST --> MODELS
    HOST --> GPU
  end
  ROUTE <-->|"TLS: Ethernet, LAN or VPN"| HOST
  ROUTE -->|"Independent approval gate"| ACTION["Local permitted app tools"]
```

The Pi can use the same paired PC host for AI Tools' **own** chat inference and for individually authorized applications. Model weights may remain on PC. The GUI is not required at application startup; remote jobs require a reachable host service.

## H. Feature-level inference routing

```mermaid
flowchart TD
  R["AI Tools feature or app AI request"] --> PREF["Auto / This device / Paired GPU"]
  PREF --> L{"Compatible local runtime?"}
  L -->|"Local chosen and compatible"| LOCAL["Run locally"]
  L -->|"Remote chosen / approved fallback"| P{"Paired host authorized + reachable + compatible?"}
  P -->|Yes| HOST["Run on PC GPU; show host/model + queue status"]
  P -->|No| FAIL["Report AI task unavailable; app stays usable"]
  LOCAL --> OUTPUT["Return result with origin"]
  HOST --> OUTPUT
  HOST --> POLICY["Quota, cancellation, and audit"]
```

## I. Chat agents share routing, not tool privileges

```mermaid
flowchart LR
  ADVISOR["Advisor conversation"] --> ROUTER["Rust inference router"]
  AGENT["App Agent conversation"] --> ROUTER
  ROUTER --> LOCAL["Local compatible LLM"]
  ROUTER --> PC["Paired PC GPU"]
  AGENT --> GATE{"Scoped tool permission and user approval"}
  GATE -->|Yes| APP["Approved app adapter"]
  GATE -->|No| DENY["No app action"]
```

## J. Discovery by app or folder; register, then match

```mermaid
flowchart TD
  SRC["Owner enters app name, directory, model file or authorized local API"] --> SCOPE{"User approves read-only scope?"}
  SCOPE -->|No| STOP["No scan / no changes"]
  SCOPE -->|Yes| DETECT["Rust discovery adapters: Jan / Ollama / other / custom"]
  DETECT --> INVENTORY["Model metadata, format, owner, candidate runtime, ID/hash"]
  INVENTORY --> REG["Model Market registry: Already installed / External reference"]
  REG --> MATCH["Task-level deterministic compatibility and suitability"]
  MATCH --> READY{"Verified target model + runtime + app adapter?"}
  READY -->|No| PROPOSE["Show unknown/incompatible or Market alternatives"]
  READY -->|Yes| BIND{"User chooses Use this model & approves config preview?"}
  BIND -->|No| REG
  BIND -->|Yes| CONFIG["Atomically update approved app model or inference mapping"]
  CONFIG --> TEST{"Smoke test on approved target?"}
  TEST -->|Pass| ACTIVE["Binding active; owner model path unchanged"]
  TEST -->|Fail| ROLL["Rollback consumer mapping; preserve owner files"]
```

## K. Experimental online AI recommendations

```mermaid
flowchart TB
  TASK["Chosen TIHULU app + AI task"] --> HARD["Hard filters: modality / runtime / format / memory / adapter"]
  HARD --> META["Local registered model candidates + versioned evidence"]
  META --> OPT{"User opts into Internet research?"}
  OPT -->|No| RANK["Evidence-grounded local suggestions"]
  OPT -->|Yes| WEB["Fetch permitted public docs and model cards"]
  WEB --> LLM["Optional local or paired GPU LLM explains/compares; cite evidence"]
  LLM --> RANK
  RANK --> REVIEW["User reviews candidates + uncertainty + test plan"]
  REVIEW --> ENGINE["Trusted Rust adapter validates before any approved bind"]
```

## L. Jan chat tools through optional Rust MCP bridge

```mermaid
sequenceDiagram
  participant U as User
  participant J as Jan Desktop Chat (MCP client)
  participant M as Rust TIHULU MCP Server
  participant P as Permission broker
  participant T as TIHULU app adapter
  U->>J: Ask for an authorized TIHULU app operation
  J->>M: Call named, scoped MCP tool
  M->>P: Validate Jan client / app scope / action
  alt Approved
    P->>T: Invoke supported task adapter
    T-->>M: Actual result
    M-->>J: Scoped result
  else Not approved
    P-->>M: Denied / approval required
    M-->>J: Permission error
  end
  J-->>U: Reply in Jan chat
```

Jan Desktop MCP integration is independent of Jan Agent's separate plugin package; no generic shell or unrestricted device control is exposed.

## Related specifications

- [Full product plan](TIHULU_AI_TOOLS_PLAN.md)
- [Proposed aipath compatibility contract](AIPATH_CONTRACT.md)
- [Decisions and phase gates](DECISIONS.md)
- [Interactive mockup (non-functional)](../ui/index.html)
- [Universal Model Discovery & Matching](MODEL_DISCOVERY_AND_MATCHING.md)
- [Jan Desktop and Jan Agent optional integrations](JAN_INTEGRATION.md)
