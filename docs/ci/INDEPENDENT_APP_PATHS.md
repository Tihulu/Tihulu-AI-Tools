# TIHULU independent application installs, shared model directories and legacy migration

**Status: user-confirmed requirement + pure Rust path-planning prototype in coordinator Draft PR #17. No application has been installed, scanned, moved or reconfigured.** This contract supersedes any assumption that AI Tools must be running to resolve a model path.

## Nonnegotiable behavior

- Every TIHULU app (Media Center, Text Center, Satellite Center, Flight Radar, RF Geolocation, Survival Kit and future apps) is independently downloadable, installable, runnable and uninstallable **without TIHULU AI Tools**.
- Installing or closing AI Tools must **not** change app execution, remove model data, block offline operation or kill an external service. AI Tools is an optional opt-in inventory, compatibility, model/path management and remote-compute control plane.
- Each independently installed app may provision models using a common **documented convention**, without requiring AI Tools to exist. It still has its own app settings and a legacy fallback.
- AI Tools can **discover** an installed app's declared installation location, user data/config location and active model root, plus known old model directories **with scoped approval**, and present editable per-app mappings.
- Changing the app **installation/executable directory** is not the same as changing model storage. Actual relocation of an executable/application package belongs to its installer or the OS package manager. AI Tools may update a trusted known installation-location record or invoke an approved app-specific installer adapter, never blindly rename a package-managed app.
- An installed app should consume its own versioned, persistent `aipath.txt` / adapter-owned config even when AI Tools GUI is closed. Apps with no integration keep working from their existing settings.

## Default convention (standalone friendly, proposal to be adopted in each app's installer)

Existing default model root from Registry: `<user-home>/TIHULU AI Tools/Models`. For **new** standalone TIHULU installations:

| Purpose | Relative under model root |
| --- | --- |
| App-managed data/model artifacts | `apps/<stable-app-id>/` |
| Explicitly shared user-managed pool | `shared/` |
| Old app model location | **Unchanged** until a user-approved migration |

The apparent "AI Tools" directory name is a historical compatibility root, **not** an AI Tools software dependency. App adapters must not assume every model is stored there: user overrides, external SSDs, portable installs and legacy layouts are first-class paths.

The path base is resolved from the **real user's native home/data-folder API**, never guessed from `HOME` alone for Windows/multi-profile/native sandbox installs. New apps should make their model directory configurable at installation and in app settings. Existing installations must not silently switch to the new convention.

## Resolution and discovery precedence

1. Explicit, validated **per-app user override** in trusted app configuration.
2. Active path declared by the independently installed app/manifest.
3. New shared model-directory convention, if that app supports it.
4. Read-only candidates from **known legacy paths** or user-granted old folders.

Items 3–4 are **suggestions**, not permission to recursively scan the user's disk. Dedupe exact candidates; never infer an installed/runnable model from folder name. Native adapters require consent, canonical realpath checks, bounded metadata-only scans, verified ownership and no symlink escapes. Only known supported model subfolders may be read; never scan chats, secrets, downloads or unrelated user documents.

## Migration / change model

- **Repoint in place:** approved app configuration changes its model path; no model weights are copied, moved or deleted. Check the application can load the selected model and keep the old mapping for rollback.
- **Managed copy:** explicit consent, source owner and destination grant, disk-space/busy-lease preflight, staged copy, digest/size verification, atomic destination publication, app config backup + atomic update, restart/smoke validation; preserve the source until separate deletion authorization.
- **Managed move:** same safeguards, and additionally forbid source deletion when another app/client has an active lease or a known reference. Never move borrowed Jan/Ollama/HF/external-owned files through a generic file mover. Use an owner-supported migration tool only if separately approved.
- **Failure:** roll back only app-approved mapping/config changes, preserve both model copies and originals, report partial failures; do not take down external services.
- **Versioned manifest:** deterministic path order, app-owned config version, consent/provenance, migration receipt, stable app ID and model reference, backup and rollback reference. No secrets, shell commands, raw untrusted URLs or model payloads.

## Implemented here

`crates/tihulu-ai-registry/src/app_paths.rs` supplies:
- Standalone-compatible app/default/shared path suggestions using the existing library root.
- Provenance-tagged current/override/legacy candidates (no scanning).
- Pure path-change plan with explicit approval, lexical scope bounds, foreign ownership and multiple-consumer guards, and mandatory native verification/rollback flags.
- Deterministic synthetic tests for legacy resolution, shared clients, foreign models, traversal, no approval and non-effectful planning.

**Not implemented:** automatic installed-app detection, actual read-only legacy scans, persistent manifest parsing, app adapter writes, model download/install, file relocation, atomic copy and rollback, real directory customization UI, or real OS install-path migration. They require separate review and live app integration. No claim of deployment to Media/Text/Satellite/Flight Radar/Survival Kit is made.

## App adapter acceptance

Each app needs a small, independently packaged adapter API: `GetInstallInfo`, `GetCurrentModelRoot`, `ListKnownLegacyRoots`, `PreviewModelRootChange`, `ApplyApprovedModelRootChange`, `ValidateModelRuntime` and `RollbackModelRootChange` (names proposed only). The adapter must preserve its local standalone defaults when AI Tools is absent and expose read-only discovery without launching inference or changing services. Config updates require explicit per-app grant and idle/transaction safety.

Related existing proposals: [aipath.txt](../AIPATH_CONTRACT.md), [model discovery](../MODEL_DISCOVERY_AND_MATCHING.md), [architecture](../ARCHITECTURE.md).
