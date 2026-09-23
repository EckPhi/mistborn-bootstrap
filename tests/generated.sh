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
