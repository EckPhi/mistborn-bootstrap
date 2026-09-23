# shellcheck shell=bash

module_docker_description="Docker Engine"

module_docker_apply() {
  ui_step "$module_docker_description"
  if command -v docker >/dev/null 2>&1; then
    ui_info "Docker already installed"
  else
    mistborn_apt_install docker.io
  fi
  mistborn_run systemctl enable --now docker
  ui_success "$module_docker_description"
}
