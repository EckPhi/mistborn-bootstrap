# shellcheck shell=bash

module_common_description="System prerequisites"

module_common_apply() {
  ui_step "$module_common_description"
  mistborn_require_root
  if mistborn_task_selected apt-index; then
    mistborn_task_start apt-index; mistborn_run apt-get update; mistborn_task_complete apt-index
  fi
  if mistborn_task_selected base-packages; then
    mistborn_task_start base-packages; mistborn_apt_install ca-certificates curl git; mistborn_task_complete base-packages
  fi
  ui_success "$module_common_description"
}
