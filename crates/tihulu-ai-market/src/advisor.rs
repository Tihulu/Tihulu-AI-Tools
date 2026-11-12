//! Experimental AI matching is an untrusted, cited proposal, never an action.
use crate::manifest::{ModelManifest, TaskKind};
use crate::matching::{match_model, CompatibilityContext, MatchResult, TaskRequest};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Citation {
    pub title: String,
    pub url: String,
    pub supported_claim: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorProposal {
    pub model_id: String,
    pub task: TaskKind,
    pub explanation: String,
    /// Informational model confidence; never upgrades deterministic readiness.
    pub confidence_pct: u8,
    pub citations: Vec<Citation>,
    /// Indicates whether the advisor relied on external Internet research.
    pub internet_used: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdvisorPolicy {
    /// Internet is OFF until an explicit per-request opt-in.
    pub internet_opt_in: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProposalError {
    MissingModel,
    WrongTask,
    InternetNotAuthorized,
    InvalidConfidence,
    MissingExplanation,
    MissingCitations,
    InvalidCitation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdvisorValidation {
    pub deterministic_match: MatchResult,
    pub advisor_confidence_pct: u8,
    /// Always false. Assignment, model tests and downloads require other
    /// explicitly authorized components and their own revalidation.
    pub executable: bool,
}

/// Validates only metadata and cites. Does no HTTP, file, model, or runtime IO.
/// A citation URL is a string, not proof that its claims were fact-checked.
pub fn validate_advisor_proposal(
    proposal: &AdvisorProposal,
    policy: &AdvisorPolicy,
    manifests: &[ModelManifest],
    task: &TaskRequest,
    context: &CompatibilityContext,
) -> Result<AdvisorValidation, ProposalError> {
    if proposal.internet_used && !policy.internet_opt_in {
        return Err(ProposalError::InternetNotAuthorized);
    }
    if proposal.confidence_pct > 100 {
        return Err(ProposalError::InvalidConfidence);
    }
    if proposal.task != task.task {
        return Err(ProposalError::WrongTask);
    }
    if proposal.explanation.trim().is_empty() {
        return Err(ProposalError::MissingExplanation);
    }
    if proposal.citations.is_empty() {
        return Err(ProposalError::MissingCitations);
    }
    if proposal.citations.iter().any(|citation| {
        citation.title.trim().is_empty()
            || citation.supported_claim.trim().is_empty()
            || !citation.url.starts_with("https://")
            || citation.url.chars().any(char::is_whitespace)
    }) {
        return Err(ProposalError::InvalidCitation);
    }
    let model = manifests
        .iter()
        .find(|m| m.id == proposal.model_id)
        .ok_or(ProposalError::MissingModel)?;
    Ok(AdvisorValidation {
        deterministic_match: match_model(model, task, context),
        advisor_confidence_pct: proposal.confidence_pct,
        executable: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_is_opt_in_by_default() {
        assert!(!AdvisorPolicy::default().internet_opt_in);
    }
}
