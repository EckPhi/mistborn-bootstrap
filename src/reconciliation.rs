//! Deterministic host reconciliation planning and execution primitives.
//!
//! Actions are a closed enum and remediations come from this module's registry;
//! callers cannot smuggle arbitrary commands into a plan.

use crate::domain::{
    ActionKind, ActionOutcome, ActionResult, ApplyResult, ConfirmationPolicy, Diagnostic,
    DiagnosticSeverity, ProposedAction, ReconciliationPlan, Remediation, RemediationId, RiskClass,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy)]
struct Definition {
    id: RemediationId,
    risk: RiskClass,
    confirmation: ConfirmationPolicy,
    dependency: Option<RemediationId>,
    available: bool,
}

const REGISTRY: [Definition; 12] = [
    Definition {
        id: RemediationId::SecurityUfw,
        risk: RiskClass::Access,
        confirmation: ConfirmationPolicy::Explicit,
        dependency: Some(RemediationId::PackagesUfw),
        available: true,
    },
    Definition {
        id: RemediationId::SecurityPlexFirewall,
        risk: RiskClass::Access,
        confirmation: ConfirmationPolicy::Explicit,
        dependency: Some(RemediationId::SecurityUfw),
        available: true,
    },
    Definition {
        id: RemediationId::SecurityFail2ban,
        risk: RiskClass::Low,
        confirmation: ConfirmationPolicy::None,
        dependency: Some(RemediationId::PackagesFail2banClient),
        available: true,
    },
    Definition {
        id: RemediationId::SecurityFail2banPolicy,
        risk: RiskClass::Access,
        confirmation: ConfirmationPolicy::Explicit,
        dependency: Some(RemediationId::SecurityFail2ban),
        available: true,
    },
    Definition {
        id: RemediationId::SecuritySsh,
        risk: RiskClass::Access,
        confirmation: ConfirmationPolicy::Explicit,
        dependency: None,
        available: false,
    },
    Definition {
        id: RemediationId::SecurityTailscaleSsh,
        risk: RiskClass::Access,
        confirmation: ConfirmationPolicy::Explicit,
        dependency: Some(RemediationId::PackagesTailscale),
        available: true,
    },
    Definition {
        id: RemediationId::SecurityTailscaleExitNode,
        risk: RiskClass::Access,
        confirmation: ConfirmationPolicy::Explicit,
        dependency: Some(RemediationId::PackagesTailscale),
        available: true,
    },
    Definition {
        id: RemediationId::SecurityTailscaleAutoUpdate,
        risk: RiskClass::Moderate,
        confirmation: ConfirmationPolicy::None,
        dependency: Some(RemediationId::PackagesTailscale),
        available: true,
    },
    Definition {
        id: RemediationId::PackagesDocker,
        risk: RiskClass::Low,
        confirmation: ConfirmationPolicy::None,
        dependency: None,
        available: true,
    },
    Definition {
        id: RemediationId::PackagesTailscale,
        risk: RiskClass::Low,
        confirmation: ConfirmationPolicy::None,
        dependency: None,
        available: true,
    },
    Definition {
        id: RemediationId::PackagesUfw,
        risk: RiskClass::Low,
        confirmation: ConfirmationPolicy::None,
        dependency: None,
        available: true,
    },
    Definition {
        id: RemediationId::PackagesFail2banClient,
        risk: RiskClass::Low,
        confirmation: ConfirmationPolicy::None,
        dependency: None,
        available: true,
    },
];

fn definition(id: RemediationId) -> Definition {
    *REGISTRY
        .iter()
        .find(|item| item.id == id)
        .expect("registry is exhaustive")
}

