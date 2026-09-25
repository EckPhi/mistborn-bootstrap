//! Narrow, fail-closed SSH policy adapter used by `security/ssh`.
//!
//! This module owns exactly one drop-in. It never edits the primary sshd
//! configuration, firewall, or Tailscale policy.

use mistborn_bootstrap::config::SshConfig;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const MARKER: &str = "# Managed by Mistborn: security/ssh";
const DROPIN: &str = "00-mistborn.conf";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub trait Runner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String>;
}

pub struct SystemRunner;
impl Runner for SystemRunner {
    fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| format!("cannot start {program}: {error}"))?;
        Ok(CommandResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

pub struct SshAdapter<R> {
    pub root: PathBuf,
    pub runner: R,
}

impl<R: Runner> SshAdapter<R> {
    fn primary(&self) -> PathBuf {
        self.root.join("etc/ssh/sshd_config")
    }
    fn dropin(&self) -> PathBuf {
        self.root.join("etc/ssh/sshd_config.d").join(DROPIN)
    }

    /// Apply an already explicitly confirmed target. The caller's reconciliation
    /// engine enforces per-remediation confirmation before reaching this method.
    pub fn apply(&mut self, desired: &SshConfig) -> Result<(), String> {
        let primary = self.primary();
        ensure_no_symlink_ancestors(&self.root, &self.root.join("etc/ssh"))?;
        let primary_meta = fs::symlink_metadata(&primary)
            .map_err(|error| format!("cannot inspect {}: {error}", primary.display()))?;
        if primary_meta.file_type().is_symlink() {
            return Err(format!(
                "refusing symlink primary SSH config {}",
                primary.display()
            ));
        }
        let config = fs::read_to_string(&primary)
            .map_err(|error| format!("cannot inspect {}: {error}", primary.display()))?;
        let dropin = self.dropin();
        validate_include_order(&config, &dropin)?;
        validate_dropin_directory(dropin.parent().expect("drop-in parent"))?;
        let current = self.runner.run("sshd", &["-T"])?;
        if !current.success {
            return Err(format!(
                "cannot inspect current effective SSH policy before applying: {}",
                current.stderr.trim()
            ));
        }
        let current_ports = parse_ports(&current.stdout)?;
        if !current_ports.contains(&desired.port.get()) {
            self.ensure_port_open(desired.port.get())?;
        }
        let service = self.resolve_service()?;
        let active = self
            .runner
            .run("systemctl", &["is-active", "--quiet", &service])?;
        if !active.success {
            return Err(format!(
                "{service} is not active; refusing to replace SSH policy before a reload can be verified"
            ));
        }

        let old = match fs::symlink_metadata(&dropin) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!("refusing symlink SSH drop-in {}", dropin.display()));
            }
            Ok(_) => {
                let text = fs::read_to_string(&dropin)
                    .map_err(|error| format!("cannot inspect {}: {error}", dropin.display()))?;
                if !text.lines().any(|line| line.trim() == MARKER) {
                    return Err(format!(
                        "refusing to replace unowned SSH config {}",
                        dropin.display()
                    ));
                }
                Some(text)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(format!("cannot inspect {}: {error}", dropin.display())),
        };
        let rendered = render(desired);
        atomic_write(&dropin, rendered.as_bytes())?;

        let validation = self.runner.run("sshd", &["-t"]);
        let validation = match validation {
            Ok(result) if result.success => Ok(()),
            Ok(result) => Err(format!(
                "sshd -t rejected the managed drop-in: {}",
                result.stderr.trim()
            )),
            Err(error) => Err(error),
        };
        if let Err(error) = validation {
            return Err(rollback(&dropin, old.as_deref(), error));
        }

        let effective = match self.runner.run("sshd", &["-T"]) {
            Ok(result) if result.success => parse_effective(&result.stdout, desired),
            Ok(result) => Err(format!("sshd -T failed: {}", result.stderr.trim())),
            Err(error) => Err(error),
        };
        if let Err(error) = effective {
            return Err(rollback(&dropin, old.as_deref(), error));
        }

        match self.runner.run("systemctl", &["reload", &service]) {
            Ok(result) if result.success => {}
            Ok(result) => {
                return Err(self.rollback_after_reload(
                    &dropin,
                    old.as_deref(),
                    &service,
                    format!(
                        "systemctl reload {service} failed: {}",
                        result.stderr.trim()
                    ),
                ));
            }
            Err(error) => {
                return Err(self.rollback_after_reload(&dropin, old.as_deref(), &service, error));
            }
        }
        let effective = match self.runner.run("sshd", &["-T"]) {
            Ok(result) if result.success => parse_effective(&result.stdout, desired),
            Ok(result) => Err(format!(
                "post-reload sshd -T failed: {}",
                result.stderr.trim()
            )),
            Err(error) => Err(error),
        };
        match effective {
            Ok(()) => Ok(()),
            Err(error) => Err(self.rollback_after_reload(&dropin, old.as_deref(), &service, error)),
        }
    }

    fn ensure_port_open(&mut self, port: u16) -> Result<(), String> {
        let result = self.runner.run("ufw", &["status"])?;
        if !result.success {
            return Err(format!(
                "cannot establish UFW access before SSH change: {}",
                result.stderr.trim()
            ));
        }
        let status = result.stdout.to_ascii_lowercase();
        if status.lines().any(|line| line.trim() == "status: inactive") {
            return Ok(());
        }
        if !status.lines().any(|line| line.trim() == "status: active") {
            return Err("UFW status output is incomplete or ambiguous; cannot prove the desired SSH port is reachable".to_owned());
        }
        if ufw_allows_port(&result.stdout, port) {
            Ok(())
        } else {
            Err(format!(
                "UFW is active and does not allow desired SSH port {port}/tcp; reconcile security/ufw first (the SSH adapter will not change UFW)"
            ))
        }
    }

    fn resolve_service(&mut self) -> Result<String, String> {
        for name in ["ssh.service", "sshd.service"] {
            let result = self.runner.run(
                "systemctl",
                &["show", "--property=LoadState", "--value", name],
            )?;
            if result.success && result.stdout.trim() == "loaded" {
                return Ok(name.to_owned());
            }
        }
        Err("cannot resolve a loaded ssh.service or sshd.service; refusing SSH reload".to_owned())
    }

    fn rollback_after_reload(
        &mut self,
        path: &Path,
        old: Option<&str>,
        service: &str,
        reason: String,
    ) -> String {
        let restored = rollback(path, old, reason);
        if restored.contains("CRITICAL: SSH drop-in rollback failed") {
            return restored;
        }
        match self.runner.run("systemctl", &["reload", service]) {
            Ok(result) if result.success => format!("{restored}; restored SSH policy reloaded"),
            Ok(result) => format!(
                "{restored}; CRITICAL: rollback reload of {service} failed: {}",
                result.stderr.trim()
            ),
            Err(error) => {
                format!("{restored}; CRITICAL: rollback reload of {service} failed: {error}")
            }
        }
    }
}

