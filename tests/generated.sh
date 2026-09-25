#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

tailscale_output="$(
  MISTBORN_TAILSCALE_EXIT_NODE=1 MISTBORN_TAILSCALE_AUTO_UPDATE=1 \
    bash "$root/dist/server.sh" --dry-run --yes --user root --only tailscale
)"
[[ "$tailscale_output" == *"Would enable persistent IPv4 and IPv6 forwarding"* ]]
[[ "$tailscale_output" == *"tailscale up --advertise-exit-node"* ]]
[[ "$tailscale_output" == *"tailscale set --auto-update"* ]]

rclone_output="$(
  MISTBORN_RCLONE_CONFIGURE=1 \
    bash "$root/dist/server.sh" --dry-run --yes --user root --only rclone
)"
[[ "$rclone_output" == *"env HOME="*" rclone config"* ]]
[[ "$rclone_output" != *"sudo -H -u root rclone config"* ]]

plex_output="$(
  MISTBORN_HARDEN=1 MISTBORN_DISABLE_PASSWORD_AUTH=0 MISTBORN_PLEX_UFW=1 \
    MISTBORN_PLEX_LAN_CIDR=192.168.1.0/24 MISTBORN_PLEX_TAILSCALE=1 \
    bash "$root/dist/server.sh" --dry-run --yes --user root --only security
)"
[[ "$plex_output" == *"Would install the Plex UFW application profiles"* ]]
[[ "$plex_output" == *"ufw allow 32400/tcp comment Plex\\ remote\\ access"* ]]
[[ "$plex_output" == *"ufw allow from 192.168.1.0/24 to any app plexmediaserver-all"* ]]
[[ "$plex_output" == *"ufw allow in on tailscale0 to any app plexmediaserver-all"* ]]

plex_only_output="$(
  MISTBORN_HARDEN=1 MISTBORN_DISABLE_PASSWORD_AUTH=0 MISTBORN_PLEX_UFW=1 \
    bash "$root/dist/server.sh" --dry-run --yes --user root --only security --tasks plex-firewall
)"
[[ "$plex_only_output" == *"Would install the Plex UFW application profiles"* ]]
[[ "$plex_only_output" != *"Would harden /etc/ssh/sshd_config"* ]]
[[ "$plex_only_output" != *"systemctl enable --now fail2ban"* ]]

decoded_config="$(
  sed -n "s/^export MISTBORN_CONFIG_B64='\([^']*\)'$/\1/p" "$root/dist/server.sh" | base64 -d
)"
[[ "$decoded_config" == *'version = 1'* ]]
[[ "$decoded_config" == *'Optional sections are omitted'* ]]
grep -Fq 'mistborn_publish_desired_config /etc/mistborn /usr/local/share/mistborn' "$root/dist/server.sh"
grep -Fq "export MISTBORN_CONFIG_EXAMPLE_B64='" "$root/dist/server.sh"

if MISTBORN_HARDEN=1 MISTBORN_DISABLE_PASSWORD_AUTH=0 MISTBORN_PLEX_UFW=1 \
  MISTBORN_PLEX_LAN_CIDR=not-a-cidr \
  bash "$root/dist/server.sh" --dry-run --yes --user root --only security; then
  printf 'invalid Plex LAN CIDR unexpectedly succeeded\n' >&2
  exit 1
fi
