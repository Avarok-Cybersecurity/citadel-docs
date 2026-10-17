//! `bench/plan.toml`: how many warmup and timed iterations each metric runs, ours keyed by
//! metric name and competitors' by `tool/metric`. No defaults: a key without an entry stops the
//! run naming the key.

use crate::stats::Iterations;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Why the plan could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// The file is not a plan.
    #[error("bench/plan.toml: {0}")]
    Parse(String),
    /// A key has no entry.
    #[error("no plan for {0}: add it to bench/plan.toml")]
    Missing(String),
    /// An entry times nothing.
    #[error("plan for {0} has zero iterations")]
    Zero(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    plan: BTreeMap<String, Iterations>,
}

/// The parsed plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    entries: BTreeMap<String, Iterations>,
}

impl Plan {
    /// Parses `bench/plan.toml`.
    pub fn parse(text: &str) -> Result<Self, PlanError> {
        let file: File = toml::from_str(text).map_err(|e| PlanError::Parse(e.to_string()))?;
        if let Some((key, _)) = file.plan.iter().find(|(_, it)| it.iterations == 0) {
            return Err(PlanError::Zero(key.clone()));
        }
        Ok(Self { entries: file.plan })
    }

    /// The iterations for `key`.
    pub fn get(&self, key: &str) -> Result<Iterations, PlanError> {
        self.entries
            .get(key)
            .copied()
            .ok_or_else(|| PlanError::Missing(key.to_owned()))
    }
}