fn action_for(id: RemediationId) -> ActionKind {
    use crate::domain::{PackageName, ServiceName};
    match id {
        RemediationId::PackagesDocker => ActionKind::InstallPackage {
            package: PackageName::Docker,
        },
        RemediationId::PackagesTailscale => ActionKind::InstallPackage {
            package: PackageName::Tailscale,
        },
        RemediationId::PackagesUfw => ActionKind::InstallPackage {
            package: PackageName::Ufw,
        },
        RemediationId::PackagesFail2banClient => ActionKind::InstallPackage {
            package: PackageName::Fail2ban,
        },
        RemediationId::SecurityFail2ban => ActionKind::EnableService {
            service: ServiceName::Fail2ban,
        },
        RemediationId::SecurityFail2banPolicy => {
            ActionKind::ApplyBoundedRemediation { remediation: id }
        }
        other => ActionKind::ApplyBoundedRemediation { remediation: other },
    }
}

/// Build the same deterministic host plan from a diagnostic set. PASS diagnostics
/// and unregistered IDs never produce actions.
pub fn plan(
    snapshot: &serde_json::Value,
    diagnostics: &[Diagnostic],
    target: Option<RemediationId>,
) -> Result<ReconciliationPlan, String> {
    let mut drifting_ids = BTreeSet::new();
    for diagnostic in diagnostics {
        if diagnostic.severity == DiagnosticSeverity::Pass {
            continue;
        }
        if let Some(value) = diagnostic.remediation_id.as_deref() {
            let id = value.parse::<RemediationId>()?;
            drifting_ids.insert(id);
        }
    }
    let ids = if let Some(wanted) = target {
        let mut selected = BTreeSet::new();
        let mut pending = vec![wanted];
        while let Some(id) = pending.pop() {
            if drifting_ids.contains(&id) {
                selected.insert(id);
            }
            if let Some(dependency) = definition(id)
                .dependency
                .filter(|dependency| drifting_ids.contains(dependency))
            {
                if selected.insert(dependency) {
                    pending.push(dependency);
                }
            }
        }
        selected
    } else {
        drifting_ids
    };
    // Include only dependencies that are actually out of compliance. A dependency
    // absent from the drift set is already satisfied and needs no action.
    let mut ordered = Vec::new();
    let mut temporary = BTreeSet::new();
    let mut permanent = BTreeSet::new();
    fn visit(
        id: RemediationId,
        ids: &BTreeSet<RemediationId>,
        temporary: &mut BTreeSet<RemediationId>,
        permanent: &mut BTreeSet<RemediationId>,
        ordered: &mut Vec<RemediationId>,
    ) -> Result<(), String> {
        if permanent.contains(&id) {
            return Ok(());
        }
        if !temporary.insert(id) {
            return Err("remediation dependency cycle".to_owned());
        }
        if let Some(dependency) = definition(id)
            .dependency
            .filter(|dependency| ids.contains(dependency))
        {
            visit(dependency, ids, temporary, permanent, ordered)?;
        }
        temporary.remove(&id);
        permanent.insert(id);
        ordered.push(id);
        Ok(())
    }
    for id in ids.iter().copied() {
        visit(id, &ids, &mut temporary, &mut permanent, &mut ordered)?;
    }
    let remediations = ordered
        .into_iter()
        .map(|id| {
            let def = definition(id);
            Remediation {
                id: id.as_str().to_owned(),
                risk: def.risk,
                confirmation: def.confirmation,
                dependencies: def
                    .dependency
                    .into_iter()
                    .filter(|dependency| ids.contains(dependency))
                    .map(|dependency| dependency.as_str().to_owned())
                    .collect(),
                actions: vec![ProposedAction {
                    kind: action_for(id),
                    description: action_description(id),
                }],
                verification: format!("re-run inspectors and verify {} is compliant", id.as_str()),
                available: def.available,
                unavailable_reason: (!def.available)
                    .then(|| "no host adapter is registered in this release".to_owned()),
            }
        })
        .collect();
    let bytes = serde_json::to_vec(&serde_json::json!({
        "observed": stable_observed_for_fingerprint(snapshot),
        "diagnostics": diagnostics
    }))
    .map_err(|error| error.to_string())?;
    let hash = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
    });
    Ok(ReconciliationPlan {
        snapshot_id: format!("fnv1a64-{hash:016x}"),
        remediations,
    })
}

