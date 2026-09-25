//! Narrow adapters for additive apt package installation and safe systemd activation.
//!
//! This module intentionally has no operation for package removal, service stop,
//! disable, restart, or arbitrary command execution.

use mistborn_bootstrap::domain::{PackageName, ServiceName};

pub trait SystemRunner {
    fn run(&mut self, program: &str, arguments: &[&str]) -> Result<CommandOutput, String>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
}

pub struct ProcessRunner;

impl SystemRunner for ProcessRunner {
    fn run(&mut self, program: &str, arguments: &[&str]) -> Result<CommandOutput, String> {
        let output = std::process::Command::new(program)
            .args(arguments)
            .env("LC_ALL", "C")
            .output()
            .map_err(|error| format!("cannot start {program}: {error}"))?;
        Ok(CommandOutput {
            status: output.status.code().unwrap_or(128),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub struct PackageServiceAdapter<R> {
    pub runner: R,
}

impl<R: SystemRunner> PackageServiceAdapter<R> {
    /// Install only the closed-set package selected by the typed plan. Existing
    /// packages are a no-op, and apt is forbidden from removing packages.
    pub fn install_package(&mut self, package: PackageName) -> Result<(), String> {
        let name = package_name(package);
        let query = self
            .runner
            .run("dpkg-query", &["-W", "-f=${db:Status-Status}", name])?;
        match query.status {
            0 if query.stdout.trim() == "installed" => return Ok(()),
            0 if matches!(query.stdout.trim(), "not-installed" | "config-files") => {}
            1 if explicit_not_found(&query.stderr, name) => {}
            status => {
                return Err(format!(
                    "dpkg-query could not determine {name} status (exit {status}): {}",
                    query.stderr.trim()
                ));
            }
        }

        let output = self.runner.run(
            "apt-get",
            &[
                "install",
                "-y",
                "--no-install-recommends",
                "--no-upgrade",
                "--no-remove",
                name,
            ],
        )?;
        if output.status == 0 {
            Ok(())
        } else {
            Err(format!(
                "apt-get install {name} exited with {}: {}",
                output.status,
                output.stderr.trim()
            ))
        }
    }

    /// Enable and start a known, ordinary service. Refuses units whose state is
    /// not known to be safely manageable. Never stops, disables, or restarts.
    pub fn enable_and_start(&mut self, service: ServiceName) -> Result<(), String> {
        let unit = service_name(service);
        let load_state = show_value(&mut self.runner, unit, "LoadState")?;
        if load_state != "loaded" {
            return Err(format!(
                "{unit} is not a loaded systemd unit ({load_state})"
            ));
        }
        let unit_file_state = show_value(&mut self.runner, unit, "UnitFileState")?;
        if matches!(
            unit_file_state.as_str(),
            "masked" | "masked-runtime" | "static" | "indirect" | "generated" | "transient"
        ) {
            return Err(format!(
                "{unit} cannot be safely enabled (UnitFileState={unit_file_state})"
            ));
        }
        if !matches!(unit_file_state.as_str(), "enabled" | "disabled") {
            return Err(format!(
                "{unit} has unexpected UnitFileState={unit_file_state}"
            ));
        }
        let active_state = show_value(&mut self.runner, unit, "ActiveState")?;
        if !matches!(active_state.as_str(), "active" | "inactive" | "failed") {
            return Err(format!("{unit} has unexpected ActiveState={active_state}"));
        }

        if unit_file_state == "disabled" {
            run_checked(&mut self.runner, "systemctl", &["enable", unit])?;
        }
        if active_state != "active" {
            run_checked(&mut self.runner, "systemctl", &["start", unit])?;
        }
        Ok(())
    }
}

fn show_value<R: SystemRunner>(
    runner: &mut R,
    unit: &str,
    property: &str,
) -> Result<String, String> {
    let argument = format!("--property={property}");
    let output = runner.run("systemctl", &["show", &argument, "--value", unit])?;
    if output.status != 0 {
        return Err(format!(
            "systemctl show {unit} {property} failed (exit {}): {}",
            output.status,
            output.stderr.trim()
        ));
    }
    let value = output.stdout.trim();
    if value.is_empty() || value.lines().count() != 1 {
        return Err(format!(
            "systemctl returned an invalid {property} for {unit}"
        ));
    }
    Ok(value.to_owned())
}

fn run_checked<R: SystemRunner>(
    runner: &mut R,
    program: &str,
    arguments: &[&str],
) -> Result<(), String> {
    let output = runner.run(program, arguments)?;
    if output.status == 0 {
        Ok(())
    } else {
        Err(format!(
            "{} exited with {}: {}",
            program,
            output.status,
            output.stderr.trim()
        ))
    }
}

fn package_name(package: PackageName) -> &'static str {
    match package {
        PackageName::Docker => "docker.io",
        PackageName::Tailscale => "tailscale",
        PackageName::Ufw => "ufw",
        PackageName::Fail2ban => "fail2ban",
    }
}

fn explicit_not_found(stderr: &str, package: &str) -> bool {
    let message = stderr.trim();
    message.strip_prefix("dpkg-query: no packages found matching ") == Some(package)
}

fn service_name(service: ServiceName) -> &'static str {
    match service {
        ServiceName::Docker => "docker",
        ServiceName::Tailscaled => "tailscaled",
        ServiceName::Ufw => "ufw",
        ServiceName::Fail2ban => "fail2ban",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct FixtureRunner {
        outputs: VecDeque<CommandOutput>,
        calls: Vec<(String, Vec<String>)>,
    }

    impl FixtureRunner {
        fn new(outputs: impl IntoIterator<Item = CommandOutput>) -> Self {
            Self {
                outputs: outputs.into_iter().collect(),
                calls: Vec::new(),
            }
        }

        fn ok(stdout: &str) -> CommandOutput {
            CommandOutput {
                status: 0,
                stdout: stdout.to_owned(),
                stderr: String::new(),
            }
        }
    }

    impl SystemRunner for FixtureRunner {
        fn run(&mut self, program: &str, arguments: &[&str]) -> Result<CommandOutput, String> {
            self.calls.push((
                program.to_owned(),
                arguments.iter().map(|arg| (*arg).to_owned()).collect(),
            ));
            self.outputs
                .pop_front()
                .ok_or_else(|| "unexpected command".to_owned())
        }
    }

    #[test]
    fn installed_package_is_not_reinstalled() {
        let runner = FixtureRunner::new([FixtureRunner::ok("installed\n")]);
        let mut adapter = PackageServiceAdapter { runner };
        adapter.install_package(PackageName::Docker).unwrap();
        assert_eq!(adapter.runner.calls.len(), 1);
        assert_eq!(adapter.runner.calls[0].0, "dpkg-query");
    }

    #[test]
    fn missing_package_uses_add_only_apt_flags() {
        let mut missing = FixtureRunner::ok("");
        missing.status = 1;
        missing.stderr = "dpkg-query: no packages found matching ufw".to_owned();
        let runner = FixtureRunner::new([missing, FixtureRunner::ok("")]);
        let mut adapter = PackageServiceAdapter { runner };
        adapter.install_package(PackageName::Ufw).unwrap();
        assert_eq!(
            adapter.runner.calls[1],
            (
                "apt-get".into(),
                vec![
                    "install".into(),
                    "-y".into(),
                    "--no-install-recommends".into(),
                    "--no-upgrade".into(),
                    "--no-remove".into(),
                    "ufw".into(),
                ]
            )
        );
    }

    #[test]
    fn package_query_errors_fail_closed_without_installing() {
        let mut query_error = FixtureRunner::ok("");
        query_error.status = 1;
        query_error.stderr = "dpkg-query: error: cannot read database".to_owned();
        let mut adapter = PackageServiceAdapter {
            runner: FixtureRunner::new([query_error]),
        };
        assert!(
            adapter
                .install_package(PackageName::Docker)
                .unwrap_err()
                .contains("could not determine")
        );
        assert_eq!(adapter.runner.calls.len(), 1);
    }

    #[test]
    fn empty_successful_package_query_fails_closed_without_installing() {
        let mut adapter = PackageServiceAdapter {
            runner: FixtureRunner::new([FixtureRunner::ok("")]),
        };
        assert!(
            adapter
                .install_package(PackageName::Docker)
                .unwrap_err()
                .contains("could not determine")
        );
        assert_eq!(adapter.runner.calls.len(), 1);
    }

    #[test]
    fn package_install_failure_is_not_swallowed() {
        let mut missing = FixtureRunner::ok("");
        missing.status = 1;
        missing.stderr = "dpkg-query: no packages found matching fail2ban".to_owned();
        let mut failed = FixtureRunner::ok("");
        failed.status = 100;
        let mut adapter = PackageServiceAdapter {
            runner: FixtureRunner::new([missing, failed]),
        };
        assert!(
            adapter
                .install_package(PackageName::Fail2ban)
                .unwrap_err()
                .contains("exited with 100")
        );
    }

    #[test]
    fn service_activation_enables_then_starts_only_when_needed() {
        let runner = FixtureRunner::new([
            FixtureRunner::ok("loaded\n"),
            FixtureRunner::ok("disabled\n"),
            FixtureRunner::ok("inactive\n"),
            FixtureRunner::ok(""),
            FixtureRunner::ok(""),
        ]);
        let mut adapter = PackageServiceAdapter { runner };
        adapter.enable_and_start(ServiceName::Fail2ban).unwrap();
        assert_eq!(
            adapter.runner.calls[3],
            ("systemctl".into(), vec!["enable".into(), "fail2ban".into()])
        );
        assert_eq!(
            adapter.runner.calls[4],
            ("systemctl".into(), vec!["start".into(), "fail2ban".into()])
        );
    }

    #[test]
    fn service_activation_refuses_masked_units_without_mutation() {
        let runner =
            FixtureRunner::new([FixtureRunner::ok("loaded\n"), FixtureRunner::ok("masked\n")]);
        let mut adapter = PackageServiceAdapter { runner };
        assert!(
            adapter
                .enable_and_start(ServiceName::Fail2ban)
                .unwrap_err()
                .contains("cannot be safely enabled")
        );
        assert_eq!(adapter.runner.calls.len(), 2);
    }

    #[test]
    fn service_activation_refuses_runtime_only_enablement() {
        let runner = FixtureRunner::new([
            FixtureRunner::ok("loaded\n"),
            FixtureRunner::ok("enabled-runtime\n"),
        ]);
        let mut adapter = PackageServiceAdapter { runner };
        assert!(
            adapter
                .enable_and_start(ServiceName::Fail2ban)
                .unwrap_err()
                .contains("unexpected UnitFileState")
        );
        assert_eq!(adapter.runner.calls.len(), 2);
    }

    #[test]
    fn already_healthy_service_is_a_noop() {
        let runner = FixtureRunner::new([
            FixtureRunner::ok("loaded\n"),
            FixtureRunner::ok("enabled\n"),
            FixtureRunner::ok("active\n"),
        ]);
        let mut adapter = PackageServiceAdapter { runner };
        adapter.enable_and_start(ServiceName::Docker).unwrap();
        assert_eq!(adapter.runner.calls.len(), 3);
    }
}