pub fn render(config: &SshConfig) -> String {
    format!(
        "{MARKER}\nPort {}\nPasswordAuthentication {}\nPermitRootLogin {}\n",
        config.port.get(),
        if config.password_authentication {
            "yes"
        } else {
            "no"
        },
        if config.root_login { "yes" } else { "no" },
    )
}

/// Require the primary file to include this exact managed drop-in, or a glob
/// that matches it, before any controlled directive. This deliberately accepts
/// only a directly visible early Include; complex nested include trees fail
/// closed instead of pretending our values win OpenSSH's first-value rules.
pub fn validate_include_order(primary: &str, dropin: &Path) -> Result<(), String> {
    let mut included = false;
    for raw in primary.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let key = parts.next().unwrap_or("");
        if key.eq_ignore_ascii_case("match") {
            return Err("sshd_config contains Match scope; this adapter cannot prove effective SSH policy for all connection contexts".to_owned());
        }
        if key.eq_ignore_ascii_case("include") {
            let mut has_managed_include = false;
            for pattern in parts {
                let resolved = if Path::new(pattern).is_absolute() {
                    PathBuf::from(pattern)
                } else {
                    Path::new("/etc/ssh").join(pattern)
                };
                if simple_glob_matches(&resolved.to_string_lossy(), &dropin.to_string_lossy()) {
                    has_managed_include = true;
                } else {
                    return Err(format!(
                        "sshd_config contains an unmanaged Include ({pattern}); cannot prove SSH policy ordering or Match scope"
                    ));
                }
            }
            included |= has_managed_include;
            continue;
        }
        if ["port", "passwordauthentication", "permitrootlogin"]
            .iter()
            .any(|k| key.eq_ignore_ascii_case(k))
            && !included
        {
            return Err("sshd_config sets an SSH policy before including the Mistborn drop-in; move/add an early Include for /etc/ssh/sshd_config.d/*.conf".to_owned());
        }
    }
    if included {
        Ok(())
    } else {
        Err("sshd_config does not include the Mistborn SSH drop-in early enough; add Include /etc/ssh/sshd_config.d/*.conf near the top".to_owned())
    }
}

