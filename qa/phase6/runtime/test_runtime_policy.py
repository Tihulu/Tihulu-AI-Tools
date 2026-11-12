"""Synthetic provider policy denials. No model is started, scanned or downloaded."""
from __future__ import annotations

import unittest
from runtime_policy import (
    ProviderView, RuntimeHold, choose_route, choose_stop,
    read_only_loopback_endpoint,
)


class Ai10PolicyTests(unittest.TestCase):
    def local(self, **overrides) -> ProviderView:
        p = dict(provider="TIHULU_LOCAL", ownership="app-owned", approved=True,
                 connected=True, model_id="tiny-local-v1",
                 capabilities=frozenset({"chat", "tool"}), gpu_eligible=True)
        p.update(overrides)
        return ProviderView(**p)

    def jan(self, **overrides) -> ProviderView:
        p = dict(provider="JAN", ownership="borrowed-external", approved=True,
                 connected=True, model_id="jan-approved-1",
                 capabilities=frozenset({"chat"}))
        p.update(overrides)
        return ProviderView(**p)

    def route(self, view=None, **kwargs):
        v = view if view is not None else self.local()
        defaults = dict(approved_model_id=v.model_id, task="chat", cpu_runtime_ready=True)
        defaults.update(kwargs)
        return choose_route(v, **defaults)

    def test_01_explicit_local_cpu_route_is_only_a_policy_decision(self):
        decision = self.route()
        self.assertEqual(decision.selected_compute, "cpu")
        self.assertFalse(decision.start_provider)

    def test_02_detection_without_approval_fails_closed(self):
        with self.assertRaises(RuntimeHold):
            self.route(self.jan(approved=False))

    def test_03_disconnected_or_stale_grant_refused(self):
        with self.assertRaises(RuntimeHold):
            self.route(self.jan(connected=False))
        with self.assertRaises(RuntimeHold):
            self.route(approved_model_id="other-model")

    def test_04_tool_call_must_be_supported(self):
        with self.assertRaises(RuntimeHold):
            self.route(self.jan(), task="tool")

    def test_05_agent_cannot_be_inferred_from_chat_or_tools(self):
        with self.assertRaises(RuntimeHold):
            self.route(task="agent")
        with self.assertRaises(RuntimeHold):
            self.route(self.local(capabilities=frozenset({"chat", "agent"})),
                       task="agent")

    def test_06_gpu_is_routed_only_when_approved_and_reported_eligible(self):
        d = self.route(prefer_gpu=True, gpu_available=True)
        self.assertEqual(d.selected_compute, "gpu-requested")
        self.assertFalse(d.cpu_fallback_used)

    def test_07_missing_gpu_cannot_silently_fallback(self):
        with self.assertRaises(RuntimeHold):
            self.route(prefer_gpu=True, gpu_available=False)

    def test_08_cpu_fallback_requires_optin_and_verified_cpu(self):
        d = self.route(prefer_gpu=True, gpu_available=False, allow_cpu_fallback=True)
        self.assertEqual(d.selected_compute, "cpu")
        self.assertTrue(d.cpu_fallback_used)
        with self.assertRaises(RuntimeHold):
            self.route(prefer_gpu=True, gpu_available=False,
                       allow_cpu_fallback=True, cpu_runtime_ready=False)

    def test_09_borrowed_provider_compute_not_mislabeled_cpu(self):
        d = self.route(self.jan())
        self.assertEqual(d.selected_compute, "provider-managed")
        with self.assertRaises(RuntimeHold):
            self.route(self.jan(), prefer_gpu=True, gpu_available=True)

    def test_10_external_process_never_terminated_on_stop(self):
        d = choose_stop(self.jan(), remaining_authorized_clients=0)
        self.assertTrue(d.cancel_own_request)
        self.assertFalse(d.request_owned_model_release)
        self.assertFalse(d.terminate_provider_process)

    def test_11_last_local_client_can_request_model_lease_release(self):
        self.assertTrue(choose_stop(self.local(), remaining_authorized_clients=0)
                        .request_owned_model_release)
        self.assertFalse(choose_stop(self.local(), remaining_authorized_clients=2)
                         .request_owned_model_release)

    def test_12_unknown_ownership_and_negative_client_count_refused(self):
        with self.assertRaises(RuntimeHold):
            self.route(self.local(ownership="system"))
        with self.assertRaises(RuntimeHold):
            choose_stop(self.local(), remaining_authorized_clients=-1)

    def test_13_local_provider_cannot_claim_jan_ownership(self):
        with self.assertRaises(RuntimeHold):
            self.route(self.jan(ownership="app-owned"))

    def test_14_loopback_endpoint_string_validation_only(self):
        self.assertTrue(read_only_loopback_endpoint("http://127.0.0.1:1337/v1"))
        self.assertTrue(read_only_loopback_endpoint("http://[::1]:11434/"))
        for url in ("https://example.com:1337/v1", "http://example.com:1337/v1",
                    "http://user:pass@127.0.0.1:1337/v1",
                    "http://127.0.0.1:1337/v1?token=leak",
                    "http://127.0.0.1:1337/a/../v1",
                    "http://127.0.0.1/v1", "http://127.0.0.1:99999/v1"):
            with self.subTest(url=url):
                self.assertFalse(read_only_loopback_endpoint(url))

    def test_15_unknown_model_task_and_provider_refused(self):
        for view, task in ((self.local(provider="MAGIC"), "chat"),
                           (self.local(), "root-shell")):
            with self.assertRaises(RuntimeHold):
                self.route(view, task=task)


if __name__ == "__main__":
    unittest.main()