fn stable_observed_for_fingerprint(snapshot: &serde_json::Value) -> serde_json::Value {
    let mut stable = snapshot.clone();
    if let Some(facts) = stable
        .get_mut("facts")
        .and_then(serde_json::Value::as_object_mut)
    {
        // Peer lists, connection timestamps, and endpoint changes are unrelated
        // to the Tailscale preferences comparator and can churn during preview.
        facts.remove("tailscale.status");
    }
    stable
}

fn action_description(id: RemediationId) -> String {
    match id {
        RemediationId::PackagesDocker => "install the Docker package".into(),
        RemediationId::PackagesTailscale => "install the Tailscale package".into(),
        RemediationId::PackagesUfw => "install the UFW package".into(),
        RemediationId::PackagesFail2banClient => "install fail2ban".into(),
        RemediationId::SecurityFail2ban => "enable fail2ban service".into(),
        RemediationId::SecurityFail2banPolicy => "configure the managed fail2ban sshd jail".into(),
        RemediationId::SecurityTailscaleSsh => "set Tailscale SSH preference".into(),
        RemediationId::SecurityTailscaleExitNode => {
            "set Tailscale exit-node advertisement preference".into()
        }
        RemediationId::SecurityTailscaleAutoUpdate => "set Tailscale auto-update preference".into(),
        RemediationId::SecurityUfw => {
            "apply desired UFW policy and add missing managed rules without deleting rules".into()
        }
        RemediationId::SecurityPlexFirewall => {
            "install the owned Plex UFW profile and add its scoped rules".into()
        }
        _ => format!("apply bounded remediation {}", id.as_str()),
    }
}

#[derive(Debug, Clone)]
pub struct InspectionSnapshot {
    pub observed: serde_json::Value,
    pub diagnostics: Vec<Diagnostic>,
}

pub trait ReconcileAdapter {
    /// Take a fresh structured snapshot. No mutation is permitted here.
    fn inspect(&mut self) -> Result<InspectionSnapshot, String>;
    fn apply(&mut self, action: &ActionKind) -> Result<(), String>;
    /// Return true only when fresh inspection confirms desired state.
    fn verify(&mut self, id: RemediationId) -> Result<(bool, Vec<String>), String>;
}

#[derive(Debug, Clone, Default)]
pub struct ApplyOptions {
    /// `--safe`: only registry entries explicitly allowed as low risk.
    pub safe: bool,
    /// IDs approved by an interactive per-remediation prompt or an explicit target.
    pub confirmed: BTreeSet<RemediationId>,
    pub non_interactive: bool,
}

