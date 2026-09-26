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
  global_help="$("$binary" --help 2>&1)" || { ui_error "Cannot inspect rclone global options in $binary."; return 1; }
  [[ "$global_help" == *"--config"* ]] || { ui_error "Rclone at $binary lacks the required --config option."; return 1; }
}

mistborn_rclone_find_binary() {
  command -v rclone
}

mistborn_rclone_prepare_service_binary() {
  local binary="$1" upstream_binary="$2" version
  version="$(mistborn_rclone_read_version "$binary")" || return 1
  if ! mistborn_rclone_version_at_least "$version" "$mistborn_rclone_min_service_version"; then
    if ! mistborn_rclone_version_at_least "$version" "1.55.0"; then
      ui_error "rclone $version is too old for the verified self-update command (requires 1.55.0+). Refusing the official installer because it overwrites apt-managed /usr/bin/rclone; update through a trusted package that installs to /usr/local/bin/rclone, then retry."
      return 1
    fi
    [[ -d "${upstream_binary%/*}" && ! -L "${upstream_binary%/*}" ]] || {
      ui_error "Cannot safely use the stable upstream binary directory ${upstream_binary%/*}."; return 1;
    }
    [[ ! -L "$upstream_binary" ]] || { ui_error "Refusing symlinked upstream rclone target $upstream_binary."; return 1; }
    [[ ! -e "$upstream_binary" || -f "$upstream_binary" ]] || { ui_error "Refusing non-file upstream rclone target $upstream_binary."; return 1; }

    ui_warn "Rclone $version is below $mistborn_rclone_min_service_version, required for the optional authenticated Unix-socket service."
    ui_info "The official verified stable updater will install to $upstream_binary, leaving apt-managed /usr/bin/rclone untouched. Future apt upgrades will not replace this service binary."
    ui_confirm "Install the upstream stable rclone binary at $upstream_binary?" || {
      ui_error "Rclone service setup requires explicit approval to install the upstream binary."; return 1;
    }
    mistborn_run "$binary" selfupdate --stable --output "$upstream_binary" || {
      ui_error "The verified upstream rclone self-update failed."; return 1;
    }
    hash -r
    binary="$upstream_binary"
    version="$(mistborn_rclone_read_version "$binary")" || return 1
    mistborn_rclone_version_at_least "$version" "$mistborn_rclone_min_service_version" || {
      ui_error "Upstream rclone update left version $version installed; $mistborn_rclone_min_service_version or newer is required."; return 1;
    }
  fi

  mistborn_rclone_verify_service_capability "$binary" || return 1
  local selected_path
  selected_path="$(mistborn_rclone_find_binary)" || { ui_error "rclone is not on PATH after capability verification."; return 1; }
  [[ "$selected_path" == "$binary" ]] || {
    ui_error "PATH selects $selected_path instead of the verified service binary $binary. Put /usr/local/bin before /usr/bin and retry."; return 1;
  }
}

module_rclone_apply() {
  local user home
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }
  ui_step "$module_rclone_description"
  if mistborn_task_selected package; then
    mistborn_task_start package
    mistborn_apt_install rclone
    if [[ "${MISTBORN_RCLONE_SERVICE:-0}" == 1 ]]; then
      if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
        ui_info "Would verify rclone $mistborn_rclone_min_service_version+ and, if needed, request approval before installing the verified stable binary at /usr/local/bin/rclone"
      else
        local rclone_binary
        rclone_binary="$(mistborn_rclone_find_binary)" || { ui_error "rclone is not installed after apt completed."; return 1; }
        mistborn_rclone_prepare_service_binary "$rclone_binary" /usr/local/bin/rclone || return 1
      fi
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
