# shellcheck shell=bash

ui_is_terminal() { [[ -t 1 && -z "${NO_COLOR:-}" ]]; }

ui_color() {
  local code="$1"
  if ui_is_terminal; then printf '\033[%sm' "$code"; fi
}

ui_reset() { ui_color 0; }
ui_header() { printf '\n%s%s%s\n\n' "$(ui_color '1;36')" "$1" "$(ui_reset)"; }
ui_info() { printf '  %s•%s %s\n' "$(ui_color 36)" "$(ui_reset)" "$1"; }
ui_success() { printf '  %s✓%s %s\n' "$(ui_color 32)" "$(ui_reset)" "$1"; }
ui_warn() { printf '  %s!%s %s\n' "$(ui_color 33)" "$(ui_reset)" "$1" >&2; }
ui_error() { printf '  %s✗%s %s\n' "$(ui_color 31)" "$(ui_reset)" "$1" >&2; }
ui_step() { printf '\n%s==>%s %s\n' "$(ui_color '1;34')" "$(ui_reset)" "$1"; }

ui_confirm() {
  local prompt="$1" reply
  [[ "${MISTBORN_YES:-0}" == 1 ]] && return 0
  [[ -t 0 ]] || return 1
  read -r -p "$prompt [y/N] " reply
  [[ "$reply" =~ ^[Yy]$ ]]
}