fn simple_glob_matches(pattern: &str, value: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == value;
    };
    let filename = Path::new(value)
        .file_name()
        .and_then(|v| v.to_str())
        .unwrap_or("");
    Path::new(pattern).parent() == Path::new(value).parent()
        && filename.starts_with(prefix.rsplit('/').next().unwrap_or(""))
        && filename.ends_with(suffix)
}

fn validate_dropin_directory(directory: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(directory).map_err(|error| {
        format!(
            "cannot inspect SSH drop-in directory {}: {error}",
            directory.display()
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "refusing symlink SSH drop-in directory {}",
            directory.display()
        ));
    }
    if !metadata.is_dir() {
        return Err(format!(
            "SSH drop-in path is not a directory: {}",
            directory.display()
        ));
    }
    let entries = fs::read_dir(directory).map_err(|error| {
        format!(
            "cannot inspect SSH drop-in directory {}: {error}",
            directory.display()
        )
    })?;
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot enumerate SSH drop-in directory: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("conf") {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("cannot inspect SSH include {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!("SSH include is a symlink: {}", path.display()));
        }
        if !metadata.is_file() {
            return Err(format!(
                "SSH include is not a regular file: {}",
                path.display()
            ));
        }
        let content = fs::read_to_string(&path)
            .map_err(|error| format!("cannot read SSH include {}: {error}", path.display()))?;
        for raw in content.lines() {
            let directive = raw
                .split('#')
                .next()
                .unwrap_or("")
                .split_whitespace()
                .next()
                .unwrap_or("");
            if directive.eq_ignore_ascii_case("match") {
                return Err(format!(
                    "SSH include {} contains Match scope; effective policy cannot be proven for all contexts",
                    path.display()
                ));
            }
            if directive.eq_ignore_ascii_case("include") {
                return Err(format!(
                    "SSH include {} contains a nested Include; effective policy cannot be safely ordered",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn parse_effective(text: &str, desired: &SshConfig) -> Result<(), String> {
    let mut values = std::collections::BTreeMap::<String, Vec<String>>::new();
    for line in text.lines() {
        let Some((key, value)) = line.split_once(char::is_whitespace) else {
            continue;
        };
        if ["port", "passwordauthentication", "permitrootlogin"].contains(&key) {
            values
                .entry(key.to_owned())
                .or_default()
                .push(value.trim().to_ascii_lowercase());
        }
    }
    let port = values.get("port").ok_or("sshd -T omitted port")?;
    if port != &[desired.port.get().to_string()] {
        return Err(format!(
            "effective SSH ports {port:?} do not match desired port {}",
            desired.port.get()
        ));
    }
    let expected = [
        (
            "passwordauthentication",
            if desired.password_authentication {
                "yes"
            } else {
                "no"
            },
        ),
        (
            "permitrootlogin",
            if desired.root_login { "yes" } else { "no" },
        ),
    ];
    for (key, wanted) in expected {
        let actual = values
            .get(key)
            .ok_or_else(|| format!("sshd -T omitted {key}"))?;
        if actual.len() != 1 || actual[0] != wanted {
            return Err(format!(
                "effective SSH {key} does not match desired value {wanted}"
            ));
        }
    }
    Ok(())
}

fn parse_ports(text: &str) -> Result<Vec<u16>, String> {
    let ports = text
        .lines()
        .filter_map(|line| {
            let (key, value) = line.split_once(char::is_whitespace)?;
            (key == "port")
                .then(|| value.trim().parse::<u16>().ok())
                .flatten()
        })
        .collect::<Vec<_>>();
    if ports.is_empty() || ports.contains(&0) {
        Err("sshd -T did not provide a valid current SSH port".to_owned())
    } else {
        Ok(ports)
    }
}

pub fn verify_effective(text: &str, desired: &SshConfig) -> Result<(), String> {
    parse_effective(text, desired)
}

fn ufw_allows_port(status: &str, port: u16) -> bool {
    status.lines().any(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        fields.first() == Some(&format!("{port}/tcp").as_str())
            && fields
                .get(1)
                .is_some_and(|field| field.eq_ignore_ascii_case("ALLOW"))
            && fields
                .get(2)
                .is_some_and(|field| field.eq_ignore_ascii_case("IN"))
            && fields.get(3).is_some_and(|field| {
                field.eq_ignore_ascii_case("Anywhere") || field.eq_ignore_ascii_case("Anywhere(v6)")
            })
    })
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("SSH drop-in has no parent directory")?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp = parent.join(format!(".mistborn-ssh-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|error| format!("cannot create SSH temporary file: {error}"))?;
        file.write_all(bytes)
            .map_err(|error| format!("cannot write SSH temporary file: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("cannot sync SSH temporary file: {error}"))?;
        fs::rename(&temp, path).map_err(|error| format!("cannot publish SSH drop-in: {error}"))?;
        if let Ok(dir) = OpenOptions::new().read(true).open(parent) {
            let _ = dir.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn ensure_no_symlink_ancestors(root: &Path, path: &Path) -> Result<(), String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "SSH config path escapes adapter root")?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "refusing SSH config path containing symlink {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!(
                    "SSH config path component is missing: {}",
                    current.display()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "cannot inspect SSH config path {}: {error}",
                    current.display()
                ));
            }
        }
    }
    Ok(())
}

fn rollback(path: &Path, old: Option<&str>, reason: String) -> String {
    let rollback_result = match old {
        Some(old) => atomic_write(path, old.as_bytes()),
        None => {
            fs::remove_file(path).map_err(|error| format!("cannot remove new SSH drop-in: {error}"))
        }
    };
    match rollback_result {
        Ok(()) => format!("{reason}; previous SSH drop-in restored"),
        Err(error) => format!("{reason}; CRITICAL: SSH drop-in rollback failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_dropin_is_rendered_from_desired_state() {
        let config = mistborn_bootstrap::config::DesiredState::from_toml("version=1\nprofile='vps'\n[ssh]\nport=2222\npassword_authentication=false\nroot_login=false\n").unwrap().ssh.unwrap();
        assert_eq!(
            render(&config),
            format!("{MARKER}\nPort 2222\nPasswordAuthentication no\nPermitRootLogin no\n")
        );
    }

    #[test]
    fn include_must_precede_policy_and_match_dropin() {
        let path = Path::new("/etc/ssh/sshd_config.d/00-mistborn.conf");
        assert!(
            validate_include_order("Include /etc/ssh/sshd_config.d/*.conf\nPort 22\n", path)
                .is_ok()
        );
        assert!(
            validate_include_order("Port 22\nInclude /etc/ssh/sshd_config.d/*.conf\n", path)
                .is_err()
        );
        assert!(validate_include_order("Include /other/*.conf\n", path).is_err());
        assert!(
            validate_include_order(
                "Include /etc/ssh/other.conf\nInclude /etc/ssh/sshd_config.d/*.conf\n",
                path
            )
            .is_err()
        );
    }

    #[test]
    fn nested_match_and_include_fragments_fail_closed() {
        let root = fixture();
        let directory = root.join("etc/ssh/sshd_config.d");
        fs::write(
            directory.join("50-admin.conf"),
            "Match User admin\n  PermitRootLogin yes\n",
        )
        .unwrap();
        assert!(
            validate_dropin_directory(&directory)
                .unwrap_err()
                .contains("Match scope")
        );
        fs::write(
            directory.join("50-admin.conf"),
            "Include /etc/ssh/other.conf\n",
        )
        .unwrap();
        assert!(
            validate_dropin_directory(&directory)
                .unwrap_err()
                .contains("nested Include")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_dropin_directory_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = fixture();
        let directory = root.join("etc/ssh/sshd_config.d");
        let target = root.join("outside");
        fs::create_dir_all(&target).unwrap();
        fs::remove_dir(&directory).unwrap();
        symlink(&target, &directory).unwrap();
        assert!(
            validate_dropin_directory(&directory)
                .unwrap_err()
                .contains("symlink SSH drop-in directory")
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn effective_ssh_fields_are_checked_exactly() {
        let config = mistborn_bootstrap::config::DesiredState::from_toml("version=1\nprofile='vps'\n[ssh]\nport=2222\npassword_authentication=false\nroot_login=false\n").unwrap().ssh.unwrap();
        assert!(
            parse_effective(
                "port 2222\npasswordauthentication no\npermitrootlogin no\n",
                &config
            )
            .is_ok()
        );
        assert!(
            parse_effective(
                "port 22\npasswordauthentication yes\npermitrootlogin no\n",
                &config
            )
            .is_err()
        );
    }

    #[test]
    fn ufw_precondition_requires_allow_for_changed_ssh_port() {
        assert!(ufw_allows_port(
            "Status: active\n22/tcp ALLOW IN Anywhere\n",
            22
        ));
        assert!(!ufw_allows_port(
            "Status: active\n22/tcp ALLOW IN Anywhere\n",
            2222
        ));
        assert!(!ufw_allows_port(
            "Status: active\n2222/tcp ALLOW IN Anywhere\n",
            22
        ));
    }

    #[derive(Default)]
    struct FakeRunner {
        calls: Vec<String>,
        results: std::collections::VecDeque<Result<CommandResult, String>>,
    }
    impl Runner for FakeRunner {
        fn run(&mut self, program: &str, args: &[&str]) -> Result<CommandResult, String> {
            self.calls.push(format!("{program} {}", args.join(" ")));
            self.results.pop_front().unwrap_or_else(|| {
                Ok(CommandResult {
                    success: true,
                    stdout: String::new(),
                    stderr: String::new(),
                })
            })
        }
    }

    fn fixture() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "mistborn-ssh-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(dir.join("etc/ssh/sshd_config.d")).unwrap();
        let include = dir.join("etc/ssh/sshd_config.d/*.conf");
        fs::write(
            dir.join("etc/ssh/sshd_config"),
            format!("Include {}\n", include.display()),
        )
        .unwrap();
        dir
    }

    fn desired() -> SshConfig {
        mistborn_bootstrap::config::DesiredState::from_toml("version=1\nprofile='vps'\n[ssh]\nport=22\npassword_authentication=false\nroot_login=false\n").unwrap().ssh.unwrap()
    }

    fn success(stdout: &str) -> Result<CommandResult, String> {
        Ok(CommandResult {
            success: true,
            stdout: stdout.to_owned(),
            stderr: String::new(),
        })
    }

    #[test]
    fn syntax_failure_rolls_back_and_never_reloads() {
        let root = fixture();
        let mut runner = FakeRunner::default();
        runner.results.extend([
            success("port 22\npasswordauthentication yes\npermitrootlogin yes\n"),
            success("loaded\n"),
            success(""),
            Ok(CommandResult {
                success: false,
                stdout: String::new(),
                stderr: "bad config".into(),
            }),
        ]);
        let mut adapter = SshAdapter {
            root: root.clone(),
            runner,
        };
        let error = adapter.apply(&desired()).unwrap_err();
        assert!(error.contains("restored"), "{error}");
        assert!(!root.join("etc/ssh/sshd_config.d/00-mistborn.conf").exists());
        assert!(
            !adapter
                .runner
                .calls
                .iter()
                .any(|call| call.starts_with("systemctl reload"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn effective_verification_failure_rolls_back_before_reload() {
        let root = fixture();
        let mut runner = FakeRunner::default();
        runner.results.extend([
            success("port 22\npasswordauthentication yes\npermitrootlogin yes\n"),
            success("loaded\n"),
            success(""),
            success(""),
            success("port 22\npasswordauthentication yes\npermitrootlogin yes\n"),
        ]);
        let mut adapter = SshAdapter {
            root: root.clone(),
            runner,
        };
        assert!(
            adapter
                .apply(&desired())
                .unwrap_err()
                .contains("effective SSH passwordauthentication")
        );
        assert!(!root.join("etc/ssh/sshd_config.d/00-mistborn.conf").exists());
        assert!(
            !adapter
                .runner
                .calls
                .iter()
                .any(|call| call.starts_with("systemctl reload"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reload_failure_restores_dropin_and_attempts_prior_policy_reload() {
        let root = fixture();
        let mut runner = FakeRunner::default();
        runner.results.extend([
            success("port 22\npasswordauthentication yes\npermitrootlogin yes\n"),
            success("loaded\n"),
            success(""),
            success(""),
            success("port 22\npasswordauthentication no\npermitrootlogin no\n"),
            Ok(CommandResult {
                success: false,
                stdout: String::new(),
                stderr: "reload failed".into(),
            }),
            success(""),
        ]);
        let mut adapter = SshAdapter {
            root: root.clone(),
            runner,
        };
        let error = adapter.apply(&desired()).unwrap_err();
        assert!(error.contains("previous SSH drop-in restored"), "{error}");
        assert!(error.contains("restored SSH policy reloaded"), "{error}");
        assert!(!root.join("etc/ssh/sshd_config.d/00-mistborn.conf").exists());
        assert_eq!(
            adapter
                .runner
                .calls
                .iter()
                .filter(|call| call.starts_with("systemctl reload"))
                .count(),
            2
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn port_change_refuses_before_writing_when_active_ufw_lacks_public_rule() {
        let root = fixture();
        let desired = mistborn_bootstrap::config::DesiredState::from_toml("version=1\nprofile='vps'\n[ssh]\nport=2222\npassword_authentication=false\nroot_login=false\n").unwrap().ssh.unwrap();
        let mut runner = FakeRunner::default();
        runner.results.extend([
            success("port 22\n"),
            success("Status: active\n22/tcp ALLOW IN Anywhere\n"),
        ]);
        let mut adapter = SshAdapter {
            root: root.clone(),
            runner,
        };
        assert!(
            adapter
                .apply(&desired)
                .unwrap_err()
                .contains("reconcile security/ufw first")
        );
        assert!(!root.join("etc/ssh/sshd_config.d/00-mistborn.conf").exists());
        assert!(
            !adapter
                .runner
                .calls
                .iter()
                .any(|call| call.starts_with("systemctl"))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn symlinked_and_unowned_dropins_are_never_replaced() {
        let root = fixture();
        let path = root.join("etc/ssh/sshd_config.d/00-mistborn.conf");
        fs::write(&path, "Port 2222\n").unwrap();
        let mut runner = FakeRunner::default();
        runner
            .results
            .extend([success("port 22\n"), success("loaded"), success("")]);
        let mut adapter = SshAdapter {
            root: root.clone(),
            runner,
        };
        let error = adapter.apply(&desired()).unwrap_err();
        assert!(error.contains("unowned"), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "Port 2222\n");
        fs::remove_file(&path).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("sshd_config", &path).unwrap();
        adapter.runner = FakeRunner::default();
        adapter
            .runner
            .results
            .extend([success("port 22\n"), success("loaded"), success("")]);
        assert!(adapter.apply(&desired()).unwrap_err().contains("symlink"));
        let _ = fs::remove_dir_all(root);
    }
}
