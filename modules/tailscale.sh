# shellcheck shell=bash

module_tailscale_description="Tailscale"

module_tailscale_apply() {
  local sysctl_file=/etc/sysctl.d/99-mistborn-tailscale.conf
  ui_step "$module_tailscale_description"
  if mistborn_task_selected install; then
  mistborn_task_start install
  if command -v tailscale >/dev/null 2>&1; then
    ui_info "Tailscale already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install Tailscale from packages.tailscale.com"
  else
    curl -fsSL https://tailscale.com/install.sh | sh
  fi
  mistborn_task_complete install
  fi
  if mistborn_task_selected forwarding && [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]]; then
    mistborn_task_start forwarding
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would enable persistent IPv4 and IPv6 forwarding in $sysctl_file"
    else
      printf '%s\n' 'net.ipv4.ip_forward = 1' 'net.ipv6.conf.all.forwarding = 1' >"$sysctl_file"
      sysctl -p "$sysctl_file"
    fi
    mistborn_task_complete forwarding
  elif mistborn_task_selected forwarding; then
    mistborn_task_skip forwarding
  fi
  local args=(up)
  [[ "${MISTBORN_TAILSCALE_SSH:-0}" == 1 ]] && args+=(--ssh)
  [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]] && args+=(--advertise-exit-node)
  if mistborn_task_selected connect; then
  mistborn_task_start connect
  if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
    args+=(--auth-key "$TAILSCALE_AUTH_KEY")
    mistborn_run tailscale "${args[@]}"
  else
    mistborn_run_interactive tailscale "${args[@]}"
  fi
  mistborn_task_complete connect
  fi
  if mistborn_task_selected auto-update && [[ "${MISTBORN_TAILSCALE_AUTO_UPDATE:-0}" == 1 ]]; then
    mistborn_task_start auto-update
    mistborn_run tailscale set --auto-update
    mistborn_task_complete auto-update
  elif mistborn_task_selected auto-update; then
    mistborn_task_skip auto-update
  fi
  ui_success "$module_tailscale_description"
}
