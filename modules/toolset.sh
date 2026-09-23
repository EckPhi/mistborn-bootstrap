# shellcheck shell=bash

module_toolset_description="Mistborn host-management commands"

module_toolset_apply() {
  ui_step "$module_toolset_description"
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install /usr/local/bin/mistborn and its Ratatui runner"
  else
    install -d -m 0755 /usr/local/lib/mistborn
    mistborn_task_start runner
    if [[ -n "${MISTBORN_RUNNER_BINARY:-}" && -x "$MISTBORN_RUNNER_BINARY" && "$MISTBORN_RUNNER_BINARY" != /usr/local/bin/mistborn-bootstrap ]]; then
      install -m 0755 "$MISTBORN_RUNNER_BINARY" /usr/local/bin/mistborn-bootstrap
      mistborn_task_complete runner
    else
      mistborn_task_skip runner
    fi
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
  ui_success "$module_toolset_description"
}