fn explicit_confirmation_missing(id: RemediationId, options: &ApplyOptions) -> bool {
    definition(id).confirmation == ConfirmationPolicy::Explicit && !options.confirmed.contains(&id)
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct History {
    events: Vec<HistoryEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryEvent {
    at: u64,
    remediation_id: String,
    outcome: String,
    detail: Option<String>,
}

/// Revalidate a proposed plan under an advisory process lock, then execute and
/// verify each remediation. History is separate from desired configuration and
/// atomically replaced after every event.
pub fn reconcile(
    plan: &ReconciliationPlan,
    _diagnostics: &[Diagnostic],
    adapter: &mut impl ReconcileAdapter,
    history_path: &Path,
    options: &ApplyOptions,
) -> Result<Vec<ApplyResult>, String> {
    if let Some(unavailable) = plan
        .remediations
        .iter()
        .find(|item| !item.available && !(options.safe && item.risk != RiskClass::Low))
    {
        return Err(format!(
            "{} is unavailable: {}",
            unavailable.id,
            unavailable
                .unavailable_reason
                .as_deref()
                .unwrap_or("no adapter")
        ));
    }
    let _lock = ReconcileLock::acquire(&sibling_lock_path(history_path))?;
    let fresh = adapter.inspect()?;
    let fresh_all = self::plan(&fresh.observed, &fresh.diagnostics, None)?;
    if fresh_all.snapshot_id != plan.snapshot_id {
        return Err("stale reconciliation plan; re-inspection changed observed or desired state, review and approve the new plan".to_owned());
    }
    let selected_ids = plan
        .remediations
        .iter()
        .map(|remediation| remediation.id.as_str())
        .collect::<BTreeSet<_>>();
    let selected_diagnostics = fresh
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .remediation_id
                .as_deref()
                .is_some_and(|id| selected_ids.contains(id))
        })
        .cloned()
        .collect::<Vec<_>>();
    let fresh_plan = self::plan(&fresh.observed, &selected_diagnostics, None)?;
    if let Some(unavailable) = fresh_plan
        .remediations
        .iter()
        .find(|item| !item.available && !(options.safe && item.risk != RiskClass::Low))
    {
        return Err(format!(
            "{} is unavailable: {}",
            unavailable.id,
            unavailable
                .unavailable_reason
                .as_deref()
                .unwrap_or("no adapter")
        ));
    }
    let mut history = read_history(history_path)?;
    let mut results = Vec::new();
    for remediation in &fresh_plan.remediations {
        let id: RemediationId = remediation.id.parse()?;
        let def = definition(id);
        if options.safe && (def.risk != RiskClass::Low || !safe_allowlisted(id)) {
            continue;
        }
        if explicit_confirmation_missing(id, options) {
            let reason = if options.non_interactive {
                "confirmation required (non-interactive)"
            } else {
                "explicit per-remediation confirmation required"
            };
            append_event(
                history_path,
                &mut history,
                id,
                "confirmation_required",
                Some(reason.to_owned()),
            )?;
            return Err(format!("{}: {reason}", id.as_str()));
        }
        let started = epoch_seconds();
        append_event(
            history_path,
            &mut history,
            id,
            "started",
            Some(format!("{} action sequence started", id.as_str())),
        )?;
        let mut action_results = Vec::new();
        let mut failed = None;
        for action in &remediation.actions {
            match adapter.apply(&action.kind) {
                Ok(()) => action_results.push(ActionResult {
                    action_id: id.as_str().to_owned(),
                    outcome: ActionOutcome::Applied,
                    message: None,
                }),
                Err(error) => {
                    failed = Some(error.clone());
                    action_results.push(ActionResult {
                        action_id: id.as_str().to_owned(),
                        outcome: ActionOutcome::Failed,
                        message: Some(error),
                    });
                    break;
                }
            }
        }
        let verification = if failed.is_none() {
            adapter.verify(id)
        } else {
            Ok((false, Vec::new()))
        };
        let (verified, evidence, verification_error) = match verification {
            Ok((verified, evidence)) => (verified, evidence, None),
            Err(error) => (
                false,
                Vec::new(),
                Some(format!("verification inspection failed: {error}")),
            ),
        };
        let success = failed.is_none() && verification_error.is_none() && verified;
        let detail = failed
            .clone()
            .or(verification_error)
            .or_else(|| (!verified).then(|| "verification did not match desired state".to_owned()));
        append_event(
            history_path,
            &mut history,
            id,
            if success { "verified" } else { "failed" },
            detail.clone(),
        )?;
        results.push(ApplyResult {
            remediation_id: id.as_str().to_owned(),
            started_at: started,
            finished_at: epoch_seconds(),
            actions: action_results,
            verified,
            verification_evidence: evidence,
        });
        if !success {
            return Err(format!(
                "{} failed: {}",
                id.as_str(),
                detail.unwrap_or_else(|| "verification failed".to_owned())
            ));
        }
    }
    Ok(results)
}

