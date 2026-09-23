# shellcheck shell=bash

module_rclone_description="rclone"

module_rclone_apply() {
  local user home
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }
  ui_step "$module_rclone_description"
  mistborn_apt_install rclone
  if [[ "${MISTBORN_RCLONE_CONFIGURE:-0}" == 1 ]]; then
    if [[ "$user" == root ]]; then
      mistborn_run_interactive env HOME="$home" rclone config
    else
      mistborn_run_interactive runuser -u "$user" -- env HOME="$home" rclone config
    fi
  fi
  ui_success "$module_rclone_description"
}
