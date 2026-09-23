# shellcheck shell=bash

module_rclone_description="rclone"

module_rclone_apply() {
  local user
  user="$(mistborn_target_user)"
  ui_step "$module_rclone_description"
  mistborn_apt_install rclone
  if [[ "${MISTBORN_RCLONE_CONFIGURE:-0}" == 1 ]]; then
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would launch rclone config for $user"
    elif [[ -t 0 ]]; then
      sudo -H -u "$user" rclone config
    else
      ui_warn "rclone config needs a terminal; run it later as $user"
    fi
  fi
  ui_success "$module_rclone_description"
}
