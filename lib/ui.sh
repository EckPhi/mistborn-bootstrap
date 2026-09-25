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

mistborn_task_event() {
  local stage="$1" task="$2" state="$3"
  [[ -n "${MISTBORN_PROGRESS_FILE:-}" ]] || return 0
  printf '%s\t%s\t%s\n' "$stage" "$task" "$state" >>"$MISTBORN_PROGRESS_FILE" || true
}
mistborn_task_start() { mistborn_task_event "${MISTBORN_PROGRESS_STAGE:-}" "$1" started; }
mistborn_task_complete() { mistborn_task_event "${MISTBORN_PROGRESS_STAGE:-}" "$1" completed; }
mistborn_task_skip() { mistborn_task_event "${MISTBORN_PROGRESS_STAGE:-}" "$1" skipped; }

mistborn_task_selected() {
  local task="$1"
  [[ -z "${MISTBORN_TASKS:-}" || ",${MISTBORN_TASKS}," == *",${task},"* ]]
}

ui_confirm() {
  local prompt="$1" reply
  [[ "${MISTBORN_YES:-0}" == 1 ]] && return 0
  [[ -t 0 ]] || return 1
  read -r -p "$prompt [y/N] " reply
  [[ "$reply" =~ ^[Yy]$ ]]
}
