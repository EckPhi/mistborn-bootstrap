use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InspectionStatus<T> {
    Available(T),
    Unavailable { reason: String },
    Error { message: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedState {
    pub facts: BTreeMap<String, InspectionStatus<serde_json::Value>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum DiagnosticSeverity {
    Pass,
    Warn,
    Fail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub id: String,
    pub severity: DiagnosticSeverity,
    pub summary: String,
    pub evidence: Vec<String>,
    pub remediation_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskClass {
    Low,
    Moderate,
    Access,
    Destructive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationPolicy {
    None,
    Interactive,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposedAction {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remediation {
    pub id: String,
    pub risk: RiskClass,
    pub confirmation: ConfirmationPolicy,
    pub dependencies: Vec<String>,
    pub actions: Vec<ProposedAction>,
    pub verification: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReconciliationPlan {
    pub snapshot_id: String,
    pub remediations: Vec<Remediation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionOutcome {
    Applied,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_id: String,
    pub outcome: ActionOutcome,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyResult {
    pub remediation_id: String,
    pub started_at: u64,
    pub finished_at: u64,
    pub actions: Vec<ActionResult>,
    pub verified: bool,
    pub verification_evidence: Vec<String>,
}

pub trait CommandAdapter {
    fn run(&self, program: &str, arguments: &[&str]) -> Result<CommandOutput, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub trait FileAdapter {
    fn read(&self, path: &std::path::Path) -> Result<Vec<u8>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inspection_errors_remain_distinct_from_unavailable_facts() {
        let unavailable: InspectionStatus<()> = InspectionStatus::Unavailable {
            reason: "command missing".to_owned(),
        };
        let error: InspectionStatus<()> = InspectionStatus::Error {
            message: "malformed output".to_owned(),
        };
        assert_ne!(unavailable, error);
    }
}
