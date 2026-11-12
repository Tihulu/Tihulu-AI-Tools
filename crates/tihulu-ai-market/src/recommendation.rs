//! Read-only application task -> Registry model -> Market recommendation flow.
//!
//! No runtime leases, model downloads, filesystem access, or app config writes.
//! Only an authorized adapter can populate trusted Registry/probe snapshots.

use crate::manifest::{ModelManifest, ModelStatus};
use crate::matching::MatchVerdict;
use crate::registry_contract::{
    match_registry_model, ApplicationTask, RegistryMatchDecision, RegistryMatchRequest,
    RegistryModelSnapshot, RuntimeTarget, TrustedHardwareProbe,
};

pub struct RegistryCandidate<'a> {
    pub manifest: &'a ModelManifest,
    pub registry: &'a RegistryModelSnapshot,
    pub target: &'a RuntimeTarget,
    pub probe: &'a TrustedHardwareProbe,
}

fn verdict_order(verdict: MatchVerdict) -> u8 {
    match verdict {
        MatchVerdict::Recommended => 0,
        MatchVerdict::Candidate => 1,
        MatchVerdict::Unknown => 2,
        MatchVerdict::Incompatible => 3,
    }
}

fn inventory_order(status: ModelStatus) -> u8 {
    match status {
        ModelStatus::Installed => 0,
        ModelStatus::External => 1,
        ModelStatus::Unverified => 2,
        ModelStatus::Candidate => 3,
        ModelStatus::Downloadable => 4,
        ModelStatus::Unknown => 5,
        ModelStatus::Incompatible => 6,
    }
}

/// Evaluate only pre-approved, Registry-bound candidates for an application.
/// A known incompatible installed file never outranks a working external model.
/// Sorting is stable across input order; no backend is started or kept warm.
/// Returned recommendations remain read-only and need user approval.
pub fn recommend_registered_models(
    application_task: &ApplicationTask,
    candidates: &[RegistryCandidate<'_>],
) -> Vec<RegistryMatchDecision> {
    let mut results: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            let request = RegistryMatchRequest {
                catalog_model_id: candidate.manifest.id.clone(),
                registry_model_id: candidate.registry.id.clone(),
                application_task: application_task.clone(),
                target: candidate.target.clone(),
            };
            match_registry_model(
                candidate.manifest,
                candidate.registry,
                &request,
                candidate.probe,
            )
        })
        .collect();
    results.sort_by(|a, b| {
        verdict_order(a.compatibility.verdict)
            .cmp(&verdict_order(b.compatibility.verdict))
            .then_with(|| {
                inventory_order(a.inventory_status).cmp(&inventory_order(b.inventory_status))
            })
            .then_with(|| a.catalog_model_id.cmp(&b.catalog_model_id))
            .then_with(|| a.target.host_id.as_str().cmp(b.target.host_id.as_str()))
    });
    results
}
