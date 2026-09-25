//! Read-only host inspection used by the `mistborn status` and `doctor` commands.

use mistborn_bootstrap::domain::{
    CommandAdapter, CommandOutput, Diagnostic, DiagnosticSeverity, ObservedState, RiskClass,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

pub use mistborn_bootstrap::domain::InspectionStatus;

pub(crate) const PLEX_UFW_PROFILE_MARKER: &str = "# Managed by Mistborn: security/plex-firewall";
pub(crate) const PLEX_UFW_PROFILE: &str = "# Managed by Mistborn: security/plex-firewall\n[plexmediaserver]\ntitle=Plex Media Server (Standard)\ndescription=The Plex Media Server\nports=32400/tcp|3005/tcp|5353/udp|8324/tcp|32410:32414/udp\n\n[plexmediaserver-dlna]\ntitle=Plex Media Server (DLNA)\ndescription=The Plex Media Server (additional DLNA capability only)\nports=1900/udp|32469/tcp\n\n[plexmediaserver-all]\ntitle=Plex Media Server (Standard + DLNA)\ndescription=The Plex Media Server (with additional DLNA capability)\nports=32400/tcp|3005/tcp|5353/udp|8324/tcp|32410:32414/udp|1900/udp|32469/tcp\n";

pub struct SystemCommands;

impl CommandAdapter for SystemCommands {
    fn run(&self, program: &str, arguments: &[&str]) -> Result<CommandOutput, String> {
        let output = Command::new(program)
            .args(arguments)
            .output()
            .map_err(|error| error.to_string())?;
        Ok(CommandOutput {
            status: output.status.code().unwrap_or(128),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub command: &'static str,
    pub version: String,
    pub config: ConfigSummary,
    pub summary: ReportSummary,
    pub observed: ObservedState,
    pub diagnostics: Vec<Diagnostic>,
    pub inspection_incomplete: bool,
}

#[derive(Debug, Serialize)]
pub struct ConfigSummary {
    pub path: &'static str,
    pub schema_version: Option<u32>,
    pub status: &'static str,
}

#[derive(Debug, Default, Serialize, PartialEq, Eq)]
pub struct ReportSummary {
    pub pass_count: usize,
    pub warn_count: usize,
    pub fail_count: usize,
    pub error_count: usize,
    pub drift_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Status,
    Doctor,
}

pub fn inspect(kind: ReportKind, runner: &impl CommandAdapter) -> Report {
    inspect_with_package_lookup(kind, runner, find_executable)
}

pub(super) fn inspect_with_package_lookup(
    kind: ReportKind,
    runner: &impl CommandAdapter,
    find_package: impl Fn(&str) -> Option<PathBuf>,
) -> Report {
    let mut observed = ObservedState::default();
    let mut diagnostics = Vec::new();
    let mut versions = BTreeMap::new();
    let mut incomplete = false;
    let mut config_status = "unmanaged";
    let mut config_schema_version = None;
    let desired = match mistborn_bootstrap::config::DesiredState::load(Path::new(
        "/etc/mistborn/config.toml",
    )) {
        Ok(desired) => {
            config_status = "loaded";
            config_schema_version = Some(1);
            observed.facts.insert(
                "desired_config".to_owned(),
                InspectionStatus::Available(json!({"loaded": true, "config": desired})),
            );
            Some(desired)
        }
        Err(mistborn_bootstrap::config::ConfigError::Read(error))
            if error.kind() == ErrorKind::NotFound =>
        {
            observed.facts.insert(
                "desired_config".to_owned(),
                InspectionStatus::Available(json!({"loaded": false, "managed": false})),
            );
            None
        }
        Err(error) => {
            config_status = "error";
            let message = error.to_string();
            observed.facts.insert(
                "desired_config".to_owned(),
                InspectionStatus::Error {
                    message: message.clone(),
                },
            );
            diagnostics.push(diag(
                "config/read",
                DiagnosticSeverity::Warn,
                "Desired configuration could not be loaded",
                vec![message],
                None,
            ));
            incomplete = true;
            None
        }
    };

    for package in [
        "docker",
        "tailscale",
        "rclone",
        "ufw",
        "fail2ban-client",
        "runtipi-cli",
    ] {
        let required = package == "docker"
            || (package == "tailscale"
                && desired
                    .as_ref()
                    .is_some_and(|state| state.tailscale.is_some()))
            || (package == "ufw"
                && desired
                    .as_ref()
                    .is_some_and(|state| state.firewall.is_some()));
        let path = match find_package(package) {
            Some(path) => path.display().to_string(),
            None => {
                observed.facts.insert(
                    format!("package.{package}"),
                    InspectionStatus::Available(json!({"installed": false, "required": required})),
                );
                if required || kind == ReportKind::Doctor {
                    diagnostics.push(diag(
                        format!("package/{package}"),
                        if required {
                            DiagnosticSeverity::Fail
                        } else {
                            DiagnosticSeverity::Warn
                        },
                        format!("{package} is not installed"),
                        vec![],
                        if required {
                            Some(format!("packages/{package}"))
                        } else if package == "fail2ban-client" && kind == ReportKind::Doctor {
                            Some("packages/fail2ban-client".to_owned())
                        } else {
                            None
                        },
                    ));
                }
                continue;
            }
        };
        observed.facts.insert(
            format!("package.{package}"),
            InspectionStatus::Available(
                json!({"installed": true, "required": required, "path": path}),
            ),
        );
        let version = match runner.run(package, &["--version"]) {
            Ok(output) if output.status == 0 => output
                .stdout
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned(),
            Ok(output) => {
                let message = format!(
                    "{package} --version exited {}: {}",
                    output.status,
                    output.stderr.trim()
                );
                observed.facts.insert(
                    format!("package.version.{package}"),
                    InspectionStatus::Error { message },
                );
                String::new()
            }
            Err(error) => {
                let state = command_error(error);
                observed
                    .facts
                    .insert(format!("package.version.{package}"), state);
                String::new()
            }
        };
        if version.is_empty() {
            observed
                .facts
                .entry(format!("package.version.{package}"))
                .or_insert_with(|| InspectionStatus::Error {
                    message: format!("{package} --version returned no version text"),
                });
            incomplete |= required;
            versions.insert(package, None);
        } else {
            observed.facts.insert(
                format!("package.version.{package}"),
                InspectionStatus::Available(json!(version)),
            );
            versions.insert(package, Some(version));
        }
    }

    let runtipi_path = PathBuf::from("/opt/runtipi");
    let runtipi_exists = match fs::metadata(&runtipi_path) {
        Ok(metadata) => Some(metadata.is_dir()),
        Err(error) if error.kind() == ErrorKind::NotFound => Some(false),
        Err(error) => {
            let message = format!("cannot inspect {}: {error}", runtipi_path.display());
            observed.facts.insert(
                "runtipi.directory".to_owned(),
                InspectionStatus::Error {
                    message: message.clone(),
                },
            );
            diagnostics.push(diag(
                "inspection/runtipi/directory",
                DiagnosticSeverity::Warn,
                "Runtipi directory could not be inspected",
                vec![message],
                None,
            ));
            incomplete = true;
            None
        }
    };
    if let Some(exists) = runtipi_exists {
        observed.facts.insert(
            "runtipi.directory".to_owned(),
            InspectionStatus::Available(json!({"path": runtipi_path, "exists": exists})),
        );
        if kind == ReportKind::Doctor {
            diagnostics.push(diag(
                "runtipi/directory",
                if exists {
                    DiagnosticSeverity::Pass
                } else {
                    DiagnosticSeverity::Fail
                },
                if exists {
                    "Runtipi directory exists"
                } else {
                    "Runtipi directory missing"
                },
                vec!["/opt/runtipi".to_owned()],
                None,
            ));
        }
    }

    for service in ["docker", "tailscaled", "fail2ban"] {
        let key = format!("service.{service}");
        let required_package = match service {
            "docker" => Some("docker"),
            "tailscaled" => Some("tailscale"),
            "fail2ban" => Some("fail2ban-client"),
            _ => None,
        };
        if required_package.is_some_and(|package| package_is_missing(&observed, package)) {
            observed.facts.insert(
                key,
                InspectionStatus::Unavailable {
                    reason: format!("{} package is not installed", required_package.unwrap()),
                },
            );
            continue;
        }
        match runner.run("systemctl", &["is-active", service]) {
            Ok(output) => match parse_systemctl_active(&output) {
                Ok(true) => {
                    observed
                        .facts
                        .insert(key, InspectionStatus::Available(json!({"active": true})));
                    if kind == ReportKind::Doctor {
                        diagnostics.push(diag(
                            format!("service/{service}"),
                            if service == "fail2ban" {
                                DiagnosticSeverity::Warn
                            } else {
                                DiagnosticSeverity::Pass
                            },
                            if service == "fail2ban" {
                                "fail2ban active; desired policy is unmanaged".to_owned()
                            } else {
                                format!("{service} active")
                            },
                            vec![],
                            None,
                        ));
                    }
                }
                Ok(false) => {
                    observed.facts.insert(
                        key,
                        InspectionStatus::Available(
                            json!({"active": false, "state": output.stdout.trim()}),
                        ),
                    );
                    if kind == ReportKind::Doctor {
                        let remediation = if service == "fail2ban" {
                            "security/fail2ban".to_owned()
                        } else {
                            format!("services/{service}")
                        };
                        diagnostics.push(diag(
                            format!("service/{service}"),
                            DiagnosticSeverity::Warn,
                            format!("{service} {}", output.stdout.trim()),
                            vec!["systemctl is-active exit status 3".to_owned()],
                            if service == "fail2ban" {
                                Some(remediation)
                            } else {
                                None
                            },
                        ));
                    }
                }
                Err(message) => {
                    let message = format!("systemctl is-active {service}: {message}");
                    observed.facts.insert(
                        key,
                        InspectionStatus::Error {
                            message: message.clone(),
                        },
                    );
                    if kind == ReportKind::Doctor {
                        diagnostics.push(diag(
                            format!("service/{service}"),
                            DiagnosticSeverity::Warn,
                            format!("Could not determine {service} state"),
                            vec![message],
                            None,
                        ));
                    }
                    incomplete |= (service == "tailscaled"
                        && desired
                            .as_ref()
                            .is_some_and(|state| state.tailscale.is_some()))
                        || (service == "docker");
                }
            },
            Err(error) => {
                let state = command_error(error);
                let message = inspection_message(&state);
                if kind == ReportKind::Doctor {
                    diagnostics.push(diag(
                        format!("inspection/service/{service}"),
                        DiagnosticSeverity::Warn,
                        format!("Could not inspect {service} service state"),
                        vec![message],
                        None,
                    ));
                }
                incomplete |= service == "docker"
                    || (service == "tailscaled"
                        && desired
                            .as_ref()
                            .is_some_and(|state| state.tailscale.is_some()));
                observed.facts.insert(key, state);
            }
        }
    }

    incomplete |= inspect_ufw(
        runner,
        desired.as_ref(),
        &mut observed,
        kind,
        &mut diagnostics,
    );

    inspect_command(
        runner,
        &mut observed,
        CommandCheck {
            key: "fail2ban.sshd",
            program: "fail2ban-client",
            args: &["status", "sshd"],
            pass: "fail2ban sshd jail available",
            fail: "fail2ban sshd jail unavailable",
        },
        ReportKind::Status,
        &mut diagnostics,
    );
    if kind == ReportKind::Doctor
        && desired
            .as_ref()
            .and_then(|state| state.firewall.as_ref())
            .and_then(|firewall| firewall.plex.as_ref())
            .is_none()
    {
        diagnostics.push(diag(
            "security/plex-firewall",
            DiagnosticSeverity::Warn,
            "Plex firewall policy is unmanaged",
            vec![],
            None,
        ));
    }
    // fail2ban has no desired-state section in config schema v1. Keep its facts
    // visible, but do not report a passing policy check until it is managed.
    let tailscale_incomplete = inspect_tailscale(
        runner,
        desired.as_ref(),
        &mut observed,
        kind,
        &mut diagnostics,
    );
    incomplete |= tailscale_incomplete;
    inspect_command(
        runner,
        &mut observed,
        CommandCheck {
            key: "ssh.effective",
            program: "sshd",
            args: &["-T"],
            pass: "SSH effective configuration available",
            fail: "SSH effective configuration unavailable",
        },
        ReportKind::Status,
        &mut diagnostics,
    );

    if kind == ReportKind::Doctor {
        diagnostics.push(diag(
            "security/fail2ban",
            DiagnosticSeverity::Warn,
            "fail2ban policy is unmanaged",
            vec![],
            None,
        ));
    }

    if let Some(InspectionStatus::Available(value)) = observed.facts.get("ssh.effective") {
        match parse_sshd(
            value
                .get("output")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        ) {
            Ok(ssh) => {
                observed.facts.insert(
                    "ssh.policy".to_owned(),
                    InspectionStatus::Available(ssh.clone()),
                );
                if let Some(policy) = desired.as_ref().and_then(|state| state.ssh.as_ref()) {
                    let diagnostic = compare_ssh_policy(&ssh, policy);
                    observed.facts.insert(
                        "ssh.comparison".to_owned(),
                        InspectionStatus::Available(
                            json!({"compliant": diagnostic.severity == DiagnosticSeverity::Pass}),
                        ),
                    );
                    diagnostics.push(diagnostic);
                } else if kind == ReportKind::Doctor {
                    diagnostics.push(diag(
                        "security/ssh",
                        DiagnosticSeverity::Warn,
                        "SSH policy is unmanaged",
                        vec![ssh.to_string()],
                        None,
                    ));
                }
            }
            Err(message) => {
                observed
                    .facts
                    .insert("ssh.policy".to_owned(), InspectionStatus::Error { message });
            }
        }
    }
    if desired.as_ref().is_some_and(|state| state.ssh.is_some())
        && !matches!(
            observed.facts.get("ssh.policy"),
            Some(InspectionStatus::Available(_))
        )
    {
        incomplete = true;
    }

    observed.facts.insert(
        "versions".to_owned(),
        InspectionStatus::Available(serde_json::to_value(versions).unwrap_or(Value::Null)),
    );
    add_inspection_diagnostics(&observed, &mut diagnostics);
    let summary = summarize(&observed, &diagnostics);
    Report {
        schema_version: 1,
        command: match kind {
            ReportKind::Status => "status",
            ReportKind::Doctor => "doctor",
        },
        version: format!("v{}", env!("CARGO_PKG_VERSION")),
        config: ConfigSummary {
            path: "/etc/mistborn/config.toml",
            schema_version: config_schema_version,
            status: config_status,
        },
        summary,
        observed,
        diagnostics,
        inspection_incomplete: incomplete,
    }
}

fn package_is_missing(observed: &ObservedState, package: &str) -> bool {
    matches!(
        observed.facts.get(&format!("package.{package}")),
        Some(InspectionStatus::Available(value)) if value["installed"] == false
    )
}

fn summarize(observed: &ObservedState, diagnostics: &[Diagnostic]) -> ReportSummary {
    let mut summary = ReportSummary::default();
    for diagnostic in diagnostics {
        match diagnostic.severity {
            DiagnosticSeverity::Pass => summary.pass_count += 1,
            DiagnosticSeverity::Warn => summary.warn_count += 1,
            DiagnosticSeverity::Fail => summary.fail_count += 1,
        }
    }
    summary.drift_count = summary.fail_count;
    summary.error_count = observed
        .facts
        .values()
        .filter(|state| matches!(state, InspectionStatus::Error { .. }))
        .count();
    summary
}

struct CommandCheck<'a> {
    key: &'a str,
    program: &'a str,
    args: &'a [&'a str],
    pass: &'a str,
    fail: &'a str,
}

fn inspect_command(
    runner: &impl CommandAdapter,
    observed: &mut ObservedState,
    check: CommandCheck<'_>,
    kind: ReportKind,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let CommandCheck {
        key,
        program,
        args,
        pass,
        fail,
    } = check;
    match runner.run(program, args) {
        Ok(output) if output.status == 0 => {
            observed.facts.insert(
                key.to_owned(),
                InspectionStatus::Available(
                    json!({"ok": true, "output": output.stdout, "status": output.status}),
                ),
            );
            if kind == ReportKind::Doctor {
                diagnostics.push(diag(
                    key.replace('.', "/"),
                    DiagnosticSeverity::Pass,
                    pass,
                    vec![],
                    None,
                ));
            }
        }
        Ok(output) => {
            let message = format!(
                "{program} {} exited {}: {}",
                args.join(" "),
                output.status,
                output.stderr.trim()
            );
            observed.facts.insert(
                key.to_owned(),
                InspectionStatus::Error {
                    message: message.clone(),
                },
            );
            if kind == ReportKind::Doctor {
                diagnostics.push(diag(
                    key.replace('.', "/"),
                    DiagnosticSeverity::Warn,
                    fail,
                    vec![message],
                    None,
                ));
            }
        }
        Err(error) => {
            observed.facts.insert(key.to_owned(), command_error(error));
        }
    }
}

fn inspect_ufw(
    runner: &impl CommandAdapter,
    desired: Option<&mistborn_bootstrap::config::DesiredState>,
    observed: &mut ObservedState,
    kind: ReportKind,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    let managed = desired.and_then(|state| state.firewall.as_ref());
    if package_is_missing(observed, "ufw") {
        observed.facts.insert(
            "firewall.ufw".to_owned(),
            InspectionStatus::Unavailable {
                reason: "ufw package is not installed".to_owned(),
            },
        );
        observed.facts.insert(
            "firewall.managed".to_owned(),
            InspectionStatus::Available(json!({"managed": managed.is_some()})),
        );
        for fact in [
            "firewall.policy",
            "firewall.public_tcp_ports",
            "plex.ufw_profile",
            "plex.policy",
            "plex.ufw_rules",
        ] {
            observed.facts.insert(
                fact.to_owned(),
                InspectionStatus::Unavailable {
                    reason: "ufw package is not installed".to_owned(),
                },
            );
        }
        return false;
    }
    let output = match runner.run("ufw", &["status", "verbose"]) {
        Ok(output) if output.status == 0 => match parse_ufw(&output.stdout) {
            Ok(parsed) => {
                observed.facts.insert(
                    "firewall.ufw".to_owned(),
                    InspectionStatus::Available(json!({
                        "active": parsed.active,
                        "default_incoming": parsed.default_incoming,
                        "output": output.stdout,
                    })),
                );
                Some((parsed, output.stdout))
            }
            Err(error) => {
                observed.facts.insert(
                    "firewall.ufw".to_owned(),
                    InspectionStatus::Error {
                        message: error.clone(),
                    },
                );
                diagnostics.push(diag(
                    "inspection/firewall/ufw",
                    DiagnosticSeverity::Warn,
                    "UFW output could not be interpreted",
                    vec![error],
                    None,
                ));
                None
            }
        },
        Ok(output) => {
            let error = format!(
                "ufw status verbose exited {}: {}",
                output.status,
                output.stderr.trim()
            );
            observed.facts.insert(
                "firewall.ufw".to_owned(),
                InspectionStatus::Error {
                    message: error.clone(),
                },
            );
            diagnostics.push(diag(
                "inspection/firewall/ufw",
                DiagnosticSeverity::Warn,
                "UFW state could not be inspected",
                vec![error],
                None,
            ));
            None
        }
        Err(error) => {
            let state = command_error(error);
            let message = inspection_message(&state);
            observed.facts.insert("firewall.ufw".to_owned(), state);
            diagnostics.push(diag(
                "inspection/firewall/ufw",
                DiagnosticSeverity::Warn,
                "UFW command is unavailable",
                vec![message],
                None,
            ));
            None
        }
    };

    let Some((ufw, raw_status)) = output else {
        return managed.is_some();
    };
    let Some(firewall) = managed else {
        if kind == ReportKind::Doctor {
            diagnostics.push(diag(
                "security/ufw",
                DiagnosticSeverity::Warn,
                "UFW policy is unmanaged",
                vec![],
                None,
            ));
        }
        observed.facts.insert(
            "firewall.managed".to_owned(),
            InspectionStatus::Available(json!({"managed": false})),
        );
        return false;
    };
    observed.facts.insert(
        "firewall.managed".to_owned(),
        InspectionStatus::Available(json!({"managed": true})),
    );
    let mut mismatch = Vec::new();
    if ufw.active != firewall.enabled {
        mismatch.push(format!(
            "enabled: observed {}, desired {}",
            ufw.active, firewall.enabled
        ));
    }
    let desired_default = match firewall.default_incoming {
        mistborn_bootstrap::config::IncomingPolicy::Allow => "allow",
        mistborn_bootstrap::config::IncomingPolicy::Deny => "deny",
        mistborn_bootstrap::config::IncomingPolicy::Reject => "reject",
    };
    if ufw.default_incoming != desired_default {
        mismatch.push(format!(
            "default incoming: observed {}, desired {desired_default}",
            ufw.default_incoming
        ));
    }
    let missing_ports = if firewall.enabled {
        firewall
            .public_tcp_ports
            .iter()
            .filter(|port| !ufw_allows_public_tcp(&raw_status, port.get()))
            .map(|port| port.get())
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    if !missing_ports.is_empty() {
        mismatch.push(format!(
            "public TCP ports missing: {}",
            missing_ports
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if firewall.enabled
        && firewall.default_incoming != mistborn_bootstrap::config::IncomingPolicy::Allow
        && let Some(ssh) = desired.and_then(|state| state.ssh.as_ref())
        && !ufw_allows_public_tcp(&raw_status, ssh.port.get())
    {
        mismatch.push(format!(
            "managed SSH TCP port {} is not allowed publicly before the default-deny policy",
            ssh.port.get()
        ));
    }
    let stale_owned_rules = stale_ufw_managed_ports(&raw_status, firewall, desired);
    mismatch.extend(stale_owned_rules.iter().cloned());
    observed.facts.insert("firewall.public_tcp_ports".to_owned(), InspectionStatus::Available(json!({"desired": firewall.public_tcp_ports.iter().map(|port| port.get()).collect::<Vec<_>>(), "missing": missing_ports})));
    let compliant = mismatch.is_empty();
    observed.facts.insert(
        "firewall.policy".to_owned(),
        InspectionStatus::Available(json!({"compliant": compliant, "issues": mismatch})),
    );
    {
        let mut diagnostic = diag(
            "security/ufw",
            if compliant {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Fail
            },
            if compliant {
                "UFW matches desired firewall policy"
            } else {
                "UFW differs from desired firewall policy"
            },
            observed
                .facts
                .get("firewall.policy")
                .and_then(|fact| match fact {
                    InspectionStatus::Available(value) => {
                        value.get("issues").and_then(Value::as_array)
                    }
                    _ => None,
                })
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            if compliant || !stale_owned_rules.is_empty() {
                None
            } else {
                Some("security/ufw".to_owned())
            },
        );
        if !compliant {
            diagnostic.risk = Some(RiskClass::Access);
            diagnostic.confirmation_required = true;
        }
        diagnostics.push(diagnostic);
    }
    if !stale_owned_rules.is_empty() {
        diagnostics.push(diag(
            "security/ufw/manual-cleanup",
            DiagnosticSeverity::Fail,
            "UFW contains stale Mistborn-owned public port rules that are not deleted automatically",
            stale_owned_rules,
            None,
        ));
    }

    let Some(plex) = firewall.plex.as_ref() else {
        observed.facts.insert(
            "plex.ufw_profile".to_owned(),
            InspectionStatus::Available(json!({"managed": false})),
        );
        return false;
    };
    let profile_path = Path::new("/etc/ufw/applications.d/plexmediaserver");
    let profile = if plex.enabled {
        match fs::read_to_string(profile_path) {
            Ok(contents) => {
                let profile_is_expected = plex_profile_is_expected(&contents);
                observed.facts.insert(
                    "plex.ufw_profile".to_owned(),
                    InspectionStatus::Available(
                        json!({"managed": true, "exists": true, "path": profile_path, "profile_is_expected": profile_is_expected}),
                    ),
                );
                Some(contents)
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                observed.facts.insert(
                    "plex.ufw_profile".to_owned(),
                    InspectionStatus::Available(
                        json!({"managed": true, "exists": false, "path": profile_path}),
                    ),
                );
                None
            }
            Err(error) => {
                let message = format!("cannot read {}: {error}", profile_path.display());
                observed.facts.insert(
                    "plex.ufw_profile".to_owned(),
                    InspectionStatus::Error {
                        message: message.clone(),
                    },
                );
                diagnostics.push(diag(
                    "inspection/plex/ufw-profile",
                    DiagnosticSeverity::Warn,
                    "Plex UFW profile could not be inspected",
                    vec![message],
                    None,
                ));
                return true;
            }
        }
    } else {
        observed.facts.insert(
            "plex.ufw_profile".to_owned(),
            InspectionStatus::Available(
                json!({"managed": true, "enabled": false, "profile_required": false}),
            ),
        );
        None
    };
    let plex_comparison = compare_plex_profile_and_rules(profile.as_deref(), &raw_status, plex);
    let plex_issues = plex_comparison.issues;
    let public = ufw_allows_public_tcp(&raw_status, 32400);
    let plex_compliant = plex_issues.is_empty();
    observed.facts.insert(
        "plex.policy".to_owned(),
        InspectionStatus::Available(json!({"compliant": plex_compliant, "issues": plex_issues})),
    );
    observed.facts.insert(
        "plex.ufw_rules".to_owned(),
        InspectionStatus::Available(json!({"public_tcp_32400": public, "issues": plex_issues})),
    );
    {
        let mut diagnostic = diag(
            "security/plex-firewall",
            if plex_compliant {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Fail
            },
            if plex_compliant {
                "Plex UFW profile and effective rules match desired policy"
            } else {
                "Plex UFW profile or effective rules drifted"
            },
            observed
                .facts
                .get("plex.policy")
                .and_then(|fact| match fact {
                    InspectionStatus::Available(value) => {
                        value.get("issues").and_then(Value::as_array)
                    }
                    _ => None,
                })
                .map(|items| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
            if plex_compliant || plex_comparison.requires_manual_cleanup {
                None
            } else {
                Some("security/plex-firewall".to_owned())
            },
        );
        if !plex_compliant {
            diagnostic.risk = Some(RiskClass::Access);
            diagnostic.confirmation_required = true;
        }
        diagnostics.push(diagnostic);
    }
    if plex_comparison.requires_manual_cleanup {
        diagnostics.push(diag(
            "security/plex-firewall/manual-cleanup",
            DiagnosticSeverity::Fail,
            "Plex firewall has stale or overbroad allow rules that Mistborn will not delete automatically",
            plex_issues.clone(),
            None,
        ));
    }
    false
}

#[derive(Debug)]
struct UfwState {
    active: bool,
    default_incoming: String,
}

fn parse_ufw(output: &str) -> Result<UfwState, String> {
    let mut status = None;
    let mut default = None;
    let mut rules_header = false;
    for line in output.lines() {
        if let Some(value) = line.trim().strip_prefix("Status:") {
            status = match value.trim() {
                "active" => Some(true),
                "inactive" => Some(false),
                value => return Err(format!("unknown UFW status value: {value}")),
            };
        }
        if let Some(value) = line.trim().strip_prefix("Default:") {
            let incoming = value
                .split(',')
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .next()
                .unwrap_or_default();
            if matches!(incoming, "allow" | "deny" | "reject") {
                default = Some(incoming.to_owned());
            }
        }
        if line.trim().starts_with("To") && line.contains("Action") && line.contains("From") {
            rules_header = true;
        }
    }
    if !rules_header {
        return Err("ufw status output omitted the rules table header".to_owned());
    }
    Ok(UfwState {
        active: status.ok_or_else(|| "ufw status output omitted Status".to_owned())?,
        default_incoming: default
            .ok_or_else(|| "ufw status output omitted or malformed Default policy".to_owned())?,
    })
}

fn ufw_allows_public_tcp(output: &str, port: u16) -> bool {
    let target = format!("{port}/tcp");
    output.lines().any(|line| {
        line.split_whitespace().next() == Some(target.as_str())
            && line.contains("ALLOW IN")
            && line.contains("Anywhere")
            && !line.contains("tailscale0")
    })
}

fn ufw_target_overlaps(rule_target: &str, expected_target: &str) -> bool {
    let Some((rule_ports, rule_protocol)) = rule_target.split_once('/') else {
        return false;
    };
    let Some((expected_ports, expected_protocol)) = expected_target.split_once('/') else {
        return false;
    };
    if !rule_protocol.eq_ignore_ascii_case(expected_protocol) {
        return false;
    }
    let parse_range = |value: &str| -> Option<(u16, u16)> {
        if let Some((start, end)) = value.split_once(':') {
            Some((start.parse().ok()?, end.parse().ok()?))
        } else {
            let port = value.parse().ok()?;
            Some((port, port))
        }
    };
    let (rule_start, rule_end) = match parse_range(rule_ports) {
        Some(range) => range,
        None => return false,
    };
    let (expected_start, expected_end) = match parse_range(expected_ports) {
        Some(range) => range,
        None => return false,
    };
    rule_start <= expected_end && expected_start <= rule_end
}

fn stale_ufw_managed_ports(
    status: &str,
    firewall: &mistborn_bootstrap::config::FirewallConfig,
    desired: Option<&mistborn_bootstrap::config::DesiredState>,
) -> Vec<String> {
    let mut stale = Vec::new();
    let desired_ports = firewall
        .public_tcp_ports
        .iter()
        .map(|port| port.get())
        .collect::<std::collections::BTreeSet<_>>();
    let ssh_port = desired
        .and_then(|state| state.ssh.as_ref())
        .map(|ssh| ssh.port.get());
    for line in status.lines() {
        let Some((target, comment)) = line.split_once('#') else {
            continue;
        };
        let rule = target.split_whitespace().next().unwrap_or_default();
        let Some(port) = rule
            .strip_suffix("/tcp")
            .and_then(|port| port.parse::<u16>().ok())
        else {
            continue;
        };
        let comment = comment.trim();
        if comment.starts_with("mistborn:security/ufw:public")
            && (!firewall.enabled || !desired_ports.contains(&port))
        {
            stale.push(format!(
                "stale Mistborn public allow rule remains for TCP {port}"
            ));
        } else if comment.starts_with("mistborn:security/ufw:ssh")
            && (!firewall.enabled || Some(port) != ssh_port)
        {
            stale.push(format!(
                "stale Mistborn SSH allow rule remains for TCP {port}"
            ));
        }
    }
    stale
}

fn profile_port_map(profile: &str) -> BTreeMap<String, Vec<String>> {
    let mut section = String::new();
    let mut ports = BTreeMap::<String, Vec<String>>::new();
    for line in profile.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_ascii_lowercase();
        } else if let Some((key, value)) = line.split_once('=')
            && key.trim().eq_ignore_ascii_case("ports")
            && !section.is_empty()
        {
            let mut entries = value
                .split(['|', ','])
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>();
            entries.sort();
            ports.insert(section.clone(), entries);
        }
    }
    ports
}

pub(crate) fn plex_profile_is_expected(profile: &str) -> bool {
    let observed = profile_port_map(profile);
    let expected = profile_port_map(PLEX_UFW_PROFILE);
    let sections = profile
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            (line.starts_with('[') && line.ends_with(']'))
                .then(|| line[1..line.len() - 1].trim().to_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    let expected_sections = [
        "plexmediaserver",
        "plexmediaserver-dlna",
        "plexmediaserver-all",
    ]
    .map(str::to_owned);
    sections.len() == expected_sections.len()
        && expected_sections
            .iter()
            .all(|section| sections.contains(section))
        && observed == expected
}

fn ufw_app_rule_is_allowed<'a>(status: &'a str, app: &str) -> Vec<Vec<&'a str>> {
    status
        .lines()
        .filter_map(|line| {
            let tokens = line.split_whitespace().collect::<Vec<_>>();
            (tokens
                .first()
                .is_some_and(|target| target.eq_ignore_ascii_case(app))
                && tokens.windows(2).any(|pair| pair == ["ALLOW", "IN"]))
            .then_some(tokens)
        })
        .collect()
}

fn ufw_app_rule_from(status: &str, app: &str, source: &str) -> bool {
    ufw_app_rule_is_allowed(status, app)
        .iter()
        .any(|tokens| tokens.iter().any(|token| *token == source))
}

fn ufw_app_rule_on_interface(status: &str, app: &str, interface: &str) -> bool {
    ufw_app_rule_is_allowed(status, app)
        .iter()
        .any(|tokens| tokens.windows(2).any(|pair| pair == ["on", interface]))
}

fn ufw_app_rule_public(status: &str, app: &str) -> bool {
    ufw_app_rule_is_allowed(status, app).iter().any(|tokens| {
        tokens.contains(&"Anywhere") && !tokens.windows(2).any(|pair| pair[0] == "on")
    })
}

#[derive(Debug, Default)]
struct PlexComparison {
    issues: Vec<String>,
    requires_manual_cleanup: bool,
}

fn compare_plex_profile_and_rules(
    profile: Option<&str>,
    status: &str,
    plex: &mistborn_bootstrap::config::PlexFirewallConfig,
) -> PlexComparison {
    let mut comparison = PlexComparison::default();
    let issues = &mut comparison.issues;
    if !plex.enabled {
        let direct_rule = status
            .lines()
            .any(|line| line.contains("32400/tcp") && line.contains("ALLOW IN"));
        let app_rule = [
            "plexmediaserver",
            "plexmediaserver-dlna",
            "plexmediaserver-all",
        ]
        .iter()
        .any(|app| !ufw_app_rule_is_allowed(status, app).is_empty());
        if direct_rule || app_rule {
            issues.push("Plex firewall is disabled but an allow rule for TCP 32400 or a Plex app profile remains".to_owned());
            comparison.requires_manual_cleanup = true;
        }
        return comparison;
    }
    match profile {
        None => issues.push("Plex UFW profile is missing".to_owned()),
        Some(contents) if !plex_profile_is_expected(contents) => {
            issues.push("Plex UFW profile port declarations differ from the expected standard, DLNA, or all-services profile".to_owned())
        }
        Some(_) => {}
    }
    if let Some(contents) = profile
        && !contents.starts_with(PLEX_UFW_PROFILE_MARKER)
        && !plex_profile_is_expected(contents)
    {
        issues.push(
            "unowned Plex UFW profile differs from expected definitions and cannot be replaced automatically"
                .to_owned(),
        );
        comparison.requires_manual_cleanup = true;
    }
    let public = ufw_allows_public_tcp(status, 32400);
    if plex.public_remote_access && !public {
        issues.push("public TCP 32400 rule is missing".to_owned());
    }
    if !plex.public_remote_access && public {
        issues.push("TCP 32400 is publicly allowed although public access is disabled".to_owned());
    }
    if let Some(cidr) = &plex.lan_cidr {
        if !ufw_app_rule_from(status, "plexmediaserver-all", cidr.as_str()) {
            issues.push(format!(
                "LAN plexmediaserver-all app rule for {} is missing",
                cidr.as_str()
            ));
        }
    }
    let app_names = [
        "plexmediaserver",
        "plexmediaserver-dlna",
        "plexmediaserver-all",
    ];
    for app in app_names {
        for tokens in ufw_app_rule_is_allowed(status, app) {
            let interface = tokens
                .windows(2)
                .find(|pair| pair[0] == "on")
                .map(|pair| pair[1]);
            let source = tokens
                .windows(2)
                .position(|pair| pair == ["ALLOW", "IN"])
                .and_then(|index| tokens.get(index + 2))
                .copied();
            if let Some(interface) = interface {
                if interface != "tailscale0" || !plex.tailscale || app != "plexmediaserver-all" {
                    issues.push(format!(
                        "unexpected {app} app allow rule on interface {interface}"
                    ));
                    comparison.requires_manual_cleanup = true;
                }
            } else if let Some(source) = source
                && source != "Anywhere"
                && !source.starts_with("Anywhere")
                && (app != "plexmediaserver-all"
                    || plex
                        .lan_cidr
                        .as_ref()
                        .is_none_or(|cidr| cidr.as_str() != source))
            {
                issues.push(format!("unexpected {app} app allow rule from {source}"));
                comparison.requires_manual_cleanup = true;
            }
        }
    }
    if plex.tailscale && !ufw_app_rule_on_interface(status, "plexmediaserver-all", "tailscale0") {
        issues.push("Tailscale plexmediaserver-all app rule on tailscale0 is missing".to_owned());
    }
    for app in [
        "plexmediaserver",
        "plexmediaserver-dlna",
        "plexmediaserver-all",
    ] {
        if ufw_app_rule_public(status, app) {
            issues.push(format!(
                "Plex app profile {app} is exposed on all public interfaces; only TCP 32400 may be public"
            ));
            comparison.requires_manual_cleanup = true;
        }
    }
    let plex_ports = profile_port_map(PLEX_UFW_PROFILE);
    for line in status.lines() {
        let Some(rule_target) = line.split_whitespace().next() else {
            continue;
        };
        if !line.contains("ALLOW IN")
            || !line.contains("Anywhere")
            || line.split_whitespace().any(|token| token == "on")
        {
            continue;
        }
        for expected_target in plex_ports.values().flatten() {
            if ufw_target_overlaps(rule_target, expected_target)
                && !(expected_target == "32400/tcp"
                    && rule_target == "32400/tcp"
                    && plex.public_remote_access)
            {
                issues.push(format!(
                    "Plex profile port {rule_target} is publicly allowed; only TCP 32400 may be public"
                ));
                comparison.requires_manual_cleanup = true;
                break;
            }
        }
    }
    comparison
}

fn inspection_message(status: &InspectionStatus<Value>) -> String {
    match status {
        InspectionStatus::Unavailable { reason } => reason.clone(),
        InspectionStatus::Error { message } => message.clone(),
        InspectionStatus::Available(_) => "available".to_owned(),
    }
}

fn add_inspection_diagnostics(observed: &ObservedState, diagnostics: &mut Vec<Diagnostic>) {
    for (fact_id, state) in &observed.facts {
        let message = match state {
            InspectionStatus::Unavailable { reason } => {
                Some(format!("inspection unavailable: {reason}"))
            }
            InspectionStatus::Error { message } => Some(format!("inspection error: {message}")),
            InspectionStatus::Available(_) => None,
        };
        let Some(message) = message else { continue };
        let id = format!("inspection/{}", fact_id.replace('.', "/"));
        if diagnostics
            .iter()
            .any(|item| item.id == id || item.id == "config/read" && fact_id == "desired_config")
        {
            continue;
        }
        diagnostics.push(diag(
            id,
            DiagnosticSeverity::Warn,
            format!("Could not inspect {fact_id}"),
            vec![message],
            None,
        ));
    }
}

fn inspect_tailscale(
    runner: &impl CommandAdapter,
    desired: Option<&mistborn_bootstrap::config::DesiredState>,
    observed: &mut ObservedState,
    kind: ReportKind,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    if package_is_missing(observed, "tailscale") {
        for fact in [
            "tailscale.status",
            "tailscale.preferences",
            "tailscale.ssh",
            "tailscale.exit_node",
            "tailscale.auto_update",
        ] {
            observed.facts.insert(
                fact.to_owned(),
                InspectionStatus::Unavailable {
                    reason: "tailscale package is not installed".to_owned(),
                },
            );
        }
        if kind == ReportKind::Doctor
            && desired.and_then(|state| state.tailscale.as_ref()).is_none()
        {
            diagnostics.push(diag(
                "security/tailscale",
                DiagnosticSeverity::Warn,
                "Tailscale policy is unmanaged",
                vec![],
                None,
            ));
        }
        return false;
    }
    let tailscale_running = match runner.run("tailscale", &["status", "--json"]) {
        Ok(output) if output.status == 0 => match serde_json::from_str::<Value>(&output.stdout) {
            Ok(value) => match value.get("BackendState").and_then(Value::as_str) {
                Some("Running") => {
                    observed.facts.insert(
                        "tailscale.status".to_owned(),
                        InspectionStatus::Available(value),
                    );
                    true
                }
                Some(state) => {
                    observed.facts.insert(
                        "tailscale.status".to_owned(),
                        InspectionStatus::Unavailable {
                            reason: format!("Tailscale backend is {state}, not Running"),
                        },
                    );
                    false
                }
                None => {
                    observed.facts.insert(
                        "tailscale.status".to_owned(),
                        InspectionStatus::Error {
                            message: "tailscale status JSON has no BackendState".to_owned(),
                        },
                    );
                    false
                }
            },
            Err(error) => {
                observed.facts.insert(
                    "tailscale.status".to_owned(),
                    InspectionStatus::Error {
                        message: format!("tailscale status returned malformed JSON: {error}"),
                    },
                );
                false
            }
        },
        Ok(output) => {
            observed.facts.insert(
                "tailscale.status".to_owned(),
                InspectionStatus::Error {
                    message: format!(
                        "tailscale status exited {}: {}",
                        output.status,
                        output.stderr.trim()
                    ),
                },
            );
            false
        }
        Err(error) => {
            observed
                .facts
                .insert("tailscale.status".to_owned(), command_error(error));
            false
        }
    };
    let tailscale = desired.and_then(|state| state.tailscale.as_ref());
    if !tailscale_running {
        let reason = observed
            .facts
            .get("tailscale.status")
            .map(inspection_message)
            .unwrap_or_else(|| "Tailscale backend state unavailable".to_owned());
        observed.facts.insert(
            "tailscale.preferences".to_owned(),
            InspectionStatus::Unavailable {
                reason: reason.clone(),
            },
        );
        for fact in [
            "tailscale.ssh",
            "tailscale.exit_node",
            "tailscale.auto_update",
        ] {
            observed.facts.insert(
                fact.to_owned(),
                InspectionStatus::Unavailable {
                    reason: reason.clone(),
                },
            );
        }
        if kind == ReportKind::Doctor {
            diagnostics.push(diag(
                "inspection/tailscale/status",
                DiagnosticSeverity::Warn,
                "Tailscale backend is unavailable or not Running",
                vec![reason],
                None,
            ));
            if tailscale.is_none() {
                diagnostics.push(diag(
                    "security/tailscale",
                    DiagnosticSeverity::Warn,
                    "Tailscale policy is unmanaged",
                    vec![],
                    None,
                ));
            }
        }
        return tailscale.is_some();
    }
    let prefs_state = match runner.run("tailscale", &["get", "--json"]) {
        Ok(output) if output.status == 0 => match serde_json::from_str::<Value>(&output.stdout) {
            Ok(value) => {
                // Retain only the three explicitly managed values. `get --json`
                // includes all preferences, but unrelated values are neither
                // owned nor useful in state reports/fingerprints.
                let managed = json!({
                    "ssh": value.get("ssh").cloned().unwrap_or(Value::Null),
                    "advertise-exit-node": value.get("advertise-exit-node").cloned().unwrap_or(Value::Null),
                    "auto-update": value.get("auto-update").cloned().unwrap_or(Value::Null),
                });
                observed.facts.insert(
                    "tailscale.preferences".to_owned(),
                    InspectionStatus::Available(managed.clone()),
                );
                Some(managed)
            }
            Err(error) => {
                observed.facts.insert(
                    "tailscale.preferences".to_owned(),
                    InspectionStatus::Error {
                        message: format!("tailscale get returned malformed JSON: {error}"),
                    },
                );
                None
            }
        },
        Ok(output) => {
            observed.facts.insert(
                "tailscale.preferences".to_owned(),
                InspectionStatus::Error {
                    message: format!(
                        "tailscale get exited {}: {}",
                        output.status,
                        output.stderr.trim()
                    ),
                },
            );
            None
        }
        Err(error) => {
            observed
                .facts
                .insert("tailscale.preferences".to_owned(), command_error(error));
            None
        }
    };
    let Some(tailscale) = tailscale else {
        if kind == ReportKind::Doctor {
            diagnostics.push(diag(
                "security/tailscale",
                DiagnosticSeverity::Warn,
                "Tailscale policy is unmanaged",
                vec![],
                None,
            ));
        }
        return false;
    };
    let Some(prefs) = prefs_state else {
        if kind == ReportKind::Doctor {
            diagnostics.push(diag(
                "inspection/tailscale/preferences",
                DiagnosticSeverity::Warn,
                "Managed Tailscale policy could not be inspected",
                vec![
                    observed
                        .facts
                        .get("tailscale.preferences")
                        .map(inspection_message)
                        .unwrap_or_default(),
                ],
                None,
            ));
        }
        return true;
    };
    let preferences = [
        (
            "ssh",
            "tailscale.ssh",
            "security/tailscale-ssh",
            tailscale.ssh,
            RiskClass::Access,
        ),
        (
            "advertise-exit-node",
            "tailscale.exit_node",
            "security/tailscale-exit-node",
            tailscale.advertise_exit_node,
            RiskClass::Access,
        ),
        (
            "auto-update",
            "tailscale.auto_update",
            "security/tailscale-auto-update",
            tailscale.auto_update,
            RiskClass::Moderate,
        ),
    ];
    let mut incomplete = false;
    for (name, fact, remediation, wanted, risk) in preferences {
        let actual = prefs.get(name).and_then(Value::as_bool);
        let Some(actual) = actual else {
            let message = format!("tailscale get omitted or returned an invalid {name} value");
            observed.facts.insert(
                fact.to_owned(),
                InspectionStatus::Error {
                    message: message.clone(),
                },
            );
            diagnostics.push(diag(
                format!("inspection/{fact}"),
                DiagnosticSeverity::Warn,
                "Managed Tailscale preference could not be inspected",
                vec![message],
                None,
            ));
            incomplete = true;
            continue;
        };
        let compliant = actual == wanted;
        observed.facts.insert(
            fact.to_owned(),
            InspectionStatus::Available(
                json!({"observed": actual, "desired": wanted, "compliant": compliant}),
            ),
        );
        let mut diagnostic = diag(
            remediation,
            if compliant {
                DiagnosticSeverity::Pass
            } else {
                DiagnosticSeverity::Fail
            },
            if compliant {
                format!("Tailscale {name} preference matches desired policy")
            } else {
                format!("Tailscale {name} preference drifted")
            },
            if compliant {
                vec![]
            } else {
                vec![format!("{name}: observed {actual}, desired {wanted}")]
            },
            (!compliant).then_some(remediation.to_owned()),
        );
        if !compliant && risk == RiskClass::Access {
            diagnostic.risk = Some(RiskClass::Access);
            diagnostic.confirmation_required = true;
        } else if !compliant {
            diagnostic.risk = Some(RiskClass::Moderate);
        }
        diagnostics.push(diagnostic);
    }
    incomplete
}

fn compare_ssh_policy(
    observed: &Value,
    desired: &mistborn_bootstrap::config::SshConfig,
) -> Diagnostic {
    let mut observed_ports = observed
        .get("ports")
        .and_then(Value::as_array)
        .and_then(|ports| ports.iter().map(Value::as_u64).collect::<Option<Vec<_>>>());
    if let Some(ports) = observed_ports.as_mut() {
        ports.sort_unstable();
    }
    let expected_ports = vec![u64::from(desired.port.get())];
    let compliant = observed_ports.as_ref() == Some(&expected_ports)
        && observed
            .get("passwordauthentication")
            .and_then(Value::as_str)
            .is_some_and(|value| {
                value.eq_ignore_ascii_case(if desired.password_authentication {
                    "yes"
                } else {
                    "no"
                })
            })
        && observed
            .get("permitrootlogin")
            .and_then(Value::as_str)
            .is_some_and(|value| {
                value.eq_ignore_ascii_case(if desired.root_login { "yes" } else { "no" })
            });
    let mut diagnostic = diag(
        "security/ssh",
        if compliant {
            DiagnosticSeverity::Pass
        } else {
            DiagnosticSeverity::Fail
        },
        if compliant {
            "SSH policy matches desired configuration"
        } else {
            "SSH policy drifted from desired configuration"
        },
        vec![observed.to_string()],
        if compliant {
            None
        } else {
            Some("security/ssh".to_owned())
        },
    );
    if !compliant {
        diagnostic.risk = Some(RiskClass::Access);
        diagnostic.confirmation_required = true;
    }
    diagnostic
}

fn command_error(error: String) -> InspectionStatus<Value> {
    if error.contains("No such file") || error.contains("not found") {
        InspectionStatus::Unavailable { reason: error }
    } else {
        InspectionStatus::Error { message: error }
    }
}

fn parse_systemctl_active(output: &CommandOutput) -> Result<bool, String> {
    match (output.status, output.stdout.trim()) {
        (0, "active") => Ok(true),
        (3, "inactive" | "failed") => Ok(false),
        _ => Err(format!(
            "unexpected exit status {} or state {:?}",
            output.status,
            output.stdout.trim()
        )),
    }
}

fn find_executable(name: &str) -> Option<PathBuf> {
    let paths = env::var_os("PATH")?;
    env::split_paths(&paths)
        .map(|directory| directory.join(name))
        .find(|candidate| {
            let Ok(metadata) = fs::metadata(candidate) else {
                return false;
            };
            if !metadata.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        })
}

fn diag(
    id: impl Into<String>,
    severity: DiagnosticSeverity,
    summary: impl Into<String>,
    evidence: Vec<String>,
    remediation_id: Option<String>,
) -> Diagnostic {
    Diagnostic {
        id: id.into(),
        severity,
        summary: summary.into(),
        evidence,
        remediation_id,
        risk: None,
        confirmation_required: false,
    }
}

fn parse_sshd(output: &str) -> Result<Value, String> {
    let mut values = BTreeMap::<String, Value>::new();
    let mut ports = Vec::new();
    for line in output.lines() {
        if let Some((key, value)) = line.split_once(' ') {
            let value = value.trim();
            match key {
                "port" => ports.push(
                    value
                        .parse::<u16>()
                        .map_err(|_| format!("sshd -T output has invalid port {value:?}"))?,
                ),
                "passwordauthentication" | "permitrootlogin" => {
                    values.insert(key.to_owned(), json!(value));
                }
                _ => {}
            }
        }
    }
    if ports.is_empty() {
        return Err("sshd -T output missing port".to_owned());
    }
    values.insert("ports".to_owned(), json!(ports));
    for key in ["passwordauthentication", "permitrootlogin"] {
        if !values.contains_key(key) {
            return Err(format!("sshd -T output missing {key}"));
        }
    }
    serde_json::to_value(values).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct FixtureRunner(HashMap<String, Result<CommandOutput, String>>);
    impl CommandAdapter for FixtureRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
            self.0
                .get(&format!("{program} {}", args.join(" ")))
                .cloned()
                .unwrap_or_else(|| Err("No such file or directory".to_owned()))
        }
    }

    fn output(stdout: &str, status: i32) -> Result<CommandOutput, String> {
        Ok(CommandOutput {
            status,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    #[test]
    fn json_report_contract_has_stable_envelope_and_counts() {
        let observed = ObservedState {
            facts: BTreeMap::from([(
                "inspection.sample".to_owned(),
                InspectionStatus::Error {
                    message: "fixture error".to_owned(),
                },
            )]),
        };
        let diagnostics = vec![
            diag("ok", DiagnosticSeverity::Pass, "ok", vec![], None),
            diag("warning", DiagnosticSeverity::Warn, "warning", vec![], None),
            diag("drift", DiagnosticSeverity::Fail, "drift", vec![], None),
        ];
        let summary = summarize(&observed, &diagnostics);
        assert_eq!(summary.pass_count, 1);
        assert_eq!(summary.warn_count, 1);
        assert_eq!(summary.fail_count, 1);
        assert_eq!(summary.error_count, 1);
        assert_eq!(summary.drift_count, 1);

        let report = Report {
            schema_version: 1,
            command: "doctor",
            version: "v1.2.3".to_owned(),
            config: ConfigSummary {
                path: "/etc/mistborn/config.toml",
                schema_version: Some(1),
                status: "loaded",
            },
            summary,
            observed,
            diagnostics,
            inspection_incomplete: false,
        };
        let value = serde_json::to_value(report).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["command"], "doctor");
        assert_eq!(value["config"]["path"], "/etc/mistborn/config.toml");
        assert_eq!(value["config"]["schema_version"], 1);
        assert_eq!(value["config"]["status"], "loaded");
        assert_eq!(value["summary"]["drift_count"], 1);
        assert!(value["observed"].is_object());
    }

    #[test]
    fn package_lookup_does_not_spawn_a_shell() {
        assert!(find_executable("mistborn-nonexistent-package-test").is_none());
    }

    #[test]
    fn command_failure_is_observed_as_unavailable_or_error() {
        let mut fixtures = HashMap::new();
        fixtures.insert(
            "systemctl is-active docker".to_owned(),
            output("inactive\n", 3),
        );
        let report =
            inspect_with_package_lookup(ReportKind::Doctor, &FixtureRunner(fixtures), |package| {
                Some(PathBuf::from(format!("/fixture/{package}")))
            });
        assert!(
            matches!(report.observed.facts.get("service.docker"), Some(InspectionStatus::Available(value)) if value["active"] == false)
        );
    }

    #[test]
    fn systemctl_inactive_state_requires_expected_exit_code_and_text() {
        assert_eq!(
            parse_systemctl_active(&CommandOutput {
                status: 3,
                stdout: "inactive\n".to_owned(),
                stderr: String::new()
            }),
            Ok(false)
        );
        assert_eq!(
            parse_systemctl_active(&CommandOutput {
                status: 0,
                stdout: "active\n".to_owned(),
                stderr: String::new()
            }),
            Ok(true)
        );
        assert!(
            parse_systemctl_active(&CommandOutput {
                status: 4,
                stdout: "inactive\n".to_owned(),
                stderr: String::new()
            })
            .is_err()
        );
        assert!(
            parse_systemctl_active(&CommandOutput {
                status: 3,
                stdout: "garbage\n".to_owned(),
                stderr: String::new()
            })
            .is_err()
        );
    }

    #[test]
    fn ufw_parser_rejects_unknown_or_incomplete_output() {
        assert!(parse_ufw("some unrelated command output\n").is_err());
        assert!(
            parse_ufw(
                "Status: active\nDefault: deny (incoming), allow (outgoing), disabled (routed)\n"
            )
            .is_err()
        );
    }

    #[test]
    fn managed_ufw_compares_enabled_default_and_required_public_ports() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[ssh]\nport=22\npassword_authentication=false\nroot_login=false\n[firewall]\nenabled=true\ndefault_incoming='deny'\npublic_tcp_ports=[80,443]\n",
        ).unwrap();
        let fixtures = FixtureRunner(HashMap::from([(
            "ufw status verbose".to_owned(),
            output(
                "Status: active\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nTo Action From\n-- ------ ----\n22/tcp ALLOW IN Anywhere # mistborn:security/ufw:ssh\n80/tcp ALLOW IN Anywhere # mistborn:security/ufw:public\n443/tcp ALLOW IN Anywhere # mistborn:security/ufw:public\n",
                0,
            ),
        )]));
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_ufw(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Pass);

        let fixtures = FixtureRunner(HashMap::from([(
            "ufw status verbose".to_owned(),
            output(
                "Status: inactive\nDefault: allow (incoming), allow (outgoing), disabled (routed)\nTo Action From\n-- ------ ----\n",
                0,
            ),
        )]));
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_ufw(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Fail);
        assert_eq!(diagnostics[0].risk, Some(RiskClass::Access));
        assert!(diagnostics[0].confirmation_required);
    }

    #[test]
    fn ufw_public_port_matching_uses_exact_rule_targets() {
        assert!(!ufw_allows_public_tcp("180/tcp ALLOW IN Anywhere\n", 80));
        assert!(ufw_allows_public_tcp("80/tcp ALLOW IN Anywhere\n", 80));
    }

    #[test]
    fn disabled_ufw_does_not_require_allow_rules_for_convergence() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=false\ndefault_incoming='deny'\npublic_tcp_ports=[80,443]\n",
        )
        .unwrap();
        let fixtures = FixtureRunner(HashMap::from([(
            "ufw status verbose".to_owned(),
            output(
                "Status: inactive\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nTo Action From\n-- ------ ----\n",
                0,
            ),
        )]));
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_ufw(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Pass);
    }

    #[test]
    fn managed_ufw_requires_ssh_access_and_surfaces_stale_owned_ports_for_manual_cleanup() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[ssh]\nport=22\npassword_authentication=false\nroot_login=false\n[firewall]\nenabled=true\ndefault_incoming='deny'\npublic_tcp_ports=[80,443]\n",
        )
        .unwrap();
        let inspect_fixture = |status: &str| {
            let runner = FixtureRunner(HashMap::from([(
                "ufw status verbose".to_owned(),
                output(status, 0),
            )]));
            let mut observed = ObservedState::default();
            let mut diagnostics = Vec::new();
            inspect_ufw(
                &runner,
                Some(&desired),
                &mut observed,
                ReportKind::Doctor,
                &mut diagnostics,
            );
            diagnostics
        };
        let missing_ssh = inspect_fixture(
            "Status: active\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nTo Action From\n-- ------ ----\n80/tcp ALLOW IN Anywhere\n443/tcp ALLOW IN Anywhere\n",
        );
        assert!(missing_ssh[0].severity == DiagnosticSeverity::Fail);
        assert!(
            missing_ssh[0]
                .evidence
                .iter()
                .any(|item| item.contains("managed SSH TCP port 22"))
        );
        assert_eq!(
            missing_ssh[0].remediation_id.as_deref(),
            Some("security/ufw")
        );

        let stale = inspect_fixture(
            "Status: active\nDefault: deny (incoming), allow (outgoing), disabled (routed)\nTo Action From\n-- ------ ----\n22/tcp ALLOW IN Anywhere # mistborn:security/ufw:ssh\n80/tcp ALLOW IN Anywhere # mistborn:security/ufw:public\n443/tcp ALLOW IN Anywhere # mistborn:security/ufw:public\n8080/tcp ALLOW IN Anywhere # mistborn:security/ufw:public\n",
        );
        assert_eq!(stale[0].severity, DiagnosticSeverity::Fail);
        assert_eq!(stale[0].remediation_id, None);
        assert!(
            stale
                .iter()
                .any(|item| item.id == "security/ufw/manual-cleanup")
        );
        assert!(
            !stale_ufw_managed_ports(
                "8081/tcp ALLOW IN Anywhere\n",
                desired.firewall.as_ref().unwrap(),
                Some(&desired)
            )
            .iter()
            .any(|issue| issue.contains("8081"))
        );
    }

    #[test]
    fn optional_ufw_unavailability_does_not_mark_the_report_incomplete() {
        let fixtures = FixtureRunner(HashMap::new());
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_ufw(
            &fixtures,
            None,
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert!(matches!(
            observed.facts.get("firewall.ufw"),
            Some(InspectionStatus::Unavailable { .. })
        ));
        assert!(!diagnostics.is_empty());
    }

    #[test]
    fn missing_ufw_package_leaves_policy_unavailable_without_inventing_security_drift() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=true\ndefault_incoming='deny'\npublic_tcp_ports=[80,443]\n[firewall.plex]\nenabled=true\npublic_remote_access=true\ntailscale=true\n",
        ).unwrap();
        let fixtures = FixtureRunner(HashMap::new());
        let mut observed = ObservedState::default();
        observed.facts.insert(
            "package.ufw".into(),
            InspectionStatus::Available(json!({"installed": false, "required": true})),
        );
        let mut diagnostics = Vec::new();
        assert!(!inspect_ufw(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert!(matches!(
            observed.facts.get("firewall.policy"),
            Some(InspectionStatus::Unavailable { .. })
        ));
        assert!(
            !diagnostics.iter().any(
                |item| item.remediation_id.as_deref() == Some("security/ufw")
                    || item.remediation_id.as_deref() == Some("security/plex-firewall")
            )
        );
    }

    #[test]
    fn plex_effective_profile_and_rules_are_compared() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=true\ndefault_incoming='deny'\n[firewall.plex]\nenabled=true\npublic_remote_access=true\nlan_cidr='192.168.1.0/24'\ntailscale=true\n",
        ).unwrap();
        let plex = desired.firewall.unwrap().plex.unwrap();
        let status = "To                         Action      From\n--                         ------      ----\n32400/tcp                  ALLOW IN    Anywhere                  # mistborn:security/plex-firewall:public\nplexmediaserver-all        ALLOW IN    192.168.1.0/24             # mistborn:security/plex-firewall:lan\nplexmediaserver-all on tailscale0 ALLOW IN Anywhere             # mistborn:security/plex-firewall:tailscale\n";
        assert!(
            compare_plex_profile_and_rules(Some(PLEX_UFW_PROFILE), status, &plex)
                .issues
                .is_empty()
        );
        assert!(
            !compare_plex_profile_and_rules(
                Some(PLEX_UFW_PROFILE),
                "32400/tcp ALLOW IN Anywhere\n",
                &plex
            )
            .issues
            .is_empty()
        );
    }

    #[test]
    fn plex_profile_requires_exact_standard_dlna_and_all_port_sets() {
        assert!(plex_profile_is_expected(PLEX_UFW_PROFILE));
        assert!(plex_profile_is_expected(
            &PLEX_UFW_PROFILE.replace('|', ",")
        ));
        let missing_dlna = PLEX_UFW_PROFILE.replace("1900/udp|32469/tcp", "1900/udp");
        assert!(!plex_profile_is_expected(&missing_dlna));
        let extra_public_service = PLEX_UFW_PROFILE.replace(
            "ports=32400/tcp|3005/tcp",
            "ports=32400/tcp|22/tcp|3005/tcp",
        );
        assert!(!plex_profile_is_expected(&extra_public_service));
    }

    #[test]
    fn plex_app_scope_must_match_exact_profile_source_and_interface() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=true\ndefault_incoming='deny'\n[firewall.plex]\nenabled=true\npublic_remote_access=true\nlan_cidr='192.168.1.0/24'\ntailscale=true\n",
        )
        .unwrap();
        let plex = desired.firewall.unwrap().plex.unwrap();
        let mismatched_app = "32400/tcp ALLOW IN Anywhere\nplexmediaserver ALLOW IN 192.168.1.0/24\nplexmediaserver-all on eth0 ALLOW IN Anywhere\n";
        let issues = compare_plex_profile_and_rules(Some(PLEX_UFW_PROFILE), mismatched_app, &plex);
        assert!(
            issues
                .issues
                .iter()
                .any(|issue| issue.contains("LAN plexmediaserver-all"))
        );
        assert!(
            issues
                .issues
                .iter()
                .any(|issue| issue.contains("tailscale0"))
        );
        let overbroad = "32400/tcp ALLOW IN Anywhere\nplexmediaserver-all ALLOW IN Anywhere\nplexmediaserver-all ALLOW IN 192.168.1.0/24\nplexmediaserver-all on tailscale0 ALLOW IN Anywhere\n";
        assert!(
            compare_plex_profile_and_rules(Some(PLEX_UFW_PROFILE), overbroad, &plex)
                .issues
                .iter()
                .any(|issue| issue.contains("all public interfaces"))
        );
        let raw_plex_port = "32400/tcp ALLOW IN Anywhere\n3005/tcp ALLOW IN Anywhere\n";
        let raw_exposure =
            compare_plex_profile_and_rules(Some(PLEX_UFW_PROFILE), raw_plex_port, &plex);
        assert!(raw_exposure.requires_manual_cleanup);
        assert!(
            raw_exposure
                .issues
                .iter()
                .any(|issue| issue.contains("Plex profile port 3005/tcp"))
        );
        let raw_plex_udp = "32400/tcp ALLOW IN Anywhere\n32412/udp ALLOW IN Anywhere\n";
        assert!(
            compare_plex_profile_and_rules(Some(PLEX_UFW_PROFILE), raw_plex_udp, &plex)
                .requires_manual_cleanup
        );

        let no_local_scopes = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=true\ndefault_incoming='deny'\n[firewall.plex]\nenabled=true\npublic_remote_access=true\ntailscale=false\n",
        )
        .unwrap();
        let no_local_scopes = no_local_scopes.firewall.unwrap().plex.unwrap();
        let stale_scope = "32400/tcp ALLOW IN Anywhere\nplexmediaserver-all ALLOW IN 192.168.1.0/24\nplexmediaserver-all on tailscale0 ALLOW IN Anywhere\n";
        let stale =
            compare_plex_profile_and_rules(Some(PLEX_UFW_PROFILE), stale_scope, &no_local_scopes);
        assert!(stale.requires_manual_cleanup);
        assert!(
            stale
                .issues
                .iter()
                .any(|issue| issue.contains("unexpected plexmediaserver-all app allow rule from"))
        );
        assert!(stale.issues.iter().any(|issue| {
            issue.contains("unexpected plexmediaserver-all app allow rule on interface tailscale0")
        }));
    }

    #[test]
    fn disabled_plex_rejects_any_allow_rule() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[firewall]\nenabled=true\ndefault_incoming='deny'\n[firewall.plex]\nenabled=false\npublic_remote_access=false\ntailscale=false\n",
        ).unwrap();
        let plex = desired.firewall.unwrap().plex.unwrap();
        assert!(
            compare_plex_profile_and_rules(None, "", &plex)
                .issues
                .is_empty()
        );
        assert!(
            !compare_plex_profile_and_rules(None, "32400/tcp ALLOW IN Anywhere\n", &plex)
                .issues
                .is_empty()
        );
    }

    #[test]
    fn ssh_access_drift_requires_explicit_confirmation() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[ssh]\nport=22\npassword_authentication=false\nroot_login=false\n",
        ).unwrap();
        let diagnostic = compare_ssh_policy(
            &json!({"ports":[2222],"passwordauthentication":"yes","permitrootlogin":"yes"}),
            desired.ssh.as_ref().unwrap(),
        );
        assert_eq!(diagnostic.severity, DiagnosticSeverity::Fail);
        assert_eq!(diagnostic.remediation_id.as_deref(), Some("security/ssh"));
        assert_eq!(diagnostic.risk, Some(RiskClass::Access));
        assert!(diagnostic.confirmation_required);
    }

    #[test]
    fn tailscale_interface_rule_is_not_mistaken_for_a_public_rule() {
        assert!(!ufw_allows_public_tcp(
            "32400/tcp on tailscale0 ALLOW IN Anywhere\\n",
            32400
        ));
    }

    #[test]
    fn unavailable_unmanaged_tailscale_does_not_make_inspection_incomplete() {
        let fixtures = FixtureRunner(HashMap::new());
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_tailscale(
            &fixtures,
            None,
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert!(diagnostics.iter().any(
            |item| item.id == "security/tailscale" && item.severity == DiagnosticSeverity::Warn
        ));
    }

    #[test]
    fn unavailable_managed_tailscale_is_incomplete_and_reported() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[tailscale]\nssh=true\nadvertise_exit_node=false\nauto_update=true\n",
        ).unwrap();
        let fixtures = FixtureRunner(HashMap::new());
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(inspect_tailscale(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert!(
            diagnostics
                .iter()
                .any(|item| item.id == "inspection/tailscale/status")
        );
    }

    #[test]
    fn tailscale_requires_running_backend_and_never_reads_preferences_when_offline() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[tailscale]\nssh=true\nadvertise_exit_node=false\nauto_update=true\n",
        )
        .unwrap();
        let fixtures = FixtureRunner(HashMap::from([(
            "tailscale status --json".to_owned(),
            output("{\"BackendState\":\"NeedsLogin\"}", 0),
        )]));
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(inspect_tailscale(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics,
        ));
        assert!(matches!(
            observed.facts.get("tailscale.status"),
            Some(InspectionStatus::Unavailable { reason }) if reason.contains("NeedsLogin")
        ));
        assert!(matches!(
            observed.facts.get("tailscale.ssh"),
            Some(InspectionStatus::Unavailable { .. })
        ));
        assert!(diagnostics.iter().all(|item| item.remediation_id.is_none()));
    }

    #[test]
    fn missing_tailscale_package_leaves_policy_unavailable_without_security_drift() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[tailscale]\nssh=true\nadvertise_exit_node=false\nauto_update=true\n",
        ).unwrap();
        let fixtures = FixtureRunner(HashMap::new());
        let mut observed = ObservedState::default();
        observed.facts.insert(
            "package.tailscale".into(),
            InspectionStatus::Available(json!({"installed": false, "required": true})),
        );
        let mut diagnostics = Vec::new();
        assert!(!inspect_tailscale(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert!(matches!(
            observed.facts.get("tailscale.ssh"),
            Some(InspectionStatus::Unavailable { .. })
        ));
        assert!(!diagnostics.iter().any(|item| {
            item.remediation_id
                .as_deref()
                .is_some_and(|id| id.starts_with("security/tailscale-"))
        }));
    }

    #[test]
    fn missing_required_package_is_available_drift_not_incomplete_inspection() {
        let report =
            inspect_with_package_lookup(ReportKind::Doctor, &FixtureRunner(HashMap::new()), |_| {
                None
            });
        assert!(!report.inspection_incomplete);
        assert!(matches!(
            report.observed.facts.get("package.docker"),
            Some(InspectionStatus::Available(value)) if value["installed"] == false
        ));
        assert!(report.diagnostics.iter().any(|item| {
            item.id == "package/docker"
                && item.severity == DiagnosticSeverity::Fail
                && item.remediation_id.as_deref() == Some("packages/docker")
        }));
        assert!(matches!(
            report.observed.facts.get("service.docker"),
            Some(InspectionStatus::Unavailable { .. })
        ));
        assert!(matches!(
            report.observed.facts.get("firewall.policy"),
            Some(InspectionStatus::Unavailable { .. })
        ));
    }

    #[test]
    fn missing_fail2ban_package_is_planned_before_service_enable() {
        let report = inspect_with_package_lookup(
            ReportKind::Doctor,
            &FixtureRunner(HashMap::new()),
            |name| (name != "fail2ban-client").then(|| PathBuf::from("/usr/bin").join(name)),
        );
        assert!(report.diagnostics.iter().any(|item| {
            item.id == "package/fail2ban-client"
                && item.severity == DiagnosticSeverity::Warn
                && item.remediation_id.as_deref() == Some("packages/fail2ban-client")
        }));
        assert!(matches!(
            report.observed.facts.get("service.fail2ban"),
            Some(InspectionStatus::Unavailable { .. })
        ));
        assert!(
            !report
                .diagnostics
                .iter()
                .any(|item| { item.remediation_id.as_deref() == Some("security/fail2ban") })
        );
        let plan = mistborn_bootstrap::reconciliation::plan(
            &serde_json::json!({}),
            &report.diagnostics,
            Some("security/fail2ban".parse().unwrap()),
        )
        .unwrap();
        assert_eq!(plan.remediations.len(), 1);
        assert_eq!(
            plan.remediations[0].id.to_string(),
            "packages/fail2ban-client"
        );
    }

    #[test]
    fn all_unavailable_facts_receive_structured_diagnostics() {
        let observed = ObservedState {
            facts: BTreeMap::from([(
                "tailscale.preferences".to_owned(),
                InspectionStatus::Unavailable {
                    reason: "missing".to_owned(),
                },
            )]),
        };
        let mut diagnostics = Vec::new();
        add_inspection_diagnostics(&observed, &mut diagnostics);
        assert_eq!(diagnostics[0].id, "inspection/tailscale/preferences");
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Warn);
    }

    #[test]
    fn malformed_ssh_output_cannot_become_a_pass() {
        let result = parse_sshd("port 22\npasswordauthentication no\n");
        assert!(result.unwrap_err().contains("permitrootlogin"));
    }

    #[test]
    fn ssh_policy_parser_extracts_only_supported_fields() {
        let parsed = parse_sshd(
            "port 2222\npasswordauthentication yes\npermitrootlogin prohibit-password\n",
        )
        .unwrap();
        assert_eq!(parsed["ports"], json!([2222]));
        assert_eq!(parsed["passwordauthentication"], "yes");
    }

    #[test]
    fn ssh_policy_preserves_all_effective_ports_and_flags_extras_as_drift() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[ssh]\nport=22\npassword_authentication=false\nroot_login=false\n",
        )
        .unwrap();
        let parsed =
            parse_sshd("port 22\nport 2222\npasswordauthentication no\npermitrootlogin no\n")
                .unwrap();
        assert_eq!(parsed["ports"], json!([22, 2222]));
        assert_eq!(
            compare_ssh_policy(&parsed, desired.ssh.as_ref().unwrap()).severity,
            DiagnosticSeverity::Fail
        );
        let duplicate_ports =
            parse_sshd("port 22\nport 22\npasswordauthentication no\npermitrootlogin no\n")
                .unwrap();
        assert_eq!(duplicate_ports["ports"], json!([22, 22]));
        assert_eq!(
            compare_ssh_policy(&duplicate_ports, desired.ssh.as_ref().unwrap()).severity,
            DiagnosticSeverity::Fail
        );
    }

    #[test]
    fn malformed_tailscale_json_is_an_inspection_error() {
        let mut fixtures = HashMap::new();
        fixtures.insert("tailscale status --json".to_owned(), output("not-json", 0));
        let report =
            inspect_with_package_lookup(ReportKind::Doctor, &FixtureRunner(fixtures), |package| {
                Some(PathBuf::from(format!("/fixture/{package}")))
            });
        assert!(matches!(
            report.observed.facts.get("tailscale.status"),
            Some(InspectionStatus::Error { message }) if message.contains("malformed JSON")
        ));
    }

    #[test]
    fn managed_tailscale_preferences_are_compared_field_by_field() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[tailscale]\nssh=true\nadvertise_exit_node=false\nauto_update=true\n",
        ).unwrap();
        let fixtures = FixtureRunner(HashMap::from([
            (
                "tailscale status --json".to_owned(),
                output("{\"BackendState\":\"Running\"}", 0),
            ),
            (
                "tailscale get --json".to_owned(),
                output(
                    "{\"ssh\":true,\"advertise-exit-node\":false,\"auto-update\":true,\"hostname\":\"must-not-be-retained\"}",
                    0,
                ),
            ),
        ]));
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_tailscale(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert_eq!(diagnostics.len(), 3);
        assert!(
            diagnostics
                .iter()
                .all(|item| item.severity == DiagnosticSeverity::Pass)
        );
        assert!(matches!(
            observed.facts.get("tailscale.preferences"),
            Some(InspectionStatus::Available(value)) if value.get("hostname").is_none()
        ));

        let fixtures = FixtureRunner(HashMap::from([
            (
                "tailscale status --json".to_owned(),
                output("{\"BackendState\":\"Running\"}", 0),
            ),
            (
                "tailscale get --json".to_owned(),
                output(
                    "{\"ssh\":false,\"advertise-exit-node\":true,\"auto-update\":false}",
                    0,
                ),
            ),
        ]));
        let mut observed = ObservedState::default();
        let mut diagnostics = Vec::new();
        assert!(!inspect_tailscale(
            &fixtures,
            Some(&desired),
            &mut observed,
            ReportKind::Doctor,
            &mut diagnostics
        ));
        assert!(diagnostics[0].id == "security/tailscale-ssh");
        let ssh = diagnostics
            .iter()
            .find(|item| item.id == "security/tailscale-ssh")
            .unwrap();
        let exit_node = diagnostics
            .iter()
            .find(|item| item.id == "security/tailscale-exit-node")
            .unwrap();
        let auto_update = diagnostics
            .iter()
            .find(|item| item.id == "security/tailscale-auto-update")
            .unwrap();
        assert_eq!(ssh.risk, Some(RiskClass::Access));
        assert_eq!(exit_node.risk, Some(RiskClass::Access));
        assert!(ssh.confirmation_required && exit_node.confirmation_required);
        assert_eq!(auto_update.risk, Some(RiskClass::Moderate));
        assert!(!auto_update.confirmation_required);
        assert!(
            diagnostics
                .iter()
                .all(|item| item.severity == DiagnosticSeverity::Fail)
        );
    }

    #[test]
    fn tailscale_single_preference_drift_has_only_its_bounded_remediation() {
        let desired = mistborn_bootstrap::config::DesiredState::from_toml(
            "version=1\nprofile='vps'\n[tailscale]\nssh=true\nadvertise_exit_node=false\nauto_update=true\n",
        )
        .unwrap();
        for (json, expected_id) in [
            (
                r#"{"ssh":false,"advertise-exit-node":false,"auto-update":true}"#,
                "security/tailscale-ssh",
            ),
            (
                r#"{"ssh":true,"advertise-exit-node":true,"auto-update":true}"#,
                "security/tailscale-exit-node",
            ),
            (
                r#"{"ssh":true,"advertise-exit-node":false,"auto-update":false}"#,
                "security/tailscale-auto-update",
            ),
        ] {
            let fixtures = FixtureRunner(HashMap::from([
                (
                    "tailscale status --json".to_owned(),
                    output("{\"BackendState\":\"Running\"}", 0),
                ),
                ("tailscale get --json".to_owned(), output(json, 0)),
            ]));
            let mut observed = ObservedState::default();
            let mut diagnostics = Vec::new();
            assert!(!inspect_tailscale(
                &fixtures,
                Some(&desired),
                &mut observed,
                ReportKind::Doctor,
                &mut diagnostics,
            ));
            let drifted = diagnostics
                .iter()
                .filter(|item| item.severity == DiagnosticSeverity::Fail)
                .collect::<Vec<_>>();
            assert_eq!(drifted.len(), 1);
            assert_eq!(drifted[0].id, expected_id);
            assert_eq!(drifted[0].remediation_id.as_deref(), Some(expected_id));
        }
    }

    #[test]
    fn docker_inspection_never_invokes_daemon_connecting_commands() {
        use std::sync::Mutex;
        struct RecordingRunner(Mutex<Vec<String>>);
        impl CommandAdapter for RecordingRunner {
            fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
                self.0
                    .lock()
                    .unwrap()
                    .push(format!("{program} {}", args.join(" ")));
                Err("No such file or directory".to_owned())
            }
        }
        let runner = RecordingRunner(Mutex::new(Vec::new()));
        let _ = inspect(ReportKind::Doctor, &runner);
        let calls = runner.0.lock().unwrap();
        assert!(!calls.iter().any(|call| call == "docker info"));
        assert!(!calls.iter().any(|call| call.starts_with("sh -c")));
    }
}
