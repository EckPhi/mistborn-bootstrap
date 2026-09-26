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

upstream_dir="$fixture/usr/local/bin"
upstream_binary="$upstream_dir/rclone"
mkdir -p "$upstream_dir"

current_version="1.60.1"
upstream_version="1.75.1"
selected_binary="/usr/bin/rclone"
capability_ok=1
confirmation=0
confirm_calls=0
run_calls=0
apt_calls=0
update_selects_binary=1
diagnostics=""

ui_error() { diagnostics+="error:$*\n"; }
ui_warn() { diagnostics+="warn:$*\n"; }
ui_info() { diagnostics+="info:$*\n"; }
ui_step() { :; }
ui_success() { :; }
ui_confirm() { confirm_calls=$((confirm_calls + 1)); return "$confirmation"; }
mistborn_rclone_read_version() {
  if [[ "$1" == "$upstream_binary" ]]; then printf '%s\n' "$upstream_version"; else printf '%s\n' "$current_version"; fi
}
mistborn_rclone_verify_service_capability() {
  [[ "$capability_ok" == 1 ]] || { ui_error "fixture: missing RC capability"; return 1; }
}
mistborn_rclone_find_binary() { printf '%s\n' "$selected_binary"; }
mistborn_run() {
  run_calls=$((run_calls + 1))
  [[ "$1" == /usr/bin/rclone && "$2" == selfupdate && "$3" == --stable && "$4" == --output && "$5" == "$upstream_binary" ]]
  [[ "$update_selects_binary" == 0 ]] || selected_binary="$upstream_binary"
}

if mistborn_rclone_version_at_least 1.69.0 1.69.0 &&
  ! mistborn_rclone_version_at_least 1.68.9 1.69.0 &&
  ! mistborn_rclone_version_at_least 1.69.0-beta.1 1.69.0 &&
  mistborn_rclone_version_at_least 1.69.0+build.2 1.69.0; then
  :
else
  printf 'rclone version gate comparison failed\n' >&2
  exit 1
fi

# An apt-provided release with selfupdate support asks first, updates to the
# stable path outside dpkg ownership, then verifies version and capabilities.
mistborn_rclone_prepare_service_binary /usr/bin/rclone "$upstream_binary"
[[ "$confirm_calls" == 1 && "$run_calls" == 1 && "$selected_binary" == "$upstream_binary" ]]
[[ "$diagnostics" == *"apt-managed /usr/bin/rclone untouched"* ]]

# A pre-selfupdate apt binary is refused before confirmation or any mutation;
# running the official install script here would overwrite /usr/bin/rclone.
current_version="1.53.3"
selected_binary=/usr/bin/rclone
confirm_calls=0 run_calls=0 diagnostics=""
if mistborn_rclone_prepare_service_binary /usr/bin/rclone "$upstream_binary"; then
  printf 'expected safe refusal for rclone without selfupdate support\n' >&2
  exit 1
fi
[[ "$confirm_calls" == 0 && "$run_calls" == 0 ]]
[[ "$diagnostics" == *"Refusing the official installer because it overwrites apt-managed /usr/bin/rclone"* ]]

# Declining the explicit upgrade must not run it.
current_version="1.60.1"
selected_binary=/usr/bin/rclone
confirm_calls=0 run_calls=0 diagnostics="" confirmation=1
if mistborn_rclone_prepare_service_binary /usr/bin/rclone "$upstream_binary"; then
  printf 'expected refusal after declining upstream update\n' >&2
  exit 1
fi
[[ "$confirm_calls" == 1 && "$run_calls" == 0 ]]

# Capability and PATH checks happen after the verified update.
confirmation=0 capability_ok=0 selected_binary=/usr/bin/rclone run_calls=0 diagnostics=""
if mistborn_rclone_prepare_service_binary /usr/bin/rclone "$upstream_binary"; then
  printf 'expected post-update capability verification failure\n' >&2
  exit 1
fi
[[ "$selected_binary" == "$upstream_binary" && "$run_calls" == 1 ]]

capability_ok=1 selected_binary=/usr/bin/rclone run_calls=0 diagnostics="" update_selects_binary=0
if mistborn_rclone_prepare_service_binary /usr/bin/rclone "$upstream_binary"; then
  printf 'expected PATH mismatch failure\n' >&2
  exit 1
fi
[[ "$diagnostics" == *"PATH selects /usr/bin/rclone instead of the verified service binary"* ]]
update_selects_binary=1

# A capable apt binary remains unchanged: no prompt, selfupdate, or path change.
current_version="1.69.0"
selected_binary=/usr/bin/rclone
confirm_calls=0 run_calls=0 diagnostics=""
mistborn_rclone_prepare_service_binary /usr/bin/rclone "$upstream_binary"
[[ "$confirm_calls" == 0 && "$run_calls" == 0 && "$selected_binary" == /usr/bin/rclone ]]

# Normal rclone install path never probes or upgrades the upstream binary.
mistborn_target_user() { printf 'root\n'; }
mistborn_user_home() { printf '/root\n'; }
mistborn_task_selected() { [[ "$1" == package ]]; }
mistborn_task_start() { :; }
mistborn_task_complete() { :; }
mistborn_task_skip() { :; }
mistborn_apt_install() { apt_calls=$((apt_calls + 1)); }
MISTBORN_TASKS=package MISTBORN_RCLONE_SERVICE=0 MISTBORN_DRY_RUN=0
export MISTBORN_TASKS MISTBORN_RCLONE_SERVICE MISTBORN_DRY_RUN
module_rclone_apply
[[ "$apt_calls" == 1 && "$run_calls" == 0 ]]

# Opt-in dry-run announces the guarded upgrade without probing or mutating host.
apt_calls=0 diagnostics=""
MISTBORN_RCLONE_SERVICE=1 MISTBORN_DRY_RUN=1
module_rclone_apply
if [[ "$apt_calls" != 1 || "$diagnostics" != *"Would verify rclone 1.69.0+"* ]]; then
  printf 'opt-in dry-run did not install the package or announce the version check\n' >&2
  exit 1
fi
[[ "$run_calls" == 0 ]]

printf 'rclone opt-in upgrade tests passed\n'
