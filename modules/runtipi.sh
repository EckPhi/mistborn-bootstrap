# shellcheck shell=bash

module_runtipi_description="Runtipi"

module_runtipi_apply() {
  ui_step "$module_runtipi_description"
  mistborn_task_start install
  if command -v runtipi-cli >/dev/null 2>&1 || [[ -x /opt/runtipi/runtipi-cli ]]; then
    ui_info "Runtipi already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would run the official Runtipi installer"
  else
    curl -fsSL https://setup.runtipi.io | bash
  fi
  mistborn_task_complete install
  ui_success "$module_runtipi_description"
}
