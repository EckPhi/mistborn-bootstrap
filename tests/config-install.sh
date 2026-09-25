#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT

# shellcheck disable=SC1091
source "$root/modules/toolset.sh"
MISTBORN_CONFIG_B64="$(base64 <"$root/assets/config.toml" | tr -d '\n')"
MISTBORN_CONFIG_EXAMPLE_B64="$(base64 <"$root/assets/config.toml.example" | tr -d '\n')"
MISTBORN_CONFIG_OWNER="$(id -u):$(id -g)"
export MISTBORN_CONFIG_B64 MISTBORN_CONFIG_EXAMPLE_B64 MISTBORN_CONFIG_OWNER
export MISTBORN_SKIP_FSYNC=1
validator="${MISTBORN_TEST_BINARY:-$root/target/debug/mistborn-bootstrap}"
if [[ -z "${MISTBORN_TEST_BINARY:-}" ]]; then
  cargo build --manifest-path "$root/Cargo.toml"
fi
[[ -x "$validator" ]]

fresh="$fixture/fresh"
export MISTBORN_CONFIG_ADOPTION_ALLOWED=1 MISTBORN_CONFIG_APPLIED_TASKS='security/ssh,security/firewall,security/plex-firewall,security/fail2ban,tailscale/connect'
export MISTBORN_HARDEN=1 MISTBORN_SSH_PORT=2222 MISTBORN_ALLOWED_TCP_PORTS='80 443'
export MISTBORN_PLEX_UFW=1 MISTBORN_PLEX_LAN_CIDR='192.168.50.0/24' MISTBORN_PLEX_TAILSCALE=1
export MISTBORN_TAILSCALE_SSH=1 MISTBORN_TAILSCALE_EXIT_NODE=0 MISTBORN_TAILSCALE_AUTO_UPDATE=1
mistborn_publish_desired_config "$fresh/etc/mistborn" "$fresh/share/mistborn" "$validator"
fresh_config="$fresh/etc/mistborn/config.toml"
grep -Fq 'port = 2222' "$fresh_config"
grep -Fq 'public_tcp_ports = [80, 443]' "$fresh_config"
grep -Fq 'lan_cidr = "192.168.50.0/24"' "$fresh_config"
grep -Fq 'ssh = true' "$fresh_config"
grep -Fq '[fail2ban.sshd]' "$fresh_config"
grep -Fq 'maxretry = 3' "$fresh_config"
[[ -f "$fresh/share/mistborn/config.toml.example" ]]
mode="$(stat -f '%Lp' "$fresh_config" 2>/dev/null || stat -c '%a' "$fresh_config")"
owner="$(stat -f '%u:%g' "$fresh_config" 2>/dev/null || stat -c '%u:%g' "$fresh_config")"
[[ "$mode" == 644 ]]
[[ "$owner" == "$MISTBORN_CONFIG_OWNER" ]]

unmanaged="$fixture/unmanaged"
unset MISTBORN_HARDEN MISTBORN_SSH_PORT MISTBORN_ALLOWED_TCP_PORTS MISTBORN_PLEX_UFW
unset MISTBORN_PLEX_LAN_CIDR MISTBORN_PLEX_TAILSCALE MISTBORN_TAILSCALE_SSH
unset MISTBORN_TAILSCALE_EXIT_NODE MISTBORN_TAILSCALE_AUTO_UPDATE
export MISTBORN_CONFIG_APPLIED_TASKS=''
mistborn_publish_desired_config "$unmanaged/etc/mistborn" "$unmanaged/share/mistborn" "$validator"
cmp "$root/assets/config.toml" "$unmanaged/etc/mistborn/config.toml"

upgrade="$fixture/upgrade-without-config"
export MISTBORN_CONFIG_ADOPTION_ALLOWED=0 MISTBORN_HARDEN=1 MISTBORN_CONFIG_APPLIED_TASKS='security/ssh,security/firewall'
mistborn_publish_desired_config "$upgrade/etc/mistborn" "$upgrade/share/mistborn" "$validator"
cmp "$root/assets/config.toml" "$upgrade/etc/mistborn/config.toml"

partial="$fixture/partial"
export MISTBORN_CONFIG_ADOPTION_ALLOWED=1 MISTBORN_HARDEN=1 MISTBORN_CONFIG_APPLIED_TASKS='security/ssh'
mistborn_publish_desired_config "$partial/etc/mistborn" "$partial/share/mistborn" "$validator"
grep -Fq '[ssh]' "$partial/etc/mistborn/config.toml"
if grep -Fq '[firewall]' "$partial/etc/mistborn/config.toml"; then
  printf 'unapplied firewall task was adopted\n' >&2
  exit 1
fi
if grep -Fq '[fail2ban]' "$partial/etc/mistborn/config.toml"; then
  printf 'fail2ban policy was adopted without its completed task\n' >&2
  exit 1
fi

printf 'version = 1\nprofile = "vps"\n# operator-owned\n' >"$unmanaged/etc/mistborn/config.toml"
export MISTBORN_CONFIG_ADOPTION_ALLOWED=1 MISTBORN_HARDEN=1 MISTBORN_CONFIG_APPLIED_TASKS='security/ssh'
mistborn_publish_desired_config "$unmanaged/etc/mistborn" "$unmanaged/share/mistborn" "$validator"
grep -Fq '# operator-owned' "$unmanaged/etc/mistborn/config.toml"
if grep -Fq '[ssh]' "$unmanaged/etc/mistborn/config.toml"; then
  printf 'existing configuration was overwritten\n' >&2
  exit 1
fi

invalid="$fixture/invalid"
valid_payload="$MISTBORN_CONFIG_B64"
MISTBORN_CONFIG_B64="$(printf 'version = 99\nprofile = "vps"\n' | base64 | tr -d '\n')"
export MISTBORN_CONFIG_B64
if mistborn_publish_desired_config "$invalid/etc/mistborn" "$invalid/share/mistborn" "$validator"; then
  printf 'invalid desired-state payload unexpectedly published\n' >&2
  exit 1
fi
[[ ! -e "$invalid/etc/mistborn/config.toml" ]]
if compgen -G "$invalid/etc/mistborn/.install.*" >/dev/null; then
  printf 'failed publication left a staging directory\n' >&2
  exit 1
fi
export MISTBORN_CONFIG_B64="$valid_payload"
