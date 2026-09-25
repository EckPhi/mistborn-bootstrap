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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk: Option<RiskClass>,
    #[serde(default)]
    pub confirmation_required: bool,
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
    pub kind: ActionKind,
    pub description: String,
}

/// Operations the reconciliation engine is allowed to request. This is deliberately
/// closed: plans cannot encode a program name, shell fragment, or arbitrary arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionKind {
    InstallPackage { package: PackageName },
    EnableService { service: ServiceName },
    RestartService { service: ServiceName },
    ApplyBoundedRemediation { remediation: RemediationId },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemediationId {
    SecurityUfw,
    SecurityPlexFirewall,
    SecurityFail2ban,
    SecurityFail2banPolicy,
    SecuritySsh,
    SecurityTailscaleSsh,
    SecurityTailscaleExitNode,
    SecurityTailscaleAutoUpdate,
    PackagesDocker,
    PackagesTailscale,
    PackagesUfw,
    PackagesFail2banClient,
}

impl RemediationId {
    pub const ALL: [Self; 12] = [
        Self::SecurityUfw,
        Self::SecurityPlexFirewall,
        Self::SecurityFail2ban,
        Self::SecurityFail2banPolicy,
        Self::SecuritySsh,
        Self::SecurityTailscaleSsh,
        Self::SecurityTailscaleExitNode,
        Self::SecurityTailscaleAutoUpdate,
        Self::PackagesDocker,
        Self::PackagesTailscale,
        Self::PackagesUfw,
        Self::PackagesFail2banClient,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SecurityUfw => "security/ufw",
            Self::SecurityPlexFirewall => "security/plex-firewall",
            Self::SecurityFail2ban => "security/fail2ban",
            Self::SecurityFail2banPolicy => "security/fail2ban-policy",
            Self::SecuritySsh => "security/ssh",
            Self::SecurityTailscaleSsh => "security/tailscale-ssh",
            Self::SecurityTailscaleExitNode => "security/tailscale-exit-node",
            Self::SecurityTailscaleAutoUpdate => "security/tailscale-auto-update",
            Self::PackagesDocker => "packages/docker",
            Self::PackagesTailscale => "packages/tailscale",
            Self::PackagesUfw => "packages/ufw",
            Self::PackagesFail2banClient => "packages/fail2ban-client",
        }
    }
}

impl std::str::FromStr for RemediationId {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|id| id.as_str() == value)
            .ok_or_else(|| format!("unknown remediation: {value}"))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackageName {
    Docker,
    Tailscale,
    Ufw,
    Fail2ban,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceName {
    Docker,
    Tailscaled,
    Ufw,
    Fail2ban,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remediation {
    pub id: String,
    pub risk: RiskClass,
    pub confirmation: ConfirmationPolicy,
    pub dependencies: Vec<String>,
    pub actions: Vec<ProposedAction>,
    pub verification: String,
    #[serde(default)]
    pub available: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable_reason: Option<String>,
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
