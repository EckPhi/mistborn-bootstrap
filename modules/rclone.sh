# shellcheck shell=bash

module_rclone_description="rclone"

mistborn_rclone_min_service_version="1.69.0"

mistborn_rclone_version_at_least() {
  local actual="$1" required="$2" actual_major actual_minor actual_patch required_major required_minor required_patch actual_suffix required_suffix pair actual_number required_number
  [[ "$actual" =~ ^v?([0-9]+)\.([0-9]+)\.([0-9]+)([-+].*)?$ ]] || return 2
  actual_major="${BASH_REMATCH[1]}" actual_minor="${BASH_REMATCH[2]}" actual_patch="${BASH_REMATCH[3]}" actual_suffix="${BASH_REMATCH[4]:-}"
  [[ "$required" =~ ^v?([0-9]+)\.([0-9]+)\.([0-9]+)([-+].*)?$ ]] || return 2
  required_major="${BASH_REMATCH[1]}" required_minor="${BASH_REMATCH[2]}" required_patch="${BASH_REMATCH[3]}" required_suffix="${BASH_REMATCH[4]:-}"

  for pair in "${actual_major}:${required_major}" "${actual_minor}:${required_minor}" "${actual_patch}:${required_patch}"; do
    actual_number="${pair%%:*}" required_number="${pair#*:}"
    actual_number=$((10#$actual_number)) required_number=$((10#$required_number))
    if (( actual_number > required_number )); then return 0; fi
    if (( actual_number < required_number )); then return 1; fi
  done
  if [[ "$actual_suffix" == -* && -z "$required_suffix" ]]; then return 1; fi
  return 0
}

mistborn_rclone_read_version() {
  local binary="$1" output
  output="$("$binary" --version 2>&1)" || { ui_error "Cannot inspect rclone at $binary: $output"; return 1; }
  output="${output%%$'\n'*}"
  [[ "$output" =~ ^rclone[[:space:]]+v?([0-9]+\.[0-9]+\.[0-9]+([-+][^[:space:]]*)?)$ ]] || {
    ui_error "Cannot parse rclone version from $binary: $output"; return 1;
  }
  printf '%s\n' "${BASH_REMATCH[1]}"
}

mistborn_rclone_verify_service_capability() {
  local binary="$1" rcd_help global_help
  rcd_help="$("$binary" rcd --help 2>&1)" || { ui_error "Cannot inspect rclone rcd support in $binary."; return 1; }
  for flag in --rc-addr --rc-user --rc-pass; do
    [[ "$rcd_help" == *"$flag"* ]] || { ui_error "Rclone at $binary lacks required service option $flag."; return 1; }
  done
  global_help="$("$binary" help flags 2>&1)" || { ui_error "Cannot inspect rclone global options in $binary."; return 1; }
  [[ "$global_help" == *"--config"* ]] || { ui_error "Rclone at $binary lacks the required --config option."; return 1; }
}

mistborn_rclone_find_binary() {
  command -v rclone
}

mistborn_rclone_verify_installed() {
  local binary="$1" require_service="${2:-0}" version selected_path
  version="$(mistborn_rclone_read_version "$binary")" || return 1
  if [[ "$require_service" == 1 ]]; then
    mistborn_rclone_version_at_least "$version" "$mistborn_rclone_min_service_version" || {
      ui_error "rclone $version is too old for the RC socket service; $mistborn_rclone_min_service_version or newer is required."; return 1;
    }
    mistborn_rclone_verify_service_capability "$binary" || return 1
  fi
  selected_path="$(mistborn_rclone_find_binary)" || { ui_error "rclone is not on PATH after capability verification."; return 1; }
  [[ "$selected_path" == "$binary" ]] || {
    ui_error "PATH selects $selected_path instead of the verified rclone binary $binary."; return 1;
  }
}

mistborn_rclone_apt_installed() {
  [[ "$(dpkg-query -W -f='${Status}' rclone 2>/dev/null || true)" == 'install ok installed' ]]
}

mistborn_rclone_removal_is_bounded() {
  local preview="$1" action package found=0
  while read -r action package _; do
    [[ "$action" == Remv ]] || continue
    [[ "$package" == rclone || "$package" == rclone:* ]] || {
      ui_error "Refusing apt removal because it would also remove $package."; return 1;
    }
    found=1
  done <<<"$preview"
  [[ "$found" == 1 ]] || { ui_error "Apt did not preview removal of the installed rclone package."; return 1; }
}

mistborn_rclone_confirm_apt_removal() {
  local reply
  ui_warn "The official rclone installer replaces /usr/bin/rclone; apt ownership must be removed first. Existing rclone configuration is not purged."
  [[ -t 0 ]] || { ui_error "A terminal is required to approve removal of the apt-managed rclone package."; return 1; }
  read -r -p 'Type REMOVE RCLONE to remove only the apt package and install upstream rclone: ' reply
  [[ "$reply" == 'REMOVE RCLONE' ]] || { ui_error "Apt-managed rclone removal was not approved."; return 1; }
}

mistborn_rclone_download_installer() {
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --output "$1" https://rclone.org/install.sh
}

mistborn_rclone_restore_apt() {
  [[ "$1" == 1 ]] || return 0
  ui_warn "Upstream rclone installation failed after apt removal; attempting to restore the apt package."
  apt-get install -y --no-install-recommends rclone || ui_error "Could not restore apt-managed rclone; repair the package manually before retrying."
}

mistborn_rclone_install_official() (
  set -Eeuo pipefail
  local work_dir installer removal_preview binary status had_apt=0
  mistborn_apt_install unzip
  command -v unzip >/dev/null 2>&1 || { ui_error "The official rclone installer requires unzip."; return 1; }
  command -v curl >/dev/null 2>&1 || { ui_error "The official rclone installer requires curl."; return 1; }
  work_dir="$(mktemp -d)" || return 1
  trap 'rm -rf -- "$work_dir"' EXIT
  installer="$work_dir/install.sh"
  mistborn_rclone_download_installer "$installer" || { ui_error "Could not download the official rclone installer."; return 1; }
  [[ -s "$installer" ]] || { ui_error "The downloaded rclone installer is empty."; return 1; }
  if mistborn_rclone_apt_installed; then
    had_apt=1
    removal_preview="$(LC_ALL=C apt-get -s remove rclone)" || { ui_error "Cannot preview apt removal of rclone."; return 1; }
    mistborn_rclone_removal_is_bounded "$removal_preview" || return 1
    mistborn_rclone_confirm_apt_removal || return 1
    apt-get remove -y rclone || {
      ui_error "Could not remove apt-managed rclone."
      mistborn_rclone_restore_apt "$had_apt"
      return 1
    }
  fi
  if bash "$installer"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" != 0 && "$status" != 3 ]]; then
    ui_error "The official rclone installer failed (exit $status)."
    mistborn_rclone_restore_apt "$had_apt"
    return 1
  fi
  hash -r
  binary="$(mistborn_rclone_find_binary)" || {
    ui_error "rclone is not on PATH after the official installer completed."
    mistborn_rclone_restore_apt "$had_apt"
    return 1
  }
  if ! mistborn_rclone_verify_installed "$binary" "${MISTBORN_RCLONE_SERVICE:-0}"; then
    mistborn_rclone_restore_apt "$had_apt"
    return 1
  fi
)

module_rclone_apply() {
  local user home
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }
  ui_step "$module_rclone_description"
  if mistborn_task_selected package; then
    mistborn_task_start package
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would install unzip, download the official rclone installer, and inspect apt ownership before any confirmed package removal"
      ui_info "Would verify installed rclone and RC socket support when service mode is selected"
    else
      mistborn_rclone_install_official || return 1
    fi
    mistborn_task_complete package
  fi
  if mistborn_task_selected configuration && [[ "${MISTBORN_RCLONE_SERVICE:-0}" == 1 ]]; then
    mistborn_task_skip configuration
  elif mistborn_task_selected configuration && [[ "${MISTBORN_RCLONE_CONFIGURE:-0}" == 1 ]]; then
    mistborn_task_start configuration
    if [[ "$user" == root ]]; then
      mistborn_run_interactive env HOME="$home" rclone config
    else
      mistborn_run_interactive runuser -u "$user" -- env HOME="$home" rclone config
    fi
    mistborn_task_complete configuration
  elif mistborn_task_selected configuration; then
    mistborn_task_skip configuration
  fi
  ui_success "$module_rclone_description"
}
