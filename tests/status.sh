#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture/bin" "$fixture/state" "$fixture/runtipi"

for command in docker tailscale rclone ufw fail2ban-client systemctl sshd; do
  ln -s mock "$fixture/bin/$command"
done

rm -f "$fixture/plex-profile"
if drift_output="$(
  PATH="$fixture/bin:/usr/bin:/bin" \
  RUNTIPI_PATH="$fixture/runtipi" \
  MISTBORN_STATE_DIR="$fixture/state" \
  MISTBORN_RUNNER_PATH="$fixture/runner" \
  MISTBORN_PLEX_UFW_PROFILE="$fixture/plex-profile" \
  MISTBORN_BOOTSTRAP_VERSION=v0.8.0 \
    bash "$root/assets/mistborn" status
)"; then
  printf 'status unexpectedly accepted a missing applied Plex profile\n' >&2
  exit 1
fi
[[ "$drift_output" == *'FAIL  applied Plex UFW profile is missing'* ]]
touch "$fixture/plex-profile"
printf '#!/usr/bin/env bash\nexit 0\n' >"$fixture/runner"
chmod +x "$fixture/runner"
cat >"$fixture/state/server.json" <<'JSON'
{"version":2,"collection":"server","updated_at":1700000000,"modules":{"common":{"status":"completed"},"tailscale":{"status":"completed","tasks":{"forwarding":{"status":"completed"}}},"security":{"status":"partial","tasks":{"ssh":{"status":"completed"},"firewall":{"status":"completed"},"plex-firewall":{"status":"completed"},"fail2ban":{"status":"completed"},"tailscale-only":{"status":"completed"}}}}}
JSON
cat >"$fixture/bin/mock" <<'MOCK'
#!/usr/bin/env bash
set -Eeuo pipefail
command="$(basename "$0")"
case "$command:$*" in
  docker:--version) echo 'Docker version 28.0.0' ;;
  docker:info) exit 0 ;;
  tailscale:--version) echo '1.88.0' ;;
  tailscale:'status --json') echo '{"BackendState":"Running"}' ;;
  tailscale:'debug prefs') echo '{"RunSSH":true,"AdvertiseRoutes":["0.0.0.0/0"]}' ;;
  rclone:--version) echo 'rclone v1.70.0' ;;
  ufw:--version) echo 'ufw 0.36.2' ;;
  ufw:'status verbose') cat <<'EOF'
Status: active
Default: deny (incoming), allow (outgoing), disabled (routed)
32400/tcp                 ALLOW IN    Anywhere
plexmediaserver-all on tailscale0 ALLOW IN Anywhere
EOF
    ;;
  fail2ban-client:--version) echo 'Fail2Ban v1.1.0' ;;
  fail2ban-client:'status sshd') exit 0 ;;
  systemctl:'is-active --quiet docker'|systemctl:'is-active --quiet tailscaled'|systemctl:'is-active --quiet fail2ban') exit 0 ;;
  sshd:'-T') printf '%s\n' 'port 22' 'passwordauthentication no' 'permitrootlogin no' ;;
  *) exit 1 ;;
esac
MOCK
chmod +x "$fixture/bin/mock"

output="$(
  PATH="$fixture/bin:/usr/bin:/bin" \
  RUNTIPI_PATH="$fixture/runtipi" \
  MISTBORN_STATE_DIR="$fixture/state" \
  MISTBORN_RUNNER_PATH="$fixture/runner" \
  MISTBORN_PLEX_UFW_PROFILE="$fixture/plex-profile" \
  MISTBORN_BOOTSTRAP_VERSION=v0.8.0 \
    bash "$root/assets/mistborn" status
)"

for expected in \
  'bootstrap version: v0.8.0' \
  'Docker version 28.0.0' \
  'schema v2' \
  'security: partial' \
  'SSH password authentication disabled' \
  'UFW default incoming policy is deny' \
  'Plex remote-access rule present' \
  'Plex local services allowed through tailscale0' \
  'Tailscale backend running' \
  'Tailscale SSH enabled' \
  'Tailscale routes are advertised'; do
  [[ "$output" == *"$expected"* ]] || {
    printf 'missing status output: %s\n%s\n' "$expected" "$output" >&2
    exit 1
  }
done
