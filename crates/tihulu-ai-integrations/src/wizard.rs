//! Jan setup wizard state machine (mock metadata, NO Jan API calls).
//! Desktop MCP registration and optional Jan Agent plugin are separate.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JanProduct {
    Desktop,
    Agent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedJan {
    pub product: JanProduct,
    pub version: String,
    /// True only if this exact installed version has a verified safe command.
    pub verified_cli_supported: bool,
    pub existing_entry: bool,
    pub existing_entry_owned_by_us: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupMethod {
    VerifiedCli,
    GuidedManual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardState {
    NotConfigured,
    Detected,
    NeedsOwnerApproval,
    AwaitingManualConfiguration,
    ConfiguredDisabled,
    VerificationPending,
    ConnectedVerified,
    DisconnectPending,
    Disconnected,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WizardError {
    NoDetection,
    InvalidVersion,
    ExistingForeignEntry,
    PermissionRequired,
    InvalidTransition,
    NotActuallyVerified,
}

pub struct SetupWizard {
    state: WizardState,
    detection: Option<DetectedJan>,
    method: Option<SetupMethod>,
    owned_entry: bool,
}

impl Default for SetupWizard {
    fn default() -> Self {
        Self::new()
    }
}

impl SetupWizard {
    pub fn new() -> Self {
        Self {
            state: WizardState::NotConfigured,
            detection: None,
            method: None,
            owned_entry: false,
        }
    }

    pub fn state(&self) -> WizardState {
        self.state
    }

    pub fn detect(&mut self, detection: DetectedJan) -> Result<(), WizardError> {
        if detection.version.trim().is_empty() {
            return Err(WizardError::InvalidVersion);
        }
        if detection.existing_entry && !detection.existing_entry_owned_by_us {
            // Never overwrite/disconnect a third-party or ambiguous entry.
            return Err(WizardError::ExistingForeignEntry);
        }
        self.owned_entry = detection.existing_entry && detection.existing_entry_owned_by_us;
        self.detection = Some(detection);
        self.state = WizardState::Detected;
        Ok(())
    }

    /// Preview only. For Jan Agent plugin installation a *different* wizard
    /// and package would be needed: here we only plan MCP registration.
    pub fn review(&mut self) -> Result<SetupMethod, WizardError> {
        let detection = self.detection.as_ref().ok_or(WizardError::NoDetection)?;
        if self.state != WizardState::Detected {
            return Err(WizardError::InvalidTransition);
        }
        let method = if detection.verified_cli_supported {
            SetupMethod::VerifiedCli
        } else {
            SetupMethod::GuidedManual
        };
        self.method = Some(method);
        self.state = WizardState::NeedsOwnerApproval;
        Ok(method)
    }

    /// Pure transition after explicit approval. Does NOT modify Jan.
    pub fn owner_approved(&mut self, approved: bool) -> Result<SetupMethod, WizardError> {
        if self.state != WizardState::NeedsOwnerApproval {
            return Err(WizardError::InvalidTransition);
        }
        if !approved {
            self.state = WizardState::Detected;
            return Err(WizardError::PermissionRequired);
        }
        let method = self.method.ok_or(WizardError::InvalidTransition)?;
        self.state = match method {
            SetupMethod::VerifiedCli => WizardState::ConfiguredDisabled,
            SetupMethod::GuidedManual => WizardState::AwaitingManualConfiguration,
        };
        // CLI action isn't actually performed here; caller must register
        // externally then report its verified result before testing.
        Ok(method)
    }

    /// The trusted host reports a supported CLI registration/manual save.
    /// A false report never implies a connected Jan instance.
    pub fn record_configuration(&mut self, successful: bool, created_by_us: bool) -> Result<(), WizardError> {
        if !matches!(
            self.state,
            WizardState::ConfiguredDisabled | WizardState::AwaitingManualConfiguration
        ) {
            return Err(WizardError::InvalidTransition);
        }
        if !successful {
            self.state = WizardState::Failed;
            return Ok(());
        }
        self.owned_entry |= created_by_us;
        self.state = WizardState::VerificationPending;
        Ok(())
    }

    /// Require both a real MCP initialize/tools handshake and a successful
    /// owner-authorized read-only tool result (recorded by a future adapter).
    pub fn record_verification(
        &mut self,
        handshake_ok: bool,
        readonly_probe_ok: bool,
    ) -> Result<(), WizardError> {
        if self.state != WizardState::VerificationPending {
            return Err(WizardError::InvalidTransition);
        }
        if !handshake_ok || !readonly_probe_ok {
            self.state = WizardState::Failed;
            return Err(WizardError::NotActuallyVerified);
        }
        self.state = WizardState::ConnectedVerified;
        Ok(())
    }

    /// Only request removal of our own entry; the host must implement it
    /// through supported Jan methods and confirm before marking Disconnected.
    pub fn plan_disconnect(&mut self) -> Result<bool, WizardError> {
        if self.detection.is_none() {
            return Err(WizardError::NoDetection);
        }
        self.state = WizardState::DisconnectPending;
        Ok(self.owned_entry)
    }

    pub fn record_disconnect(&mut self, success: bool) -> Result<(), WizardError> {
        if self.state != WizardState::DisconnectPending {
            return Err(WizardError::InvalidTransition);
        }
        if success {
            self.owned_entry = false;
            self.state = WizardState::Disconnected;
        } else {
            self.state = WizardState::Failed;
        }
        Ok(())
    }
}
