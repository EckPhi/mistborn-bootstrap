# shellcheck shell=bash

module_companion_description="Runtipi Companion CLI"

module_companion_apply() {
  local user
  user="$(mistborn_target_user)"
  ui_step "$module_companion_description"
  mistborn_apt_install pipx
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install Runtipi Companion for $user"
  elif sudo -H -u "$user" bash -c 'command -v runtipi-companion >/dev/null 2>&1'; then
    mistborn_run sudo -H -u "$user" pipx upgrade runtipi-companion
  else
    mistborn_run sudo -H -u "$user" pipx install runtipi-companion
  fi
  ui_success "$module_companion_description"
}
