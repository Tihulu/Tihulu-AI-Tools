# Jan Desktop MCP setup contract — proposed, inactive

**No Jan files, registry settings, Jan Agent plugin or MCP configuration were modified.**

The screen is named External Apps > Jan. Offer How to Connect Jan and
Set Up Jan Integration (EN default; TR/DE planned). Jan Desktop MCP
setup and optional Jan Agent plugin installation are distinct flows.

## Six-stage transaction

1. DETECT: use user-approved application metadata or version-verified,
   supported Jan CLI. Record Desktop vs Agent, installed version and an
   existing TIHULU MCP entry, if any. Do not inspect chats, logs, prompts,
   credentials, models outside separately granted discovery scopes.
2. REVIEW: validate supported bridge executable and installed Jan API
   behavior; preview exact tool list, apps, permissions, transport, data
   disclosure, action implications, version risk and undo operation.
   Read-only capabilities are opt-in grants with no global auto-approval;
   actions are separately off by default and require exact per-call approval.
3. SELECT METHOD: if the installed CLI supports safe, documented MCP
   registration, offer that; otherwise print guided manual steps for
   Jan Desktop Settings > MCP Servers > Add MCP Server. Default STDIO,
   verified executable and arguments; optional secured/paired HTTP only.
   Never patch Jan's mcp_config.json directly. Never listen publicly.
4. APPROVE AND CONFIGURE: explicit owner consent before registration or
   activation; never modify a foreign/pre-existing ambiguous MCP entry.
   The current Rust wizard records state transitions only, not installation.
   Real host adapter would persist a scoped transaction record, register
   through a verified method and confirm its returned entry identity.
5. VERIFY: use a real MCP initialize/tools exchange, verify an actual
   tool-capable model and call an owner-approved harmless read-only tool.
   Both MCP handshake and actual read-only probe must succeed to show
   ConnectedVerified. If bridge absent, report configured/unavailable.
6. DISCONNECT/ROLLBACK: disable/remove ONLY a transaction-owned TIHULU
   entry through Jan's supported mechanism and confirm success. Restore
   prior state when a TIHULU-owned entry was repaired. Never remove or
   modify another app's MCP connection or change global permissions.

## Fail-closed rules and restart

If unsupported Jan version, missing CLI, unknown ownership, unavailable
bridge, failed test or the user cancels, stop and display the exact
failure; provide manual instructions without file modifications. Re-run
detection on reconnect. A duplicate existing owned entry is repaired
idempotently; one with unknown provenance is blocked. Rollback must use
the saved entry identity, not a wildcard name lookup.

Jan Agent provides optional plugin.toml/skills/commands in its own
package at a later stage. Do not assume availability of jan cli mcp
commands merely because Jan Desktop is installed. CLI compatibility is
verified per installed product and release, not silently inferred.

The Rust SetupWizard represents a pure transition log. Detection,
registration, handshake, disconnect and rollback outcomes are mock
inputs supplied by a future trusted host; its current state does NOT
prove a live Jan connection. In particular record_configuration(success)
is not an external operation and requires real evidence before invocation.

## Liveness / API separation

Owner-granted read-only model discovery can refer to model files managed
by Jan without moving or loading them. Jan's separately started API
server can be borrowed, never terminated by TIHULU. An MCP server invoked
by Jan does not authorize Jan to start media devices or change another
TIHULU application's settings without its separately verified adapter
and exact-action user approval.

Additional future smoke matrix: Jan Desktop versions/packaging across
Linux x86_64 and ARM64, macOS arm64, Windows x64/ARM64; supported STDIO,
streamable HTTP and permission behavior; no claims about actual Jan Agent
or hardware acceptance from this Rust mock test suite.
