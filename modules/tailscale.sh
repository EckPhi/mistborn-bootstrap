# shellcheck shell=bash

module_tailscale_description="Tailscale"

module_tailscale_apply() {
  ui_step "$module_tailscale_description"
  if command -v tailscale >/dev/null 2>&1; then
    ui_info "Tailscale already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install Tailscale from packages.tailscale.com"
  else
    curl -fsSL https://tailscale.com/install.sh | sh
  fi
  local args=(up)
  [[ "${MISTBORN_TAILSCALE_SSH:-0}" == 1 ]] && args+=(--ssh)
  [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]] && args+=(--advertise-exit-node)
  if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
    args+=(--auth-key "$TAILSCALE_AUTH_KEY")
    mistborn_run tailscale "${args[@]}"
  else
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then ui_info "Would run tailscale ${args[*]}"
    elif [[ -t 0 ]]; then tailscale "${args[@]}"
    else ui_warn "Run 'sudo tailscale ${args[*]}' to authenticate"; fi
  fi
  ui_success "$module_tailscale_description"
}
