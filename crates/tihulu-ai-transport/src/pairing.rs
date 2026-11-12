//! Paired-host control-plane contract. NO TLS handshake or key storage is implemented.
//! Only a future verified mTLS ingress and trusted user-approval broker may turn a
//! pairing proposal into an actual host approval. The fixtures here are synthetic.
use crate::protocol::{
    valid_sha256_hex, ClientId, HostId, PairingApproval, PairingAttemptId,
};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PairingError {
    InvalidFingerprint,
    InvalidLifetime,
    DuplicateAttempt,
    UnknownOrConsumed,
    Expired,
    Denied,
    MismatchedIdentity,
    UnauthenticatedReviewer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairingOffer {
    pub attempt: PairingAttemptId,
    pub host: HostId,
    pub client: ClientId,
    pub server_cert_sha256: String,
    pub client_cert_sha256: String,
    pub expires_at_ms: u64,
}

/// A future trusted approval broker must present BOTH certificate pins over a
/// separate user-verified channel. This public value is a MOCK input only:
/// a caller setting these flags does not cryptographically authenticate anyone.
#[derive(Clone, Debug)]
pub struct MockOutOfBandApproval {
    pub host: HostId,
    pub client: ClientId,
    pub observed_server_cert_sha256: String,
    pub observed_client_cert_sha256: String,
    pub trusted_reviewer_authenticated: bool,
    pub user_confirmed_both_fingerprints: bool,
}

/// No network sockets, QR transport, OS credential vault or TLS verifier.
/// Approved records are one-shot: an old pairing attempt cannot be replayed.
#[derive(Default)]
pub struct PairingBroker {
    pending: HashMap<PairingAttemptId, PairingOffer>,
    used_attempts: std::collections::HashSet<PairingAttemptId>,
}

impl PairingBroker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn propose(
        &mut self,
        offer: PairingOffer,
        now_ms: u64,
    ) -> Result<(), PairingError> {
        if !valid_sha256_hex(&offer.server_cert_sha256)
            || !valid_sha256_hex(&offer.client_cert_sha256)
        {
            return Err(PairingError::InvalidFingerprint);
        }
        if offer.expires_at_ms <= now_ms
            || offer.expires_at_ms > now_ms.saturating_add(120_000)
        {
            return Err(PairingError::InvalidLifetime);
        }
        if self.used_attempts.contains(&offer.attempt)
            || self.pending.contains_key(&offer.attempt)
        {
            return Err(PairingError::DuplicateAttempt);
        }
        self.used_attempts.insert(offer.attempt.clone());
        self.pending.insert(offer.attempt.clone(), offer);
        Ok(())
    }

    pub fn approve_mock(
        &mut self,
        attempt: &PairingAttemptId,
        proof: &MockOutOfBandApproval,
        now_ms: u64,
    ) -> Result<PairingApproval, PairingError> {
        let offer = self
            .pending
            .get(attempt)
            .ok_or(PairingError::UnknownOrConsumed)?;
        if now_ms >= offer.expires_at_ms {
            self.pending.remove(attempt);
            return Err(PairingError::Expired);
        }
        if !proof.trusted_reviewer_authenticated {
            return Err(PairingError::UnauthenticatedReviewer);
        }
        if !proof.user_confirmed_both_fingerprints {
            return Err(PairingError::Denied);
        }
        if offer.host != proof.host
            || offer.client != proof.client
            || offer.server_cert_sha256 != proof.observed_server_cert_sha256
            || offer.client_cert_sha256 != proof.observed_client_cert_sha256
        {
            return Err(PairingError::MismatchedIdentity);
        }
        let offer = self
            .pending
            .remove(attempt)
            .ok_or(PairingError::UnknownOrConsumed)?;
        Ok(PairingApproval {
            client: offer.client,
            client_cert_sha256: offer.client_cert_sha256,
            user_approved: true,
        })
    }

    pub fn revoke_attempt(&mut self, attempt: &PairingAttemptId) -> bool {
        self.pending.remove(attempt).is_some()
    }

    pub fn expire(&mut self, now_ms: u64) {
        self.pending.retain(|_, offer| offer.expires_at_ms > now_ms);
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}
