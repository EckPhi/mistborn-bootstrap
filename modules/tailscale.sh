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
  if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
    mistborn_run tailscale up --auth-key "$TAILSCALE_AUTH_KEY"
  else
    ui_warn "Run 'sudo tailscale up' to authenticate"
  fi
  ui_success "$module_tailscale_description"
}
