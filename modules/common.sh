# shellcheck shell=bash

module_common_description="System prerequisites"

module_common_apply() {
  ui_step "$module_common_description"
  mistborn_require_root
  mistborn_run apt-get update
  mistborn_apt_install ca-certificates curl git
  ui_success "$module_common_description"
}
