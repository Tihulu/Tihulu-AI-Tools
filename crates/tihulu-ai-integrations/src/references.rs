//! Pure startup reference validation. This intentionally does not read/write
//! aipath.txt, resolve a path, contact Jan, or persist a token.
//! The schema below remains a proposal until consumer adapters are approved.

use std::collections::BTreeMap;

use crate::bridge::safe_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelReference {
    Managed(String),
    External { owner: String, model_id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    Auto,
    LocalFirst,
    RemoteFirst,
    RemoteOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartupReferences {
    pub app_id: String,
    pub model: Option<ModelReference>,
    pub paired_host_id: Option<String>,
    pub preference: Preference,
    pub fallback_legacy: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartupRoute {
    DirectVerifiedModel,
    PairedHost,
    LegacyFallback,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceError {
    InvalidLine,
    DuplicateKey,
    UnknownField,
    UnsupportedSchema,
    InvalidValue,
}

pub trait StartupAvailability {
    /// Verify runtime and access, not merely a file's existence.
    fn model_usable(&self, reference: &ModelReference) -> bool;
    /// Verify secure pairing, reachability and authorized host capability.
    fn paired_host_usable(&self, host_id: &str) -> bool;
}

impl StartupReferences {
    /// Accepts proposed v1/v2 fields, rejects all credential/command/path
    /// extensions. Nothing is resolved or persisted.
    pub fn parse_proposal(input: &str) -> Result<Self, ReferenceError> {
        let mut map = BTreeMap::new();
        for raw in input.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (key, value) = line.split_once('=').ok_or(ReferenceError::InvalidLine)?;
            let key = key.trim();
            let value = value.trim();
            if !matches!(
                key,
                "schema"
                    | "app_id"
                    | "manager_mode"
                    | "fallback"
                    | "remote_host_id"
                    | "compute_preference"
                    | "task.llm.model_ref"
                    | "task.llm.remote_host_id"
                    | "task.llm.compute"
            ) {
                return Err(ReferenceError::UnknownField);
            }
            if map.insert(key.to_owned(), value.to_owned()).is_some() {
                return Err(ReferenceError::DuplicateKey);
            }
        }

        let schema = map.get("schema").ok_or(ReferenceError::UnsupportedSchema)?;
        if schema != "1" && schema != "2-proposal" {
            return Err(ReferenceError::UnsupportedSchema);
        }
        let app_id = map.get("app_id").ok_or(ReferenceError::InvalidValue)?;
        if !safe_id(app_id) || map.get("manager_mode").map(String::as_str) != Some("optional") {
            return Err(ReferenceError::InvalidValue);
        }
        if map.get("fallback").map(String::as_str) != Some("legacy") {
            return Err(ReferenceError::InvalidValue);
        }
        let model = match map.get("task.llm.model_ref") {
            None => None,
            Some(value) => {
                if schema != "2-proposal" {
                    return Err(ReferenceError::InvalidValue);
                }
                let parts: Vec<_> = value.split(':').collect();
                match parts.as_slice() {
                    ["managed", id] if safe_id(id) => Some(ModelReference::Managed(id.to_string())),
                    ["external", owner, id] if safe_id(owner) && safe_id(id) => {
                        Some(ModelReference::External {
                            owner: owner.to_string(),
                            model_id: id.to_string(),
                        })
                    }
                    _ => return Err(ReferenceError::InvalidValue),
                }
            }
        };
        let host_id = map
            .get("task.llm.remote_host_id")
            .or_else(|| map.get("remote_host_id"))
            .cloned();
        if host_id.as_ref().is_some_and(|id| !safe_id(id)) {
            return Err(ReferenceError::InvalidValue);
        }
        let compute = map
            .get("task.llm.compute")
            .or_else(|| map.get("compute_preference"))
            .map(String::as_str)
            .unwrap_or("auto");
        let preference = match compute {
            "auto" => Preference::Auto,
            "local-first" => Preference::LocalFirst,
            "remote-first" => Preference::RemoteFirst,
            "remote-only" => Preference::RemoteOnly,
            _ => return Err(ReferenceError::InvalidValue),
        };
        if schema == "1"
            && (map.contains_key("task.llm.model_ref")
                || map.contains_key("task.llm.remote_host_id")
                || map.contains_key("task.llm.compute"))
        {
            return Err(ReferenceError::InvalidValue);
        }
        Ok(Self {
            app_id: app_id.to_owned(),
            model,
            paired_host_id: host_id,
            preference,
            fallback_legacy: true,
        })
    }

    /// Resolve only from externally verified registry/host facts. GUI presence
    /// is irrelevant; no implicit Jan start, no model copy, no remote token.
    pub fn choose_route(&self, availability: &impl StartupAvailability) -> StartupRoute {
        let local = self
            .model
            .as_ref()
            .is_some_and(|model| availability.model_usable(model));
        let remote = self
            .paired_host_id
            .as_ref()
            .is_some_and(|host| availability.paired_host_usable(host));
        match self.preference {
            Preference::RemoteOnly => {
                if remote {
                    StartupRoute::PairedHost
                } else {
                    StartupRoute::LegacyFallback
                }
            }
            Preference::RemoteFirst if remote => StartupRoute::PairedHost,
            Preference::Auto | Preference::LocalFirst | Preference::RemoteFirst if local => {
                StartupRoute::DirectVerifiedModel
            }
            _ if remote => StartupRoute::PairedHost,
            _ => StartupRoute::LegacyFallback,
        }
    }
}
