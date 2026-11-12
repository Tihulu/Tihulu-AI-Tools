"""AI-10 pure synthetic provider/session acceptance. NEVER a production runtime.

No sockets, subprocesses, OS filesystem reads, GPU calls, model loads, or writes.
All paths, capabilities, responses, timeouts and ownership are injected fixtures.
"""
from __future__ import annotations

from dataclasses import dataclass
import json
import re
from urllib.parse import urlsplit


class Denied(ValueError):
    """Fail-closed mock authorization or runtime boundary."""


@dataclass(frozen=True)
class Provider:
    kind: str
    owner: str
    model_id: str
    capabilities: frozenset[str]
    endpoint: str | None = None
    detected: bool = True
    connected: bool = False
    cpu_ready: bool = False
    gpu_ready: bool = False
    mock_host_authenticated: bool = False
    provenance: str = "synthetic_fixture"


@dataclass(frozen=True)
class Grant:
    grant_id: str
    app_id: str
    provider_kind: str
    model_id: str
    tasks: frozenset[str]
    model_roots: tuple[str, ...] = ()
    endpoint: str | None = None
    cpu_fallback: bool = False
    active: bool = True


@dataclass(frozen=True)
class FakePath:
    """An ASSERTED synthetic canonical path, not an OS realpath check."""
    declared: str
    canonical: str
    symlink_seen: bool = False


@dataclass(frozen=True)
class Request:
    request_id: str
    client_id: str
    app_id: str
    grant_id: str
    task: str
    approved_model_id: str
    prefer_gpu: bool = False
    allow_cpu_fallback: bool = False
    max_logical_tick: int = 10


@dataclass
class Session:
    request: Request
    provider: Provider
    compute: str
    cpu_fallback_used: bool
    active: bool = True
    last_tick: int = -1
    tokens_seen: int = 0
    # Explicitly never evidence of authentic model execution or RAM cleanup.
    actual_model_ran: bool = False
    physical_cleanup_verified: bool = False
    background_started: bool = False


def valid_loopback(url: str) -> bool:
    """Literal endpoint syntax ONLY; never DNS, sockets, or authentication."""
    if not isinstance(url, str) or len(url) > 150:
        return False
    try:
        parsed = urlsplit(url)
        port = parsed.port
    except ValueError:
        return False
    if parsed.scheme != "http" or parsed.hostname not in {"127.0.0.1", "::1"}:
        return False
    if parsed.username is not None or parsed.password is not None:
        return False
    if port is None or not (1 <= port <= 65535):
        return False
    if parsed.query or parsed.fragment or parsed.path not in {"", "/", "/v1", "/v1/"}:
        return False
    # Reject disguised authorities, encodings, zone IDs and non-canonical IPv4.
    literal = ("127.0.0.1:" + str(port), "[::1]:" + str(port))
    return parsed.netloc in literal


def _clean_root(path: str) -> bool:
    return (isinstance(path, str) and path.startswith("/") and path != "/"
            and not path.endswith("/") and "//" not in path
            and all(segment not in {"", ".", ".."} for segment in path.split("/")[1:])
            and "\\x00" not in path)


def granted_path(path: FakePath | None, roots: tuple[str, ...]) -> bool:
    """Pure lexical and pre-resolved fixture check; no disk/symlink traversal."""
    if path is None or path.symlink_seen:
        return False
    if not _clean_root(path.declared) or not _clean_root(path.canonical):
        return False
    if path.declared != path.canonical:
        return False  # Cannot independently attest canonical resolution.
    return any(_clean_root(root) and
               (path.canonical == root or path.canonical.startswith(root + "/"))
               for root in roots)