fn safe_allowlisted(id: RemediationId) -> bool {
    matches!(
        id,
        RemediationId::PackagesDocker
            | RemediationId::PackagesTailscale
            | RemediationId::PackagesUfw
            | RemediationId::PackagesFail2banClient
            | RemediationId::SecurityFail2ban
    )
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn sibling_lock_path(path: &Path) -> PathBuf {
    path.with_extension("lock")
}

#[cfg(unix)]
struct ReconcileLock(File);
#[cfg(unix)]
impl ReconcileLock {
    fn acquire(path: &Path) -> Result<Self, String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(path)
            .map_err(|error| error.to_string())?;
        unsafe extern "C" {
            fn flock(fd: i32, operation: i32) -> i32;
        }
        // LOCK_EX | LOCK_NB. Kernel releases this lock after a crash, unlike a
        // create-new sentinel file which can strand reconciliation indefinitely.
        let result = unsafe { flock(std::os::fd::AsRawFd::as_raw_fd(&file), 2 | 4) };
        if result != 0 {
            return Err("another reconciliation is running".to_owned());
        }
        Ok(Self(file))
    }
}
#[cfg(unix)]
impl Drop for ReconcileLock {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn flock(fd: i32, operation: i32) -> i32;
        }
        let _ = unsafe { flock(std::os::fd::AsRawFd::as_raw_fd(&self.0), 8) };
    }
}
#[cfg(not(unix))]
struct ReconcileLock;
#[cfg(not(unix))]
impl ReconcileLock {
    fn acquire(_: &Path) -> Result<Self, String> {
        Err("reconciliation locking is unsupported on this platform".into())
    }
}

fn read_history(path: &Path) -> Result<History, String> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|error| format!("invalid reconciliation history: {error}")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(History::default()),
        Err(error) => Err(error.to_string()),
    }
}

