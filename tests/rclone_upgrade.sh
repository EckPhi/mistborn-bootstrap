#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fixture="$(mktemp -d)"
trap 'rm -rf -- "$fixture"' EXIT
# shellcheck disable=SC1091
source "$root/lib/ui.sh"
# shellcheck disable=SC1091
source "$root/lib/system.sh"
# shellcheck disable=SC1091
source "$root/modules/rclone.sh"

events="$fixture/events"
selected_binary=/usr/bin/rclone
installed_version=1.75.1
apt_installed=1
preview='Remv rclone [1.60.1]'
installer_status=0
confirmation=0
capability_ok=1
remove_status=0
export installer_status events

ui_error() { printf 'error:%s\n' "$*" >>"$fixture/diagnostics"; }
ui_warn() { printf 'warn:%s\n' "$*" >>"$fixture/diagnostics"; }
ui_info() { printf 'info:%s\n' "$*" >>"$fixture/diagnostics"; }
ui_step() { :; }
ui_success() { :; }
mistborn_apt_install() { printf 'apt-install %s\n' "$*" >>"$events"; }
mistborn_rclone_apt_installed() { [[ "$apt_installed" == 1 ]]; }
mistborn_rclone_confirm_apt_removal() {
  printf 'confirm-removal\n' >>"$events"
  return "$confirmation"
}
mistborn_rclone_download_installer() {
  # Variables are expanded by the generated fixture script.
  # shellcheck disable=SC2016
  printf '#!/usr/bin/env bash\nprintf "official-installer\\n" >>"$events"\nexit "$installer_status"\n' >"$1"
}
mistborn_rclone_find_binary() { printf '%s\n' "$selected_binary"; }
mistborn_rclone_read_version() { printf '%s\n' "$installed_version"; }
mistborn_rclone_verify_service_capability() { [[ "$capability_ok" == 1 ]]; }
apt-get() {
  case "$*" in
    '-s remove rclone') printf '%s\n' "$preview" ;;
    'remove -y rclone') printf 'apt-remove\n' >>"$events"; return "$remove_status" ;;
    'install -y --no-install-recommends rclone') printf 'apt-restore\n' >>"$events" ;;
    *) printf 'unexpected apt-get call: %s\n' "$*" >&2; return 1 ;;
  esac
}

# Current rclone keeps global flags out of `--help` and exposes them under
# `help flags`; inspect the real capability function without starting RC.
fake_binary="$fixture/rclone"
# The fake binary expands this variable when the fixture runs, not here.
# shellcheck disable=SC2016
printf '%s\n' \
  '#!/usr/bin/env bash' \
  'case "$*" in' \
  '  "rcd --help") printf "%s\n" "--rc-addr --rc-user --rc-pass" ;;' \
  '  "--help") printf "%s\n" "Use rclone help flags for global options" ;;' \
  '  "help flags") if [[ "${RCLONE_TEST_MISSING_CONFIG:-0}" == 1 ]]; then printf "%s\n" "no config flag"; else printf "%s\n" "--config string"; fi ;;' \
  '  *) exit 1 ;;' \
  'esac' >"$fake_binary"
chmod 0755 "$fake_binary"
(
  # shellcheck disable=SC1091
  source "$root/modules/rclone.sh"
  mistborn_rclone_verify_service_capability "$fake_binary"
  if RCLONE_TEST_MISSING_CONFIG=1 mistborn_rclone_verify_service_capability "$fake_binary" 2>/dev/null; then
    printf 'missing global config flag unexpectedly passed\n' >&2
    exit 1
  fi
)

mistborn_rclone_version_at_least 1.69.0 1.69.0
if mistborn_rclone_version_at_least 1.68.9 1.69.0; then exit 1; fi
if mistborn_rclone_version_at_least 1.69.0-beta.1 1.69.0; then exit 1; fi
mistborn_rclone_version_at_least 1.69.0+build.2 1.69.0

# Only the apt-owned package may be removed, after explicit approval.
mistborn_rclone_install_official
[[ "$(<"$events")" == $'apt-install unzip\nconfirm-removal\napt-remove\nofficial-installer' ]]

# Extra removals are refused before confirmation or mutation.
: >"$events"
preview=$'Remv rclone [1.60.1]\nRemv another-package [1.0]'
if mistborn_rclone_install_official; then exit 1; fi
[[ "$(<"$events")" == 'apt-install unzip' ]]
[[ "$(<"$fixture/diagnostics")" == *'would also remove another-package'* ]]

# A declined migration must not remove the apt package.
: >"$events"
preview='Remv rclone [1.60.1]'
confirmation=1
if mistborn_rclone_install_official; then exit 1; fi
[[ "$(<"$events")" == $'apt-install unzip\nconfirm-removal' ]]

# A partial apt removal failure also attempts package restoration.
: >"$events"
confirmation=0 remove_status=1
if mistborn_rclone_install_official; then exit 1; fi
[[ "$(<"$events")" == $'apt-install unzip\nconfirm-removal\napt-remove\napt-restore' ]]
remove_status=0

# Failed upstream installation restores the apt package.
: >"$events"
confirmation=0 installer_status=1
export installer_status
if mistborn_rclone_install_official; then exit 1; fi
[[ "$(<"$events")" == $'apt-install unzip\nconfirm-removal\napt-remove\nofficial-installer\napt-restore' ]]

# Upstream's "already current" exit code is accepted only after verification.
: >"$events"
installer_status=3
export installer_status
mistborn_rclone_install_official
[[ "$(<"$events")" == *'official-installer' ]]
[[ "$(<"$events")" != *'apt-restore' ]]

# A missing RC capability also triggers rollback after install.
: >"$events"
installer_status=0 capability_ok=0 MISTBORN_RCLONE_SERVICE=1
export installer_status MISTBORN_RCLONE_SERVICE
if mistborn_rclone_install_official; then exit 1; fi
[[ "$(<"$events")" == *'apt-restore' ]]
capability_ok=1

# Fresh hosts do not receive an apt removal prompt.
: >"$events"
apt_installed=0
mistborn_rclone_install_official
[[ "$(<"$events")" == $'apt-install unzip\nofficial-installer' ]]

# --yes cannot bypass a real destructive confirmation without an interactive tty.
if ( source "$root/modules/rclone.sh"; MISTBORN_YES=1 mistborn_rclone_confirm_apt_removal ) </dev/null 2>/dev/null; then
  printf 'noninteractive --yes bypassed apt removal approval\n' >&2
  exit 1
fi

printf 'rclone official installer migration tests passed\n'
