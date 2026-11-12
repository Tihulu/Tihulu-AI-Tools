//! Pure, owner-aware model lease bookkeeping; never sends OS kill signals.
//!
//! A caller must own and explicitly supervise its actual runtime handles.
//! Last-lease events are *instructions* to the owner, not evidence of release.

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeOwner {
    /// Only this application/host may unload the tracked owned model context.
    AiTools { supervisor_id: String },
    /// A Jan/Ollama/other application's runtime, not ours to unload or kill.
    BorrowedExternal { application: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Consumer {
    pub app_session: String,
    pub task_session: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseToken {
    pub runtime_id: String,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseAction {
    AlreadyReleased,
    /// Other authorized consumers still need the same model context.
    KeepLoaded { remaining: usize },
    /// The owner must stop owned work, unload the context and acknowledge it.
    UnloadOwned { runtime_id: String, model_id: String },
    /// Release this client's requests/streams; NEVER stop Jan/Ollama.
    DisconnectBorrowedClient { runtime_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    DuplicateRuntime,
    UnknownRuntime,
    Stopping,
    InvalidConsumer,
}

#[derive(Debug)]
struct LoadedRuntime {
    model_id: String,
    owner: RuntimeOwner,
    leases: BTreeMap<u64, Consumer>,
    accepting: bool,
}

#[derive(Debug, Default)]
pub struct LeaseRegistry {
    runtimes: BTreeMap<String, LoadedRuntime>,
    next_sequence: u64,
}

impl LeaseRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a runtime that its owning backend *already* loaded.
    /// This function never loads a model or allocates RAM/VRAM.
    pub fn register_loaded_runtime(
        &mut self,
        runtime_id: impl Into<String>,
        model_id: impl Into<String>,
        owner: RuntimeOwner,
    ) -> Result<(), LeaseError> {
        let runtime_id = runtime_id.into();
        let model_id = model_id.into();
        if runtime_id.is_empty() || model_id.is_empty() {
            return Err(LeaseError::InvalidConsumer);
        }
        if self.runtimes.contains_key(&runtime_id) {
            return Err(LeaseError::DuplicateRuntime);
        }
        self.runtimes.insert(
            runtime_id,
            LoadedRuntime {
                model_id,
                owner,
                leases: BTreeMap::new(),
                accepting: true,
            },
        );
        Ok(())
    }

    /// One token per authorized task/session; callers must enforce real
    /// authorization and serialize/mutex-protect access to this registry.
    pub fn acquire(&mut self, runtime_id: &str, consumer: Consumer) -> Result<LeaseToken, LeaseError> {
        if consumer.app_session.is_empty() || consumer.task_session.is_empty() {
            return Err(LeaseError::InvalidConsumer);
        }
        let state = self.runtimes.get_mut(runtime_id).ok_or(LeaseError::UnknownRuntime)?;
        if !state.accepting {
            return Err(LeaseError::Stopping);
        }
        self.next_sequence = self.next_sequence.checked_add(1).ok_or(LeaseError::Stopping)?;
        let token = LeaseToken {
            runtime_id: runtime_id.to_owned(),
            sequence: self.next_sequence,
        };
        state.leases.insert(token.sequence, consumer);
        Ok(token)
    }

    /// Idempotent: a repeated release cannot unload another consumer's model.
    pub fn release(&mut self, token: &LeaseToken) -> ReleaseAction {
        let Some(state) = self.runtimes.get_mut(&token.runtime_id) else {
            return ReleaseAction::AlreadyReleased;
        };
        if state.leases.remove(&token.sequence).is_none() {
            return ReleaseAction::AlreadyReleased;
        }
        if !state.leases.is_empty() {
            return ReleaseAction::KeepLoaded {
                remaining: state.leases.len(),
            };
        }
        match &state.owner {
            RuntimeOwner::AiTools { .. } => {
                state.accepting = false;
                ReleaseAction::UnloadOwned {
                    runtime_id: token.runtime_id.clone(),
                    model_id: state.model_id.clone(),
                }
            }
            RuntimeOwner::BorrowedExternal { .. } => {
                ReleaseAction::DisconnectBorrowedClient {
                    runtime_id: token.runtime_id.clone(),
                }
            }
        }
    }

    /// App Quit / Global Hard Stop: release this app's tokens only.
    pub fn close_app(&mut self, app_session: &str) -> Vec<ReleaseAction> {
        let tokens: Vec<_> = self.runtimes.iter().flat_map(|(runtime_id, state)| {
            state.leases.iter().filter_map(move |(sequence, consumer)| {
                (consumer.app_session == app_session).then(|| LeaseToken {
                    runtime_id: runtime_id.clone(),
                    sequence: *sequence,
                })
            })
        }).collect();
        tokens.iter().map(|token| self.release(token)).collect()
    }

    /// Close one task without closing the entire application.
    pub fn close_task(&mut self, app_session: &str, task_session: &str) -> Vec<ReleaseAction> {
        let tokens: Vec<_> = self.runtimes.iter().flat_map(|(runtime_id, state)| {
            state.leases.iter().filter_map(move |(sequence, consumer)| {
                (consumer.app_session == app_session && consumer.task_session == task_session)
                    .then(|| LeaseToken {
                        runtime_id: runtime_id.clone(),
                        sequence: *sequence,
                    })
            })
        }).collect();
        tokens.iter().map(|token| self.release(token)).collect()
    }

    /// Call only after the owned backend proves that its context was unloaded.
    /// A borrowed runtime can never be acknowledged as ours.
    pub fn acknowledge_owned_unloaded(&mut self, runtime_id: &str) -> bool {
        let ready = self.runtimes.get(runtime_id).is_some_and(|state| {
            matches!(&state.owner, RuntimeOwner::AiTools { .. })
                && !state.accepting
                && state.leases.is_empty()
        });
        if ready {
            self.runtimes.remove(runtime_id);
        }
        ready
    }

    pub fn active_leases(&self, runtime_id: &str) -> usize {
        self.runtimes
            .get(runtime_id)
            .map_or(0, |state| state.leases.len())
    }

    pub fn runtime_count(&self) -> usize {
        self.runtimes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ai() -> RuntimeOwner {
        RuntimeOwner::AiTools { supervisor_id: "synthetic-owned-host".into() }
    }

    fn consumer(app: &str, task: &str) -> Consumer {
        Consumer { app_session: app.into(), task_session: task.into() }
    }

    #[test]
    fn l02_advisor_close_preserves_agent_on_shared_context() {
        let mut tracker = LeaseRegistry::new();
        tracker.register_loaded_runtime("ctx", "model", ai()).unwrap();
        let _advisor = tracker.acquire("ctx", consumer("app", "advisor")).unwrap();
        let _agent = tracker.acquire("ctx", consumer("app", "agent")).unwrap();
        assert_eq!(tracker.close_task("app", "advisor"), vec![ReleaseAction::KeepLoaded { remaining: 1 }]);
        assert_eq!(tracker.active_leases("ctx"), 1);
        assert_eq!(tracker.close_task("app", "agent"), vec![
            ReleaseAction::UnloadOwned { runtime_id: "ctx".into(), model_id: "model".into() }
        ]);
    }

    #[test]
    fn l03_l04_last_authorized_host_lease_unloads_only_its_model() {
        let mut tracker = LeaseRegistry::new();
        tracker.register_loaded_runtime("host-model", "model", ai()).unwrap();
        let first = tracker.acquire("host-model", consumer("media", "asr")).unwrap();
        let second = tracker.acquire("host-model", consumer("text", "asr")).unwrap();
        assert_eq!(tracker.release(&first), ReleaseAction::KeepLoaded { remaining: 1 });
        assert_eq!(tracker.active_leases("host-model"), 1);
        assert_eq!(tracker.release(&second), ReleaseAction::UnloadOwned {
            runtime_id: "host-model".into(), model_id: "model".into()
        });
        assert_eq!(tracker.release(&second), ReleaseAction::AlreadyReleased);
        assert_eq!(tracker.acquire("host-model", consumer("new", "task")), Err(LeaseError::Stopping));
        assert!(tracker.acknowledge_owned_unloaded("host-model"));
    }

    #[test]
    fn l07_external_jan_ollama_is_never_unloaded() {
        let mut tracker = LeaseRegistry::new();
        tracker.register_loaded_runtime("jan", "borrowed", RuntimeOwner::BorrowedExternal {
            application: "Jan Desktop".into()
        }).unwrap();
        let token = tracker.acquire("jan", consumer("media", "chat")).unwrap();
        assert_eq!(tracker.release(&token), ReleaseAction::DisconnectBorrowedClient { runtime_id: "jan".into() });
        assert!(!tracker.acknowledge_owned_unloaded("jan"));
        assert_eq!(tracker.runtime_count(), 1);
        assert!(tracker.acquire("jan", consumer("text", "chat")).is_ok());
    }

    #[test]
    fn l11_relaunch_is_fresh_not_stale() {
        let mut tracker = LeaseRegistry::new();
        tracker.register_loaded_runtime("old", "model", ai()).unwrap();
        tracker.acquire("old", consumer("one", "job")).unwrap();
        assert_eq!(tracker.close_app("one").len(), 1);
        assert_eq!(tracker.close_app("one"), Vec::<ReleaseAction>::new());
        assert!(tracker.acknowledge_owned_unloaded("old"));
        tracker.register_loaded_runtime("new", "model", ai()).unwrap();
        assert_eq!(tracker.active_leases("new"), 0);
    }

    #[test]
    fn l15_separate_models_under_one_host_remain_independent() {
        let mut tracker = LeaseRegistry::new();
        tracker.register_loaded_runtime("a", "model-a", ai()).unwrap();
        tracker.register_loaded_runtime("b", "model-b", ai()).unwrap();
        tracker.acquire("a", consumer("app-one", "job")).unwrap();
        tracker.acquire("b", consumer("app-two", "job")).unwrap();
        assert_eq!(tracker.close_app("app-one"), vec![
            ReleaseAction::UnloadOwned { runtime_id: "a".into(), model_id: "model-a".into() }
        ]);
        assert_eq!(tracker.active_leases("b"), 1);
        assert_eq!(tracker.runtime_count(), 2);
    }
}
