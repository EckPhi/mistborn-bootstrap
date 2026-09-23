# shellcheck shell=bash

module_docker_description="Docker Engine"

module_docker_apply() {
  ui_step "$module_docker_description"
  mistborn_task_start engine
  if command -v docker >/dev/null 2>&1; then
    ui_info "Docker already installed"
  else
    mistborn_apt_install docker.io
  fi
  mistborn_task_complete engine
  mistborn_task_start service
  mistborn_run systemctl enable --now docker
  mistborn_task_complete service
  ui_success "$module_docker_description"
}
