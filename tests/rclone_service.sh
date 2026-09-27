#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
MISTBORN_RCLONE_ROOT="$(cd "$fixture" && pwd -P)"
export MISTBORN_RCLONE_ROOT
# shellcheck disable=SC1091
source "$root/modules/rclone_service.sh"
# shellcheck disable=SC1091
source "$root/lib/ui.sh"

grep -Fq "EnvironmentFile=\$env_file" "$root/modules/rclone_service.sh"
grep -Fq 'RCLONE_RC_USER=%s' "$root/modules/rclone_service.sh"
grep -Fq 'RCLONE_RC_PASS=%s' "$root/modules/rclone_service.sh"
grep -Fq 'RuntimeDirectory=rclone' "$root/modules/rclone_service.sh"
grep -Fq 'RuntimeDirectoryMode=0750' "$root/modules/rclone_service.sh"
grep -Fq 'RuntimeDirectoryPreserve=restart' "$root/modules/rclone_service.sh"
grep -Fq 'UMask=0007' "$root/modules/rclone_service.sh"
grep -Fq 'Before=docker.service' "$root/modules/rclone_service.sh"
grep -Fq 'rcd --rc-addr=/run/rclone/rc.sock' "$root/modules/rclone_service.sh"
grep -Fq 'NoNewPrivileges=false' "$root/modules/rclone_service.sh"
if grep -Eq '^(ProtectSystem|ProtectHome|PrivateTmp|ProtectKernelTunables|ProtectControlGroups|ReadWritePaths|PrivateMounts)=' "$root/modules/rclone_service.sh"; then
  printf 'rclone RC service must not use mount namespace isolation\n' >&2
  exit 1
fi
if grep -Eq '5573|5574|--api-addr|gui --addr|ufw allow' "$root/modules/rclone_service.sh"; then
  printf 'TCP GUI/RC listener or firewall mutation remains in the service module\n' >&2
  exit 1
fi

# shellcheck disable=SC2317,SC2329 # Preflight verifies this mocked systemctl indirectly.
systemctl() { return 0; }
runuser() { return 0; }
docker() { printf 'unexpected Docker probe\n' >&2; return 1; }
ufw() { printf 'unexpected UFW probe\n' >&2; return 1; }
rclone() {
  case "$*" in
    'rcd --help')
      if [[ "${scenario:-}" == old-rclone ]]; then printf 'Usage: rclone rcd --rc-addr stringArray\n'; else
        if [[ "${scenario:-}" == no-rc-pass ]]; then printf 'Usage: rclone rcd --rc-addr stringArray --rc-user string\n'; return 0; fi
        printf 'Usage: rclone rcd --rc-addr stringArray --rc-user string --rc-pass string\n'
      fi
      ;;
    '--help') printf 'Use "rclone help flags" for to see the global flags.\n' ;;
    'help flags') printf 'Global flags: --config string\n' ;;
    *) return 0 ;;
  esac
}
ps() { printf 'dockerd --host-gateway-ip 192.0.2.1\n'; }
getent() { return 2; }
id() { return 1; }

mistborn_rclone_service_preflight
scenario=old-rclone
if mistborn_rclone_service_preflight 2>/dev/null; then
  printf 'expected preflight rejection for missing RC auth flags\n' >&2
  exit 1
fi
scenario=no-rc-pass
if mistborn_rclone_service_preflight 2>/dev/null; then
  printf 'expected preflight rejection for missing RC password flag\n' >&2
  exit 1
fi
scenario=ok
mistborn_rclone_credentials_valid Safe.User 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'
if mistborn_rclone_credentials_valid 'bad user' 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'; then exit 1; fi
if mistborn_rclone_credentials_valid Safe.User short; then exit 1; fi
curl() { [[ "$*" == *"--unix-socket /run/rclone/rc.sock"* ]] && printf '401'; }
mistborn_rclone_verify_socket_auth /run/rclone/rc.sock
curl() { printf '200'; }
if mistborn_rclone_verify_socket_auth /run/rclone/rc.sock 2>/dev/null; then
  printf 'expected unauthenticated RC access to fail verification\n' >&2
  exit 1
fi

# Existing account state rollback remains intact for a failed service restart.
tx="$MISTBORN_RCLONE_ROOT/transaction"
mkdir -p "$tx"
printf 'old config\n' >"$tx/rclone.conf"
printf 'old env\n' >"$tx/rc.env"
printf 'old unit\n' >"$tx/service.unit"
printf 'new config\n' >"$tx/config.new"
printf 'new env\n' >"$tx/env.new"
printf 'new unit\n' >"$tx/unit.new"
restart_calls=0
service_active=1
systemctl() {
  case "$1" in
    restart) restart_calls=$((restart_calls + 1)); if ((restart_calls == 1)); then return 1; fi; service_active=1 ;;
    stop) service_active=0 ;;
    start) service_active=1 ;;
    is-active) [[ "$service_active" == 1 ]] ;;
    is-enabled) return 0 ;;
    *) return 0 ;;
  esac
}
if mistborn_rclone_commit_staged_files "$tx/config.new" "$tx/env.new" "$tx/unit.new" \
  "$tx/rclone.conf" "$tx/rc.env" "$tx/service.unit" 1 1; then
  printf 'expected staged service restart failure\n' >&2
  exit 1
fi
[[ "$(<"$tx/rclone.conf")" == 'old config' ]]
[[ "$(<"$tx/rc.env")" == 'old env' ]]
[[ "$(<"$tx/service.unit")" == 'old unit' ]]
[[ "$service_active" == 1 && "$restart_calls" -ge 2 ]]

printf 'rclone Unix socket service tests passed\n'
