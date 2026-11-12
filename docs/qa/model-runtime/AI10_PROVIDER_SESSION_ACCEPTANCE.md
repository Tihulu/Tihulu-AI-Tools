# AI-10 — Independent synthetic provider-session and CPU fallback acceptance

**Scope:** isolated Draft PR only; release and authentic inference **HOLD**. This is
*not a model runner*, Jan/Ollama connection, native UI/backend implementation,
or deployed permissions system. It extends the independent evaluation of
[AI-10 #38](https://github.com/Tihulu/Tihulu-AI-Tools/issues/38), alongside
the pre-existing [PR #40](https://github.com/Tihulu/Tihulu-AI-Tools/pull/40)
decision oracle (15 synthetic tests), without altering that PR.

## Exact frozen ancestry / immutability

- Phase 5 [PR #32](https://github.com/Tihulu/Tihulu-AI-Tools/pull/32):
  `48669240e4595908363f48f4229e70f55e72d7db` (source/mock only).
- Phase 6 GUI [PR #33](https://github.com/Tihulu/Tihulu-AI-Tools/pull/33):
  `3684716f95a5cc411cb7999b94e5600f6cb94bef`.
- Phase 6 live read-only WebView IPC [PR #34](https://github.com/Tihulu/Tihulu-AI-Tools/pull/34):
  **base SHA `283edc8a1101f63e08f12c172d6b2d6688b9a5ca`**.
- This new branch is based *directly* on #34, not #40, to avoid ownership
  overlap and editing #40's fixed-four-path guard. Exactly four new files:
  two namespaced `qa/phase6/runtime/ai10_*` files, this document, and one
  additive workflow. Frozen Rust/web source and pre-existing QA untouched.

## Reproduction (no package installs, network, models or device access)

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -B -m unittest discover \
  -s qa/phase6/runtime -p 'test_ai10_synthetic_sessions.py' -v
```

Uses Python standard library only and injected objects, frames and logical
clock ticks. The deliberately *asserted* FakePath canonical/symlink flags
are **not real filesystem realpath or race-safety validation**. Exact literal
loopback checks do not contact or authenticate endpoints. GPU Host
authentication is a synthetic Boolean fixture, not mTLS, real compute or RAM
evidence. No invocation or configuration of Jan, Ollama, OpenCode, GPU Host,
readsb/tar1090 or actual models, and no background service starts.

## Grant/deny acceptance matrix

| Area | Synthetic allowed | Strict denial |
| --- | --- | --- |
| Identity | Exact app, grant, model ID and synthetic provenance | Missing, revoked, cross-app, mismatched or real-scan claims |
| Capability | Explicitly granted and fixture-advertised chat/tool | Tool from chat, agent without separate tool grant, arbitrary shell |
| Endpoint | Exact user-granted literal loopback URL for borrowed Jan/Ollama | DNS, credentials, query secrets, changed port, unknown endpoint |
| Model path | Selected lexical fixture path below granted root | Traversal, misleading prefix, asserted symlink, foreign root |
| CPU fallback | App-owned GPU-unavailable -> CPU only with request AND grant opt-in and ready CPU fixture | Implicit fallback, unready CPU, borrowed provider rerouting |
| GPU | Synthetic requested route or separately synthetic-authenticated host | No true GPU/VRAM/transport claims |
| Streaming | JSON token frames, bounded payload/count, monotonic logical ticks | Malformed/tool-command frame, oversized payload, timeout |
| Revocation | Next token fails closed, cancels own synthetic request | Continued use after grant revocation |
| Clients | Two concurrent app-owned leases, exactly one last-lease release **request** | Premature/double release; borrowed Jan/Ollama process termination |
| Retry | New explicit unique request ID after cancellation | Implicit restart, duplicate ID |
| Prompt injection | Malicious instruction is an inert untrusted token string | Interpreted command or authorization grant |

**Statuses:** fixture `connected` and `cpu-mock` are *synthetic*, while
`actual_model_ran=false`, `physical_cleanup_verified=false`, and
`background_started=false` remain false, including after success. There is no
real native UI state update. These tests cannot attest physical memory cleanup,
robust actual symlink handling, provider networking, real client races, or
actual inference. `owned_release_request_only` is a ledger request, not an
unload receipt.

## Next stages require separate explicit approvals

1. Review typed model/provenance/path/lease IDs with the core Registry
   ResourceLedger and [Jan SDK #6](https://github.com/Tihulu/Tihulu-AI-Tools/issues/6).
   [Jan/OpenCode #36](https://github.com/Tihulu/Tihulu-AI-Tools/issues/36)
   retains ownership of OpenCode config, Jan Agent and MCP integration.
2. Independent adversarial filesystem/socket/native race QA on disposable
   hosts after native/installer gates. Never silently modify user model stores.
3. Only after separately scoped user authorization: selected model license and
   exact path/digest, approved isolated CPU runner, actual prompt/response and
   engine/model identity, measured RAM/CPU logs, forced cancellation and safe
   cleanup. Consent must distinguish discovery, connection and model-ran.
4. Separately authorized Jan/Ollama endpoint read-only selection and authentic
   inference, authenticated GPU Host, RTX/CUDA, Metal and Hailo physical tests;
   record real resource release by last owned lease without stopping borrowed
   providers. Windows/macOS/Pi native installed acceptance is separate.
5. Independent QA and coordinator-owned composite CI is required. The
   inherited #34 live IPC path allowlist intentionally rejects new QA workflow
   paths on downstream branches; do **not** weaken it. #34's exact-head green
   run is evidence for #34 only. Composite CI/merge/release **HOLD**.

**No merge, release, user-machine access, model/provider download, model scan,
network access, installer, service change or external app configuration edit.**
Links: [AI coordinator #15](https://github.com/Tihulu/Tihulu-AI-Tools/issues/15);
[master #8](https://github.com/Tihulu/tihulu-survival-kit-plan/issues/8).
