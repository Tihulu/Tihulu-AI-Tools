//! Synthetic lifecycle contract for future preview/tests/download workers.
//!
//! This module only plans cleanup; it does NOT launch, cancel or kill OS tasks.
//! Executors must cancel and await owned workers, unload their *owned* models,
//! free allocations, enforce deadlines and preserve unrelated services.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewResource {
    /// An owned model context, not a model weight file on disk.
    OwnedModel {
        model_id: String,
    },
    /// Runtime/service is externally managed and must never be terminated.
    BorrowedExternal {
        service_id: String,
    },
    RamBuffer {
        bytes: u64,
    },
    VramBuffer {
        bytes: u64,
    },
    NpuBuffer {
        bytes: u64,
    },
    /// A future owned, cancellable download task. No actual download here.
    PendingDownload {
        job_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupAction {
    CancelOwnedDownload { job_id: String },
    FreeRamBuffer { bytes: u64 },
    FreeVramBuffer { bytes: u64 },
    FreeNpuBuffer { bytes: u64 },
    UnloadOwnedModel { model_id: String },
}

impl CleanupAction {
    fn order(&self) -> u8 {
        match self {
            Self::CancelOwnedDownload { .. } => 0,
            Self::FreeRamBuffer { .. }
            | Self::FreeVramBuffer { .. }
            | Self::FreeNpuBuffer { .. } => 1,
            Self::UnloadOwnedModel { .. } => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorError {
    Closed,
    UnknownSession,
    SessionIdExhausted,
}

#[derive(Debug, Default)]
pub struct MarketPreviewSupervisor {
    next_session: u64,
    sessions: BTreeMap<u64, Vec<PreviewResource>>,
    closed: bool,
}

impl MarketPreviewSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_session(&mut self) -> Result<u64, SupervisorError> {
        if self.closed {
            return Err(SupervisorError::Closed);
        }
        self.next_session = self
            .next_session
            .checked_add(1)
            .ok_or(SupervisorError::SessionIdExhausted)?;
        self.sessions.insert(self.next_session, Vec::new());
        Ok(self.next_session)
    }

    pub fn track(
        &mut self,
        session: u64,
        resource: PreviewResource,
    ) -> Result<(), SupervisorError> {
        if self.closed {
            return Err(SupervisorError::Closed);
        }
        let resources = self
            .sessions
            .get_mut(&session)
            .ok_or(SupervisorError::UnknownSession)?;
        resources.push(resource);
        Ok(())
    }

    /// Returns only actions for resources that were owned by this session.
    /// The caller must execute and await actions; this is NOT real teardown.
    pub fn close_session(&mut self, session: u64) -> Vec<CleanupAction> {
        let Some(resources) = self.sessions.remove(&session) else {
            return Vec::new();
        };
        let mut actions = Vec::new();
        for resource in resources {
            match resource {
                PreviewResource::OwnedModel { model_id } => {
                    let still_used = self.sessions.values().any(|resources| {
                        resources.iter().any(|candidate| {
                            matches!(candidate, PreviewResource::OwnedModel { model_id: other } if other == &model_id)
                        })
                    });
                    let already_planned = actions.contains(&CleanupAction::UnloadOwnedModel {
                        model_id: model_id.clone(),
                    });
                    if !still_used && !already_planned {
                        actions.push(CleanupAction::UnloadOwnedModel { model_id });
                    }
                }
                PreviewResource::BorrowedExternal { .. } => {}
                PreviewResource::RamBuffer { bytes } => {
                    actions.push(CleanupAction::FreeRamBuffer { bytes });
                }
                PreviewResource::VramBuffer { bytes } => {
                    actions.push(CleanupAction::FreeVramBuffer { bytes });
                }
                PreviewResource::NpuBuffer { bytes } => {
                    actions.push(CleanupAction::FreeNpuBuffer { bytes });
                }
                PreviewResource::PendingDownload { job_id } => {
                    actions.push(CleanupAction::CancelOwnedDownload { job_id });
                }
            }
        }
        actions.sort_by_key(CleanupAction::order);
        actions
    }

    /// True Quit / Hard Stop: refuse new work, then drain all owned leases.
    /// Never emits a kill action for borrowed Jan/Ollama/readsb/tar1090.
    pub fn close_all(&mut self) -> Vec<CleanupAction> {
        if self.closed {
            return Vec::new();
        }
        self.closed = true;
        let sessions: Vec<u64> = self.sessions.keys().copied().collect();
        let mut actions = Vec::new();
        for session in sessions {
            actions.extend(self.close_session(session));
        }
        actions.sort_by_key(CleanupAction::order);
        actions
    }

    pub fn active_sessions(&self) -> usize {
        self.sessions.len()
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l01_quit_releases_owned_buffers_and_context_without_external_kills() {
        let mut supervisor = MarketPreviewSupervisor::new();
        let s = supervisor.open_session().unwrap();
        supervisor
            .track(
                s,
                PreviewResource::OwnedModel {
                    model_id: "chat".into(),
                },
            )
            .unwrap();
        supervisor
            .track(s, PreviewResource::RamBuffer { bytes: 2048 })
            .unwrap();
        supervisor
            .track(s, PreviewResource::VramBuffer { bytes: 4096 })
            .unwrap();
        supervisor
            .track(
                s,
                PreviewResource::BorrowedExternal {
                    service_id: "jan".into(),
                },
            )
            .unwrap();
        let actions = supervisor.close_all();
        assert_eq!(supervisor.active_sessions(), 0);
        assert!(actions.contains(&CleanupAction::UnloadOwnedModel {
            model_id: "chat".into()
        }));
        assert!(actions.contains(&CleanupAction::FreeVramBuffer { bytes: 4096 }));
        assert!(actions.contains(&CleanupAction::FreeRamBuffer { bytes: 2048 }));
        assert_eq!(actions.len(), 3);
        assert!(supervisor.close_all().is_empty()); // idempotent
    }

    #[test]
    fn l04_model_unloads_only_after_last_owning_client() {
        let mut supervisor = MarketPreviewSupervisor::new();
        let a = supervisor.open_session().unwrap();
        let b = supervisor.open_session().unwrap();
        for session in [a, b] {
            supervisor
                .track(
                    session,
                    PreviewResource::OwnedModel {
                        model_id: "shared".into(),
                    },
                )
                .unwrap();
        }
        assert!(supervisor.close_session(a).is_empty());
        assert_eq!(
            supervisor.close_session(b),
            vec![CleanupAction::UnloadOwnedModel {
                model_id: "shared".into()
            }]
        );
    }

    #[test]
    fn l11_relaunch_uses_fresh_sessions_without_late_restart() {
        let mut first = MarketPreviewSupervisor::new();
        let previous = first.open_session().unwrap();
        first.close_all();
        assert_eq!(first.open_session(), Err(SupervisorError::Closed));
        assert_eq!(
            first.track(previous, PreviewResource::RamBuffer { bytes: 1 }),
            Err(SupervisorError::Closed)
        );
        let mut relaunched = MarketPreviewSupervisor::new();
        assert_eq!(relaunched.open_session(), Ok(1));
    }

    #[test]
    fn l16_download_worker_must_cancel_and_never_turn_into_installed() {
        let mut supervisor = MarketPreviewSupervisor::new();
        let session = supervisor.open_session().unwrap();
        supervisor
            .track(
                session,
                PreviewResource::PendingDownload {
                    job_id: "preview-only".into(),
                },
            )
            .unwrap();
        let actions = supervisor.close_all();
        assert_eq!(
            actions,
            vec![CleanupAction::CancelOwnedDownload {
                job_id: "preview-only".into()
            }]
        );
        assert!(supervisor.close_session(session).is_empty());
    }
}