class SyntheticSessions:
    """Per-client mock request leases; never has external process-control APIs."""

    def __init__(self) -> None:
        self.grants: dict[str, Grant] = {}
        self.sessions: dict[str, Session] = {}
        self.events: list[tuple[str, str]] = []
        self._active_owned: dict[tuple[str, str], set[str]] = {}
        self._released_epochs: set[tuple[str, str]] = set()

    def add_grant(self, grant: Grant) -> None:
        if not grant.grant_id or grant.grant_id in self.grants:
            raise Denied("Missing or duplicate grant")
        self.grants[grant.grant_id] = grant

    def revoke(self, grant_id: str) -> None:
        grant = self.grants.get(grant_id)
        if grant is None:
            raise Denied("Unknown grant")
        self.grants[grant_id] = Grant(
            grant.grant_id, grant.app_id, grant.provider_kind, grant.model_id,
            grant.tasks, grant.model_roots, grant.endpoint, grant.cpu_fallback, False,
        )
        # No automatic process kill. Active sessions fail closed on their next step.

    def begin(self, provider: Provider, request: Request,
              *, model_path: FakePath | None = None) -> Session:
        grant = self.grants.get(request.grant_id)
        if not grant or not grant.active:
            raise Denied("Missing or revoked consent")
        if not request.request_id or request.request_id in self.sessions:
            raise Denied("Missing or duplicate request identity")
        if not request.client_id or not request.app_id or request.max_logical_tick < 0:
            raise Denied("Missing client scope or invalid deadline")
        if grant.app_id != request.app_id or grant.model_id != request.approved_model_id:
            raise Denied("Cross-app or stale model grant")
        if provider.kind != grant.provider_kind or provider.model_id != grant.model_id:
            raise Denied("Provider/model identity mismatch")
        if provider.provenance != "synthetic_fixture" or not provider.detected:
            raise Denied("No attested synthetic provenance/detection")
        if not provider.connected:
            raise Denied("Detected is not connected")
        if request.task not in {"chat", "tool", "agent"}:
            raise Denied("Unknown task")
        if request.task not in provider.capabilities or request.task not in grant.tasks:
            raise Denied("Task not negotiated and granted")
        if request.task == "agent" and (
            "tool" not in provider.capabilities or "tool" not in grant.tasks
        ):
            raise Denied("Chat/tool does not imply agent authorization")

        if provider.kind == "TIHULU_LOCAL":
            if provider.owner != "app-owned" or provider.endpoint is not None:
                raise Denied("Local process ownership mismatch")
            if not granted_path(model_path, grant.model_roots):
                raise Denied("No trusted, selected model-root fixture")
            if request.prefer_gpu and provider.gpu_ready:
                compute, fallback = "gpu-requested-mock", False
            elif request.prefer_gpu:
                if not (request.allow_cpu_fallback and grant.cpu_fallback
                        and provider.cpu_ready):
                    raise Denied("CPU fallback requires two explicit grants and readiness")
                compute, fallback = "cpu-mock", True
            else:
                if not provider.cpu_ready:
                    raise Denied("No CPU fixture capability")
                compute, fallback = "cpu-mock", False
        elif provider.kind in {"JAN", "OLLAMA"}:
            if provider.owner != "borrowed-external":
                raise Denied("Cannot own a borrowed provider")
            if not (valid_loopback(provider.endpoint or "")
                    and provider.endpoint == grant.endpoint):
                raise Denied("No exact approved loopback endpoint")
            if request.prefer_gpu or request.allow_cpu_fallback:
                raise Denied("Provider-managed compute cannot be silently rerouted")
            compute, fallback = "borrowed-provider-managed-mock", False
        elif provider.kind == "GPU_HOST":
            if provider.owner != "borrowed-external":
                raise Denied("GPU host ownership mismatch")
            if not provider.mock_host_authenticated:
                raise Denied("Synthetic host transport lacks an explicit mock grant")
            # A fake host is never hardware evidence. There is no network transport.
            if request.allow_cpu_fallback:
                raise Denied("No implicit remote-to-local CPU fallback")
            compute, fallback = "synthetic-host-request", False
        else:
            raise Denied("Unknown provider")

        session = Session(request, provider, compute, fallback)
        self.sessions[request.request_id] = session
        if provider.owner == "app-owned":
            key = (request.app_id, provider.model_id)
            active = self._active_owned.setdefault(key, set())
            if not active:
                self._released_epochs.discard(key)
            active.add(request.request_id)
        self.events.append(("begin_synthetic", request.request_id))
        return session

    def step(self, request_id: str, raw_frame: str, *, logical_tick: int) -> str:
        session = self.sessions.get(request_id)
        if session is None or not session.active:
            raise Denied("Inactive or unknown request")
        grant = self.grants.get(session.request.grant_id)
        if not grant or not grant.active:
            self.cancel(request_id)
            raise Denied("Consent revoked during streaming")
        if (not isinstance(logical_tick, int) or
                logical_tick <= session.last_tick or
                logical_tick > session.request.max_logical_tick):
            self.cancel(request_id)
            raise Denied("Synthetic deadline or monotonicity failure")
        if not isinstance(raw_frame, str) or len(raw_frame) > 2048:
            self.cancel(request_id)
            raise Denied("Bounded frame required")
        try:
            frame = json.loads(raw_frame)
        except (ValueError, TypeError):
            self.cancel(request_id)
            raise Denied("Malformed JSON frame") from None
        if (not isinstance(frame, dict) or set(frame) != {"kind", "token"}
                or frame.get("kind") != "token"
                or not isinstance(frame.get("token"), str)
                or len(frame["token"]) > 512 or session.tokens_seen >= 8):
            self.cancel(request_id)
            raise Denied("Non-token, tool execution or oversized stream forbidden")
        session.tokens_seen += 1
        session.last_tick = logical_tick
        # This text is never interpreted as tool, instruction, shell, file or URL.
        return frame["token"]

    def cancel(self, request_id: str) -> bool:
        session = self.sessions.get(request_id)
        if not session or not session.active:
            return False
        session.active = False
        self.events.append(("cancel_own_request_only", request_id))
        if session.provider.owner == "app-owned":
            key = (session.request.app_id, session.provider.model_id)
            active = self._active_owned[key]
            active.discard(request_id)
            if not active and key not in self._released_epochs:
                self._released_epochs.add(key)
                self.events.append(("owned_release_request_only", session.provider.model_id))
        # There are intentionally no kill/unload APIs for Jan/Ollama/OpenCode.
        return True
