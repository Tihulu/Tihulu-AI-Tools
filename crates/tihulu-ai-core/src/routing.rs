//! Pure policy logic for future local and paired-host inference routing.
//! No actual inference or remote connection is initiated by this module.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputePreference {
    Auto,
    LocalOnly,
    LocalFirst,
    RemoteFirst,
    RemoteOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeReadiness {
    /// The runtime has passed an actual compatibility test for the requested task.
    pub verified_compatible: bool,
    /// This particular client is explicitly authorized to use the runtime.
    pub authorized: bool,
    /// Runtime/host can currently accept requests.
    pub available: bool,
}

impl RuntimeReadiness {
    pub const fn ready(self) -> bool {
        self.verified_compatible && self.authorized && self.available
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Local,
    PairedHost,
    Unavailable,
}

/// Select a *candidate route* only after independent compatibility and permission
/// checks. Actual inference, transport and fallback remain responsibilities of
/// future runtime adapters, never the LLM's own suggestions.
pub fn choose_route(
    preference: ComputePreference,
    local: RuntimeReadiness,
    remote: RuntimeReadiness,
) -> Route {
    let local_ready = local.ready();
    let remote_ready = remote.ready();

    match preference {
        ComputePreference::Auto | ComputePreference::LocalFirst => {
            if local_ready {
                Route::Local
            } else if remote_ready {
                Route::PairedHost
            } else {
                Route::Unavailable
            }
        }
        ComputePreference::LocalOnly => {
            if local_ready {
                Route::Local
            } else {
                Route::Unavailable
            }
        }
        ComputePreference::RemoteFirst => {
            if remote_ready {
                Route::PairedHost
            } else if local_ready {
                Route::Local
            } else {
                Route::Unavailable
            }
        }
        ComputePreference::RemoteOnly => {
            if remote_ready {
                Route::PairedHost
            } else {
                Route::Unavailable
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const READY: RuntimeReadiness = RuntimeReadiness {
        verified_compatible: true,
        authorized: true,
        available: true,
    };
    const UNVERIFIED: RuntimeReadiness = RuntimeReadiness {
        verified_compatible: false,
        authorized: true,
        available: true,
    };
    const DENIED: RuntimeReadiness = RuntimeReadiness {
        verified_compatible: true,
        authorized: false,
        available: true,
    };

    #[test]
    fn defaults_local_when_both_are_ready() {
        assert_eq!(
            choose_route(ComputePreference::Auto, READY, READY),
            Route::Local
        );
    }

    #[test]
    fn remote_first_uses_host_when_verified() {
        assert_eq!(
            choose_route(ComputePreference::RemoteFirst, READY, READY),
            Route::PairedHost
        );
    }

    #[test]
    fn remote_only_never_falls_back_to_local() {
        assert_eq!(
            choose_route(ComputePreference::RemoteOnly, READY, DENIED),
            Route::Unavailable
        );
    }

    #[test]
    fn unverified_or_unauthorized_remote_is_not_selected() {
        assert_eq!(
            choose_route(ComputePreference::RemoteFirst, READY, UNVERIFIED),
            Route::Local
        );
        assert_eq!(
            choose_route(ComputePreference::RemoteFirst, READY, DENIED),
            Route::Local
        );
    }

    #[test]
    fn local_only_is_not_secret_remote_fallback() {
        assert_eq!(
            choose_route(ComputePreference::LocalOnly, UNVERIFIED, READY),
            Route::Unavailable
        );
    }
}
