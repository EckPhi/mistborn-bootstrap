use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;

pub const CONFIG_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesiredState {
    pub version: u8,
    pub profile: Profile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh: Option<SshConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub firewall: Option<FirewallConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tailscale: Option<TailscaleConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fail2ban: Option<Fail2banConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Profile {
    Vps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SshConfig {
    pub port: Port,
    pub password_authentication: bool,
    pub root_login: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirewallConfig {
    pub enabled: bool,
    pub default_incoming: IncomingPolicy,
    #[serde(default)]
    pub public_tcp_ports: Vec<Port>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plex: Option<PlexFirewallConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IncomingPolicy {
    Allow,
    Deny,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlexFirewallConfig {
    pub enabled: bool,
    pub public_remote_access: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lan_cidr: Option<Ipv4Cidr>,
    pub tailscale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TailscaleConfig {
    pub ssh: bool,
    pub advertise_exit_node: bool,
    pub auto_update: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fail2banConfig {
    pub enabled: bool,
    pub sshd: Fail2banSshdConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fail2banSshdConfig {
    pub enabled: bool,
    pub maxretry: u32,
    /// Ban duration in seconds.
    pub bantime: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct Port(u16);

impl Port {
    pub fn get(self) -> u16 {
        self.0
    }
}

impl<'de> Deserialize<'de> for Port {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        if value == 0 {
            return Err(serde::de::Error::custom("port must be between 1 and 65535"));
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Ipv4Cidr(String);

impl Ipv4Cidr {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parse(value: String) -> Result<Self, String> {
        let (address, prefix) = value
            .split_once('/')
            .ok_or_else(|| "IPv4 CIDR must include a prefix length".to_owned())?;
        address
            .parse::<Ipv4Addr>()
            .map_err(|_| "IPv4 CIDR contains an invalid address".to_owned())?;
        let prefix = prefix
            .parse::<u8>()
            .map_err(|_| "IPv4 CIDR contains an invalid prefix length".to_owned())?;
        if prefix > 32 {
            return Err("IPv4 CIDR prefix length must be between 0 and 32".to_owned());
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for Ipv4Cidr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    Read(std::io::Error),
    Parse(toml::de::Error),
    Invalid(String),
    UnsupportedVersion(u8),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Read(error) => write!(formatter, "cannot read configuration: {error}"),
            Self::Parse(error) => write!(formatter, "invalid configuration: {error}"),
            Self::Invalid(message) => write!(formatter, "invalid configuration: {message}"),
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "unsupported configuration version {version}; expected {CONFIG_VERSION}"
            ),
        }
    }
}

impl std::error::Error for ConfigError {}

impl DesiredState {
    pub fn from_toml(contents: &str) -> Result<Self, ConfigError> {
        let config: Self = toml::from_str(contents).map_err(ConfigError::Parse)?;
        if config.version != CONFIG_VERSION {
            return Err(ConfigError::UnsupportedVersion(config.version));
        }
        if config
            .fail2ban
            .as_ref()
            .is_some_and(|policy| policy.sshd.maxretry == 0 || policy.sshd.bantime == 0)
        {
            return Err(ConfigError::Invalid(
                "fail2ban sshd maxretry and bantime must be greater than zero".to_owned(),
            ));
        }
        Ok(config)
    }

    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let contents = fs::read_to_string(path).map_err(ConfigError::Read)?;
        Self::from_toml(&contents)
    }

    pub fn to_toml(&self) -> Result<String, toml::ser::Error> {
        toml::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const COMPLETE: &str = include_str!("../assets/config.toml.example");

    #[test]
    fn complete_configuration_round_trips() {
        let desired = DesiredState::from_toml(COMPLETE).unwrap();
        let encoded = desired.to_toml().unwrap();
        assert_eq!(DesiredState::from_toml(&encoded).unwrap(), desired);
        assert_eq!(desired.ssh.unwrap().port.get(), 22);
    }

    #[test]
    fn omitted_sections_are_unmanaged() {
        let desired = DesiredState::from_toml("version = 1\nprofile = \"vps\"\n").unwrap();
        assert!(
            desired.ssh.is_none()
                && desired.firewall.is_none()
                && desired.tailscale.is_none()
                && desired.fail2ban.is_none()
        );
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = DesiredState::from_toml("version = 1\nprofile = \"vps\"\n[ssh]\nport = 22\npassword_authentication = false\nroot_login = false\npassword_authentcation = true\n").unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_invalid_ports() {
        for port in ["0", "65536", "-1"] {
            let input = format!(
                "version = 1\nprofile = \"vps\"\n[ssh]\nport = {port}\npassword_authentication = false\nroot_login = false\n"
            );
            assert!(
                DesiredState::from_toml(&input).is_err(),
                "accepted port {port}"
            );
        }
    }

    #[test]
    fn rejects_zero_fail2ban_thresholds() {
        for (maxretry, bantime) in [(0, 3600), (3, 0)] {
            let input = format!(
                "version = 1\nprofile = \"vps\"\n[fail2ban]\nenabled = true\n[fail2ban.sshd]\nenabled = true\nmaxretry = {maxretry}\nbantime = {bantime}\n"
            );
            assert!(DesiredState::from_toml(&input).is_err());
        }
    }

    #[test]
    fn rejects_invalid_cidrs() {
        for cidr in [
            "192.168.1.1",
            "192.168.1.256/24",
            "192.168.1.0/33",
            "::1/128",
        ] {
            let input = COMPLETE.replace("192.168.1.0/24", cidr);
            assert!(
                DesiredState::from_toml(&input).is_err(),
                "accepted CIDR {cidr}"
            );
        }
    }

    #[test]
    fn rejects_unsupported_versions() {
        let error = DesiredState::from_toml("version = 2\nprofile = \"vps\"\n").unwrap_err();
        assert!(matches!(error, ConfigError::UnsupportedVersion(2)));
    }
}
