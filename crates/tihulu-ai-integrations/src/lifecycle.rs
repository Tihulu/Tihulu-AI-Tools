//! Ownership-specific cleanup contracts and bounded MOCK shutdown checks.
//! Real adapters must implement nonblocking callbacks and OS-native worker
//! supervision; no process-name kill or generic service stop is provided.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ownership {
    AppOwned,
    BorrowedExternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    McpTask,
    OwnedAiWorker,
    OwnedChild,
    SdrConnection,
    RemoteModelLease,
    BorrowedJan,
    BorrowedOllama,
    BorrowedReadsb,
    BorrowedTar1090,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Running,
    Stopping,
    Stopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    Stop,
    Exit,
    HardStop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    InvalidSession,
    AlreadyStarted,
    UnknownSession,
    SessionStopping,
}

/// Implementations MUST be nonblocking and scoped to a previously captured
/// resource/child identity. force_close_owned MUST NEVER search by process
/// name, terminate another user's service or tear down a shared host.
pub trait ResourceHandle: Send + Sync {
    fn request_cancel(&self);
    fn is_released(&self) -> bool;
    fn force_close_owned(&self);
}

struct ResourceRecord {
    session: String,
    ownership: Ownership,
    #[allow(dead_code)]
    kind: ResourceKind,
    cancellation: CancellationToken,
    handle: Arc<dyn ResourceHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShutdownReport {
    pub cancellation_requests: usize,
    pub force_close_requests: usize,
    pub borrowed_preserved: usize,
    pub unresolved_owned: usize,
    pub state: SessionState,
}

#[derive(Default)]
pub struct LifecycleSupervisor {
    states: BTreeMap<String, SessionState>,
    resources: Vec<ResourceRecord>,
}

impl LifecycleSupervisor {
    pub fn start_session(&mut self, session: &str) -> Result<(), LifecycleError> {
        if session.is_empty() || session.len() > 128 {
            return Err(LifecycleError::InvalidSession);
        }
        if self.states.contains_key(session) {
            return Err(LifecycleError::AlreadyStarted);
        }
        self.states
            .insert(session.to_owned(), SessionState::Running);
        Ok(())
    }

    pub fn state(&self, session: &str) -> Option<SessionState> {
        self.states.get(session).copied()
    }

    pub fn track(
        &mut self,
        session: &str,
        ownership: Ownership,
        kind: ResourceKind,
        cancellation: CancellationToken,
        handle: Arc<dyn ResourceHandle>,
    ) -> Result<(), LifecycleError> {
        match self.state(session) {
            Some(SessionState::Running) => {}
            Some(_) => return Err(LifecycleError::SessionStopping),
            None => return Err(LifecycleError::UnknownSession),
        }
        self.resources.push(ResourceRecord {
            session: session.to_owned(),
            ownership,
            kind,
            cancellation,
            handle,
        });
        Ok(())
    }

    /// Stop only this session's resources. Repeated calls are safe. A failed
    /// release leaves state Stopping and reports the leak, never a false clean
    /// exit. Callbacks must be nonblocking for grace to be an upper bound.
    pub fn shutdown(
        &mut self,
        session: &str,
        _reason: ShutdownReason,
        grace: Duration,
    ) -> Result<ShutdownReport, LifecycleError> {
        match self.state(session) {
            None => return Err(LifecycleError::UnknownSession),
            Some(SessionState::Stopped) => {
                return Ok(ShutdownReport {
                    cancellation_requests: 0,
                    force_close_requests: 0,
                    borrowed_preserved: 0,
                    unresolved_owned: 0,
                    state: SessionState::Stopped,
                });
            }
            _ => {}
        }
        self.states
            .insert(session.to_owned(), SessionState::Stopping);
        let mut report = ShutdownReport {
            cancellation_requests: 0,
            force_close_requests: 0,
            borrowed_preserved: 0,
            unresolved_owned: 0,
            state: SessionState::Stopping,
        };
        for resource in self.resources.iter().filter(|r| r.session == session) {
            if resource.ownership == Ownership::AppOwned {
                resource.cancellation.cancel();
                resource.handle.request_cancel();
                report.cancellation_requests += 1;
            } else {
                report.borrowed_preserved += 1;
            }
        }

        let start = Instant::now();
        while start.elapsed() < grace
            && self.resources.iter().any(|r| {
                r.session == session
                    && r.ownership == Ownership::AppOwned
                    && !r.handle.is_released()
            })
        {
            std::thread::sleep(Duration::from_millis(1).min(grace.saturating_sub(start.elapsed())));
        }

        for resource in self.resources.iter().filter(|r| {
            r.session == session && r.ownership == Ownership::AppOwned && !r.handle.is_released()
        }) {
            resource.handle.force_close_owned();
            report.force_close_requests += 1;
        }

        self.resources.retain(|r| {
            if r.session != session {
                return true;
            }
            if r.ownership == Ownership::BorrowedExternal {
                return false; // drop our reference; NEVER call its close callback
            }
            if r.handle.is_released() {
                false
            } else {
                report.unresolved_owned += 1;
                true // retain for inspection/retry
            }
        });
        if report.unresolved_owned == 0 {
            report.state = SessionState::Stopped;
            self.states
                .insert(session.to_owned(), SessionState::Stopped);
        }
        Ok(report)
    }
}