fn append_event(
    path: &Path,
    history: &mut History,
    id: RemediationId,
    outcome: &str,
    detail: Option<String>,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    history.events.push(HistoryEvent {
        at: epoch_seconds(),
        remediation_id: id.as_str().to_owned(),
        outcome: outcome.to_owned(),
        detail,
    });
    let bytes = serde_json::to_vec_pretty(history).map_err(|error| error.to_string())?;
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| error.to_string())?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        error.to_string()
    })?;
    if let Some(parent) = path.parent() {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Diagnostic;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    fn temp_file() -> PathBuf {
        std::env::temp_dir().join(format!(
            "mistborn-reconcile-{}-{}.json",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }
    fn drift(id: &str, severity: DiagnosticSeverity) -> Diagnostic {
        Diagnostic {
            id: id.into(),
            severity,
            summary: id.into(),
            evidence: vec![],
            remediation_id: Some(id.into()),
            risk: Some(RiskClass::Access),
            confirmation_required: true,
        }
    }
    #[derive(Default)]
    struct Fake {
        snapshot: serde_json::Value,
        diagnostics: Vec<Diagnostic>,
        applied: Vec<ActionKind>,
        fail_apply: bool,
        verified: bool,
        verify_error: bool,
    }
    impl ReconcileAdapter for Fake {
        fn inspect(&mut self) -> Result<InspectionSnapshot, String> {
            Ok(InspectionSnapshot {
                observed: self.snapshot.clone(),
                diagnostics: self.diagnostics.clone(),
            })
        }
        fn apply(&mut self, action: &ActionKind) -> Result<(), String> {
            self.applied.push(action.clone());
            if self.fail_apply {
                Err("adapter failure".into())
            } else {
                Ok(())
            }
        }
        fn verify(&mut self, _: RemediationId) -> Result<(bool, Vec<String>), String> {
            if self.verify_error {
                Err("inspector unavailable".into())
            } else {
                Ok((self.verified, vec!["fixture".into()]))
            }
        }
    }

    #[test]
    fn plan_is_deterministic_and_orders_failed_dependencies() {
        let snapshot = serde_json::json!({"facts": {"ufw": false, "plex": false}});
        let diagnostics = vec![
            drift("security/plex-firewall", DiagnosticSeverity::Fail),
            drift("security/ufw", DiagnosticSeverity::Fail),
            drift("packages/ufw", DiagnosticSeverity::Fail),
        ];
        let first = plan(&snapshot, &diagnostics, None).unwrap();
        let second = plan(&snapshot, &diagnostics, None).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first
                .remediations
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["packages/ufw", "security/ufw", "security/plex-firewall"]
        );
        assert_eq!(first.remediations[1].dependencies, vec!["packages/ufw"]);
        assert_eq!(first.remediations[2].dependencies, vec!["security/ufw"]);
        assert!(first.remediations.iter().all(|item| item.available));
    }

    #[test]
    fn plan_fingerprint_binds_full_desired_config_when_drift_is_unchanged() {
        let diagnostics = vec![drift("security/ssh", DiagnosticSeverity::Fail)];
        let current = serde_json::json!({"desired_config": {"loaded": true, "config": {"ssh": {"password_authentication": false}}}});
        let changed = serde_json::json!({"desired_config": {"loaded": true, "config": {"ssh": {"password_authentication": true}}}});
        let current_plan = plan(&current, &diagnostics, None).unwrap();
        let changed_plan = plan(&changed, &diagnostics, None).unwrap();
        assert_ne!(current_plan.snapshot_id, changed_plan.snapshot_id);
        assert_eq!(
            current_plan.remediations[0].id,
            changed_plan.remediations[0].id
        );
    }

    #[test]
    fn unrelated_tailscale_peer_status_does_not_stale_another_plan() {
        let diagnostics = vec![drift("packages/docker", DiagnosticSeverity::Fail)];
        let before = serde_json::json!({"facts": {
            "desired_config": {"loaded": true, "config": {"profile": "vps"}},
            "tailscale.status": {"Available": {"Peer": {"100.64.0.1": {"Online": true}}}},
            "package.docker": {"Available": {"installed": false}}
        }});
        let after = serde_json::json!({"facts": {
            "desired_config": {"loaded": true, "config": {"profile": "vps"}},
            "tailscale.status": {"Available": {"Peer": {"100.64.0.9": {"Online": false}}}},
            "package.docker": {"Available": {"installed": false}}
        }});
        assert_eq!(
            plan(&before, &diagnostics, None).unwrap().snapshot_id,
            plan(&after, &diagnostics, None).unwrap().snapshot_id
        );
    }

    #[test]
    fn safe_mode_never_selects_access_even_when_target_is_confirmed() {
        let snapshot = serde_json::json!({"ufw": false});
        let diagnostics = vec![drift("security/ufw", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            verified: true,
            ..Fake::default()
        };
        let path = temp_file();
        let options = ApplyOptions {
            safe: true,
            confirmed: [RemediationId::SecurityUfw].into(),
            non_interactive: true,
        };
        assert!(
            reconcile(&proposed, &diagnostics, &mut fake, &path, &options)
                .unwrap()
                .is_empty()
        );
        assert!(fake.applied.is_empty());
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn safe_mode_applies_allowlisted_low_risk_package_actions() {
        let snapshot = serde_json::json!({"docker": false});
        let diagnostics = vec![drift("packages/docker", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            verified: true,
            ..Fake::default()
        };
        let path = temp_file();
        let results = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &path,
            &ApplyOptions {
                safe: true,
                non_interactive: true,
                ..ApplyOptions::default()
            },
        )
        .unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].verified);
        assert_eq!(
            fake.applied,
            vec![ActionKind::InstallPackage {
                package: crate::domain::PackageName::Docker
            }]
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn access_confirmation_is_required_even_for_non_interactive_yes() {
        let options = ApplyOptions {
            non_interactive: true,
            ..ApplyOptions::default()
        };
        assert!(explicit_confirmation_missing(
            RemediationId::SecuritySsh,
            &options
        ));
        let approved = ApplyOptions {
            confirmed: [RemediationId::SecuritySsh].into(),
            ..options
        };
        assert!(!explicit_confirmation_missing(
            RemediationId::SecuritySsh,
            &approved
        ));
    }

    #[test]
    fn fail2ban_policy_is_confirmation_gated_and_never_safe_allowlisted() {
        let id = RemediationId::SecurityFail2banPolicy;
        assert_eq!(definition(id).risk, RiskClass::Access);
        assert_eq!(definition(id).confirmation, ConfirmationPolicy::Explicit);
        assert!(!safe_allowlisted(id));
        let diagnostics = vec![drift(id.as_str(), DiagnosticSeverity::Fail)];
        let plan = plan(&serde_json::json!({}), &diagnostics, Some(id)).unwrap();
        assert_eq!(plan.remediations[0].id, id.as_str());
        assert!(
            matches!(plan.remediations[0].actions[0].kind, ActionKind::ApplyBoundedRemediation { remediation } if remediation == id)
        );
    }

    #[test]
    fn verification_failure_is_recorded_and_never_reported_as_success() {
        let snapshot = serde_json::json!({"package": false});
        let diagnostics = vec![drift("packages/docker", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            verified: false,
            ..Fake::default()
        };
        let path = temp_file();
        let error = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &path,
            &ApplyOptions {
                safe: true,
                ..ApplyOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("verification did not match"));
        assert_eq!(
            read_history(&path).unwrap().events.last().unwrap().outcome,
            "failed"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn verifier_errors_are_recorded_as_failed_events() {
        let snapshot = serde_json::json!({"package": false});
        let diagnostics = vec![drift("packages/docker", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            verify_error: true,
            ..Fake::default()
        };
        let path = temp_file();
        let error = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &path,
            &ApplyOptions {
                safe: true,
                ..ApplyOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("verification inspection failed"));
        assert_eq!(
            read_history(&path).unwrap().events.last().unwrap().outcome,
            "failed"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn dependency_failure_stops_following_remediations() {
        let snapshot = serde_json::json!({"fail2ban_package": false, "fail2ban_service": false});
        let diagnostics = vec![
            drift("security/fail2ban", DiagnosticSeverity::Warn),
            drift("packages/fail2ban-client", DiagnosticSeverity::Fail),
        ];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            fail_apply: true,
            ..Fake::default()
        };
        let path = temp_file();
        assert!(
            reconcile(
                &proposed,
                &diagnostics,
                &mut fake,
                &path,
                &ApplyOptions {
                    safe: true,
                    ..ApplyOptions::default()
                },
            )
            .is_err()
        );
        assert_eq!(fake.applied.len(), 1);
        assert!(matches!(fake.applied[0], ActionKind::InstallPackage { .. }));
        assert_eq!(
            read_history(&path).unwrap().events.last().unwrap().outcome,
            "failed"
        );
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn process_lock_rejects_a_second_reconciler() {
        let path = temp_file().with_extension("lock");
        let first = ReconcileLock::acquire(&path).unwrap();
        assert!(ReconcileLock::acquire(&path).is_err());
        drop(first);
        assert!(ReconcileLock::acquire(&path).is_ok());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn target_plan_includes_only_drifted_dependencies() {
        let snapshot = serde_json::json!({"ufw": false, "plex": false});
        let diagnostics = vec![
            drift("security/plex-firewall", DiagnosticSeverity::Fail),
            drift("security/ufw", DiagnosticSeverity::Fail),
            drift("security/ssh", DiagnosticSeverity::Fail),
        ];
        let target = plan(
            &snapshot,
            &diagnostics,
            Some(RemediationId::SecurityPlexFirewall),
        )
        .unwrap();
        assert_eq!(
            target
                .remediations
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec!["security/ufw", "security/plex-firewall"]
        );
    }

    #[test]
    fn tailscale_preferences_are_separate_and_auto_update_is_not_safe_allowlisted() {
        let diagnostics = vec![
            drift("security/tailscale-ssh", DiagnosticSeverity::Fail),
            drift("security/tailscale-exit-node", DiagnosticSeverity::Fail),
            drift("security/tailscale-auto-update", DiagnosticSeverity::Fail),
        ];
        let proposed = plan(&serde_json::json!({}), &diagnostics, None).unwrap();
        assert_eq!(
            proposed
                .remediations
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            [
                "security/tailscale-ssh",
                "security/tailscale-exit-node",
                "security/tailscale-auto-update"
            ]
        );
        assert_eq!(proposed.remediations[0].risk, RiskClass::Access);
        assert_eq!(proposed.remediations[1].risk, RiskClass::Access);
        assert_eq!(proposed.remediations[2].risk, RiskClass::Moderate);
        assert!(!safe_allowlisted(
            RemediationId::SecurityTailscaleAutoUpdate
        ));
        let target = plan(
            &serde_json::json!({}),
            &diagnostics,
            Some(RemediationId::SecurityTailscaleAutoUpdate),
        )
        .unwrap();
        assert_eq!(target.remediations.len(), 1);
        assert_eq!(target.remediations[0].id, "security/tailscale-auto-update");
    }

    #[test]
    fn changed_snapshot_requires_reapproval_before_any_action() {
        let snapshot = serde_json::json!({"package": false});
        let diagnostics = vec![drift("packages/docker", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut fake = Fake {
            snapshot: serde_json::json!({"package": true}),
            diagnostics: diagnostics.clone(),
            verified: true,
            ..Fake::default()
        };
        let path = temp_file();
        let error = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &path,
            &ApplyOptions {
                safe: true,
                ..ApplyOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("stale reconciliation plan"));
        assert!(fake.applied.is_empty());
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn refreshed_diagnostics_are_compared_under_lock() {
        let snapshot = serde_json::json!({"package": false});
        let diagnostics = vec![drift("packages/docker", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        let mut changed_diagnostics = diagnostics.clone();
        changed_diagnostics[0]
            .evidence
            .push("changed after preview".into());
        let mut fake = Fake {
            snapshot,
            diagnostics: changed_diagnostics,
            verified: true,
            ..Fake::default()
        };
        let path = temp_file();
        let error = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &path,
            &ApplyOptions {
                safe: true,
                ..ApplyOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("stale reconciliation plan"));
        assert!(fake.applied.is_empty());
        let _ = fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn not_yet_migrated_security_remediations_are_marked_and_refused_before_inspection() {
        let snapshot = serde_json::json!({"ssh": false});
        let diagnostics = vec![drift("security/ssh", DiagnosticSeverity::Fail)];
        let proposed = plan(&snapshot, &diagnostics, None).unwrap();
        assert!(!proposed.remediations[0].available);
        assert!(proposed.remediations[0].unavailable_reason.is_some());
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            ..Fake::default()
        };
        let error = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &temp_file(),
            &ApplyOptions {
                confirmed: [RemediationId::SecuritySsh].into(),
                ..ApplyOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("is unavailable"));
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn locked_replan_rejects_a_forged_available_flag() {
        let snapshot = serde_json::json!({"ssh": false});
        let diagnostics = vec![drift("security/ssh", DiagnosticSeverity::Fail)];
        let mut proposed = plan(&snapshot, &diagnostics, None).unwrap();
        proposed.remediations[0].available = true;
        proposed.remediations[0].unavailable_reason = None;
        let mut fake = Fake {
            snapshot,
            diagnostics: diagnostics.clone(),
            ..Fake::default()
        };
        let error = reconcile(
            &proposed,
            &diagnostics,
            &mut fake,
            &temp_file(),
            &ApplyOptions {
                confirmed: [RemediationId::SecuritySsh].into(),
                ..ApplyOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.contains("is unavailable"));
        assert!(fake.applied.is_empty());
    }

    #[test]
    fn unknown_remediation_ids_are_rejected_instead_of_becoming_commands() {
        let diagnostics = vec![drift("custom/arbitrary-command", DiagnosticSeverity::Fail)];
        assert!(
            plan(&serde_json::json!({}), &diagnostics, None)
                .unwrap_err()
                .contains("unknown remediation")
        );
    }
}
