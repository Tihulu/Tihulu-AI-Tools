"""Offline AI-10 policy *oracle* for synthetic CI tests, not production inference.

No network, model probing, filesystem writes, process spawning or GPU access.
"""
from __future__ import annotations

from dataclasses import dataclass
from urllib.parse import urlsplit


class RuntimeHold(ValueError):
    pass


@dataclass(frozen=True)
class ProviderView:
    provider: str
    ownership: str
    approved: bool
    connected: bool
    model_id: str
    capabilities: frozenset[str]
    gpu_eligible: bool = False


@dataclass(frozen=True)
class RouteDecision:
    provider: str
    model_id: str
    capability: str
    selected_compute: str
    cpu_fallback_used: bool
    start_provider: bool = False


@dataclass(frozen=True)
class StopDecision:
    cancel_own_request: bool
    request_owned_model_release: bool
    terminate_provider_process: bool = False


def read_only_loopback_endpoint(url: str) -> bool:
    """Validate an opt-in URL string; does NOT authenticate or connect."""
    try:
        target = urlsplit(url)
        port = target.port
    except (TypeError, ValueError):
        return False
    return (
        target.scheme == "http"
        and target.hostname in ("127.0.0.1", "::1")
        and port is not None
        and 1 <= port <= 65535
        and target.username is None
        and target.password is None
        and target.path in ("", "/", "/v1", "/v1/")
        and not target.query
        and not target.fragment
    )


def choose_route(
    view: ProviderView,
    *,
    approved_model_id: str,
    task: str,
    prefer_gpu: bool = False,
    gpu_available: bool = False,
    allow_cpu_fallback: bool = False,
    cpu_runtime_ready: bool = False,
) -> RouteDecision:
    if view.provider not in {"TIHULU_LOCAL", "JAN", "OLLAMA"}:
        raise RuntimeHold("Unrecognized provider")
    if view.ownership not in {"app-owned", "borrowed-external"}:
        raise RuntimeHold("Unknown ownership boundary")
    if view.provider == "TIHULU_LOCAL" and view.ownership != "app-owned":
        raise RuntimeHold("Invalid ownership for TIHULU local provider")
    if view.provider != "TIHULU_LOCAL" and view.ownership != "borrowed-external":
        raise RuntimeHold("Never take ownership of externally run providers")
    if not view.approved or not view.connected:
        raise RuntimeHold("Detection or connection is not consent")
    if not approved_model_id or approved_model_id != view.model_id:
        raise RuntimeHold("Selected approved model identity differs")
    if task not in {"chat", "tool", "agent"}:
        raise RuntimeHold("Unsupported task")
    if task not in view.capabilities:
        raise RuntimeHold("Model does not advertise requested capability")
    if task == "agent" and "tool" not in view.capabilities:
        raise RuntimeHold("Agent capability requires separately attested tool use")

    if view.ownership == "borrowed-external":
        if prefer_gpu:
            raise RuntimeHold("External provider owns CPU/GPU routing; no fake GPU promise")
        return RouteDecision(view.provider, view.model_id, task, "provider-managed", False)

    if prefer_gpu and gpu_available and view.gpu_eligible:
        return RouteDecision(view.provider, view.model_id, task, "gpu-requested", False)
    if prefer_gpu and not allow_cpu_fallback:
        raise RuntimeHold("CPU fallback was not authorized")
    if not cpu_runtime_ready:
        raise RuntimeHold("No verified compatible local CPU runtime")
    return RouteDecision(
        view.provider, view.model_id, task, "cpu",
        bool(prefer_gpu),
    )


def choose_stop(view: ProviderView, *, remaining_authorized_clients: int) -> StopDecision:
    if remaining_authorized_clients < 0:
        raise RuntimeHold("Invalid client ledger")
    if view.ownership == "borrowed-external":
        return StopDecision(True, False)
    if view.ownership != "app-owned":
        raise RuntimeHold("Unrecognized lifecycle owner")
    return StopDecision(True, remaining_authorized_clients == 0)
