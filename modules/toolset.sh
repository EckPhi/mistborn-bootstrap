# shellcheck shell=bash

module_toolset_description="Mistborn host-management commands"

module_toolset_apply() {
  ui_step "$module_toolset_description"
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install /usr/local/bin/mistborn"
  else
    printf '%s' "$MISTBORN_TOOL_B64" | base64 -d >/usr/local/bin/mistborn
    chmod 0755 /usr/local/bin/mistborn
  fi
  ui_success "$module_toolset_description"
}
