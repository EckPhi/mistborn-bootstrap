# shellcheck shell=bash

module_rclone_description="rclone"

module_rclone_apply() {
  local user
  user="$(mistborn_target_user)"
  ui_step "$module_rclone_description"
  mistborn_apt_install rclone
  if [[ "${MISTBORN_RCLONE_CONFIGURE:-0}" == 1 ]]; then
    mistborn_run_interactive sudo -H -u "$user" rclone config
  fi
  ui_success "$module_rclone_description"
}
