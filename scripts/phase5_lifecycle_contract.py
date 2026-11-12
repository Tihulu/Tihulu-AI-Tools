#!/usr/bin/env python3
"""Pure synthetic owner/lease lifecycle model. NEVER manages OS processes.

Used only to validate expected GUI/independent-host separation and cleanup
accounting. It cannot free RAM/VRAM, stop Jan/Ollama/readsb/tar1090, manage
Windows Job Objects, or prove hardware teardown.
"""
from __future__ import annotations
from dataclasses import dataclass, field

BORROWED = frozenset({'Jan', 'Ollama', 'readsb', 'tar1090'})


@dataclass
class MockRuntime:
    independent_host_opt_in: bool = False
    host_running: bool = False
    gui_running: bool = True
    clients: set[str] = field(default_factory=set)
    leases: dict[str, set[str]] = field(default_factory=dict)
    unloaded: list[str] = field(default_factory=list)
    owned_workers: dict[str, set[str]] = field(default_factory=dict)
    released_workers: list[tuple[str, str]] = field(default_factory=list)
    mock_allocations: dict[str, tuple[int, int]] = field(default_factory=dict)
    borrowed: frozenset[str] = BORROWED
    borrowed_ops: int = 0

    def enable_independent_host(self, explicit_consent: bool = False) -> None:
        if not explicit_consent:
            raise PermissionError('Independent GPU host requires explicit consent')
        self.independent_host_opt_in = True
        self.host_running = True

    def acquire(self, client: str, model: str) -> None:
        if not self.gui_running and client == 'gui':
            raise RuntimeError('GUI already stopped')
        if client not in self.clients:
            raise PermissionError('Unknown client')
        if not model or not isinstance(model, str):
            raise ValueError('Model identity required')
        self.leases.setdefault(model, set()).add(client)

    def register_worker(self, client: str, worker: str, ram_mb: int = 0, vram_mb: int = 0) -> None:
        if client not in self.clients:
            raise PermissionError('Unknown client')
        if client in self.borrowed or not worker:
            raise PermissionError('Cannot claim borrowed provider worker')
        if ram_mb < 0 or vram_mb < 0:
            raise ValueError('Invalid mock memory counters')
        self.owned_workers.setdefault(client, set()).add(worker)
        self.mock_allocations[client] = (ram_mb, vram_mb)

    def stop_client(self, client: str) -> None:
        # Only leases owned by the specified client; repeated Stop is idempotent.
        for model in tuple(sorted(self.leases)):
            owners = self.leases[model]
            owners.discard(client)
            if not owners:
                del self.leases[model]
                self.unloaded.append(model)
        for worker in sorted(self.owned_workers.pop(client, set())):
            self.released_workers.append((client, worker))
        self.mock_allocations.pop(client, None)
        self.clients.discard(client)

    def quit_gui(self) -> None:
        self.stop_client('gui')
        self.gui_running = False
        # An independent host is NOT a child of the GUI lifetime.
        if not self.independent_host_opt_in:
            self.host_running = False

    def stop_host(self, consent: bool = False) -> None:
        if not consent:
            raise PermissionError('Stopping independent host requires explicit confirmation')
        for client in tuple(sorted(self.clients)):
            self.stop_client(client)
        self.host_running = False

    def snapshot(self) -> dict:
        return {
            'gui_running': self.gui_running,
            'host_running': self.host_running,
            'host_independent': self.independent_host_opt_in,
            'models_with_active_leases': sorted(self.leases),
            'mock_owner_ram_mb': sum(x[0] for x in self.mock_allocations.values()),
            'mock_owner_vram_mb': sum(x[1] for x in self.mock_allocations.values()),
            'borrowed_untouched': self.borrowed_ops == 0,
            'physical_memory_verified': False,
            'physical_gpu_verified': False,
        }
