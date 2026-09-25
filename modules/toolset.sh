# shellcheck shell=bash

module_toolset_description="Mistborn host-management commands"

mistborn_config_task_applied() {
  [[ ",${MISTBORN_CONFIG_APPLIED_TASKS:-}," == *",$1,"* ]]
}

mistborn_config_bool() {
  local value="${1:-0}"
  [[ "$value" == 0 || "$value" == 1 ]] || return 1
  [[ "$value" == 1 ]] && printf 'true' || printf 'false'
}

mistborn_render_desired_config() {
  local output="$1" port first value
  printf '%s' "$MISTBORN_CONFIG_B64" | base64 -d >"$output" || return 1
  [[ "${MISTBORN_CONFIG_ADOPTION_ALLOWED:-0}" == 1 ]] || return 0

  if [[ "${MISTBORN_HARDEN+x}" == x && "${MISTBORN_HARDEN:-0}" == 1 ]] \
    && mistborn_config_task_applied security/ssh; then
    port="${MISTBORN_SSH_PORT:-22}"
    [[ "$port" =~ ^[0-9]+$ ]] && ((10#$port >= 1 && 10#$port <= 65535)) || return 1
    printf '\n[ssh]\nport = %s\npassword_authentication = false\nroot_login = false\n' "$port" >>"$output"
  fi

  if [[ "${MISTBORN_HARDEN+x}" == x && "${MISTBORN_HARDEN:-0}" == 1 ]] \
    && mistborn_config_task_applied security/firewall; then
    printf '\n[firewall]\nenabled = true\ndefault_incoming = "deny"\npublic_tcp_ports = [' >>"$output"
    first=1
    for value in ${MISTBORN_ALLOWED_TCP_PORTS:-}; do
      [[ "$value" =~ ^[0-9]+$ ]] && ((10#$value >= 1 && 10#$value <= 65535)) || return 1
      [[ "$first" == 1 ]] || printf ', ' >>"$output"
      printf '%s' "$value" >>"$output"
      first=0
    done
    printf ']\n' >>"$output"
    if [[ "${MISTBORN_PLEX_UFW+x}" == x && "${MISTBORN_PLEX_UFW:-0}" == 1 ]] \
      && mistborn_config_task_applied security/plex-firewall; then
      printf '\n[firewall.plex]\nenabled = true\npublic_remote_access = true\n' >>"$output"
      if [[ -n "${MISTBORN_PLEX_LAN_CIDR:-}" ]]; then
        printf 'lan_cidr = "%s"\n' "$MISTBORN_PLEX_LAN_CIDR" >>"$output"
      fi
      printf 'tailscale = %s\n' "$(mistborn_config_bool "${MISTBORN_PLEX_TAILSCALE:-0}")" >>"$output" || return 1
    fi
  fi

  if mistborn_config_task_applied tailscale/connect \
    && [[ "${MISTBORN_TAILSCALE_SSH+x}${MISTBORN_TAILSCALE_EXIT_NODE+x}${MISTBORN_TAILSCALE_AUTO_UPDATE+x}" == *x* ]]; then
    printf '\n[tailscale]\nssh = %s\nadvertise_exit_node = %s\nauto_update = %s\n' \
      "$(mistborn_config_bool "${MISTBORN_TAILSCALE_SSH:-0}")" \
      "$(mistborn_config_bool "${MISTBORN_TAILSCALE_EXIT_NODE:-0}")" \
      "$(mistborn_config_bool "${MISTBORN_TAILSCALE_AUTO_UPDATE:-0}")" >>"$output" || return 1
  fi
}

mistborn_publish_desired_config() {
  local etc_dir="$1" share_dir="$2" validator="$3" staging_dir config_staging owner
  install -d -m 0755 "$etc_dir" "$share_dir"
  staging_dir="$(mktemp -d "$etc_dir/.install.XXXXXX")"
  config_staging="$staging_dir/config.toml"
  owner="${MISTBORN_CONFIG_OWNER:-root:root}"
  (
    set -Ee
    trap 'rm -f -- "$config_staging" "$staging_dir/config.toml.example"; rmdir "$staging_dir" 2>/dev/null || true' EXIT
    mistborn_render_desired_config "$config_staging" || exit 1
    printf '%s' "$MISTBORN_CONFIG_EXAMPLE_B64" | base64 -d >"$staging_dir/config.toml.example" || exit 1
    "$validator" validate-config "$config_staging" || exit 1
    "$validator" validate-config "$staging_dir/config.toml.example" || exit 1
    chmod 0644 "$config_staging" "$staging_dir/config.toml.example" || exit 1
    chown "$owner" "$config_staging" "$staging_dir/config.toml.example" || exit 1
    if [[ "${MISTBORN_SKIP_FSYNC:-0}" != 1 ]]; then
      sync -f "$config_staging" || exit 1
      sync -f "$staging_dir/config.toml.example" || exit 1
    fi
    mv -f "$staging_dir/config.toml.example" "$share_dir/config.toml.example" || exit 1
    if [[ ! -e "$etc_dir/config.toml" ]]; then
      mv "$config_staging" "$etc_dir/config.toml" || exit 1
    fi
    if [[ "${MISTBORN_SKIP_FSYNC:-0}" != 1 ]]; then
      sync -f "$etc_dir" || exit 1
      sync -f "$share_dir" || exit 1
    fi
  )
}

module_toolset_apply() {
  ui_step "$module_toolset_description"
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install /usr/local/bin/mistborn and its Ratatui runner"
  else
    install -d -m 0755 /usr/local/lib/mistborn
    if mistborn_task_selected runner; then
    mistborn_task_start runner
    if [[ -n "${MISTBORN_RUNNER_BINARY:-}" && -x "$MISTBORN_RUNNER_BINARY" && "$MISTBORN_RUNNER_BINARY" != /usr/local/bin/mistborn-bootstrap ]]; then
      install -m 0755 "$MISTBORN_RUNNER_BINARY" /usr/local/bin/mistborn-bootstrap
      mistborn_task_complete runner
    else
      mistborn_task_skip runner
    fi
    fi
    if mistborn_task_selected command; then
    mistborn_task_start command
    install -d -m 0755 /usr/local/lib/mistborn/plans
    local staging_dir
    staging_dir="$(mktemp -d /usr/local/lib/mistborn/.install.XXXXXX)"
    printf '%s' "$MISTBORN_TOOL_B64" | base64 -d >"$staging_dir/host.sh"
    chmod 0644 "$staging_dir/host.sh"
    printf '%s' "$MISTBORN_UPDATE_PLAN_B64" | base64 -d >"$staging_dir/update.toml"
    chmod 0644 "$staging_dir/update.toml"
    mv -f "$staging_dir/host.sh" /usr/local/lib/mistborn/host.sh
    mv -f "$staging_dir/update.toml" /usr/local/lib/mistborn/plans/update.toml
    rmdir "$staging_dir"
    mistborn_publish_desired_config /etc/mistborn /usr/local/share/mistborn "${MISTBORN_RUNNER_BINARY:-/usr/local/bin/mistborn-bootstrap}"
    cat >/usr/local/bin/mistborn <<'MISTBORN_LAUNCHER'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ -x /usr/local/bin/mistborn-bootstrap ]]; then
  exec /usr/local/bin/mistborn-bootstrap host --script /usr/local/lib/mistborn/host.sh "$@"
fi
exec bash /usr/local/lib/mistborn/host.sh "$@"
MISTBORN_LAUNCHER
    chmod 0755 /usr/local/bin/mistborn
    mistborn_task_complete command
    fi
  fi
  ui_success "$module_toolset_description"
}
