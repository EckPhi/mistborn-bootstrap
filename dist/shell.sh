#!/usr/bin/env bash
set -Eeuo pipefail
trap 'ui_error "Setup failed on line $LINENO"' ERR
MISTBORN_DRY_RUN=0
MISTBORN_YES=0
MISTBORN_ONLY=""
MISTBORN_TASKS=""
MISTBORN_MODULES=( common zsh )
export MISTBORN_CONFIG_B64='IyBNaXN0Ym9ybiBkZXNpcmVkLXN0YXRlIGNvbmZpZ3VyYXRpb24uIE9wdGlvbmFsIHNlY3Rpb25zIGFyZSBvbWl0dGVkIHNvIHRoZQojIGhvc3QgcmVtYWlucyB1bm1hbmFnZWQgdW50aWwgYW4gYWRtaW5pc3RyYXRvciBleHBsaWNpdGx5IGFkb3B0cyBhIHBvbGljeS4KdmVyc2lvbiA9IDEKcHJvZmlsZSA9ICJ2cHMiCg=='
export MISTBORN_CONFIG_EXAMPLE_B64='dmVyc2lvbiA9IDEKcHJvZmlsZSA9ICJ2cHMiCgpbc3NoXQpwb3J0ID0gMjIKcGFzc3dvcmRfYXV0aGVudGljYXRpb24gPSBmYWxzZQpyb290X2xvZ2luID0gZmFsc2UKCltmaXJld2FsbF0KZW5hYmxlZCA9IHRydWUKZGVmYXVsdF9pbmNvbWluZyA9ICJkZW55IgpwdWJsaWNfdGNwX3BvcnRzID0gWzgwLCA0NDNdCgpbZmlyZXdhbGwucGxleF0KZW5hYmxlZCA9IHRydWUKcHVibGljX3JlbW90ZV9hY2Nlc3MgPSB0cnVlCmxhbl9jaWRyID0gIjE5Mi4xNjguMS4wLzI0Igp0YWlsc2NhbGUgPSB0cnVlCgpbdGFpbHNjYWxlXQpzc2ggPSB0cnVlCmFkdmVydGlzZV9leGl0X25vZGUgPSBmYWxzZQphdXRvX3VwZGF0ZSA9IHRydWUKCltmYWlsMmJhbl0KZW5hYmxlZCA9IHRydWUKCltmYWlsMmJhbi5zc2hkXQplbmFibGVkID0gdHJ1ZQptYXhyZXRyeSA9IDMKYmFudGltZSA9IDM2MDAgIyBzZWNvbmRzCg=='
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
# shellcheck shell=bash

mistborn_run() {
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    printf '  [dry-run]'; printf ' %q' "$@"; printf '\n'
    return 0
  fi
  "$@"
}

mistborn_has_terminal() {
  [[ -c /dev/tty ]] && ( : </dev/tty ) 2>/dev/null
}

mistborn_run_interactive() {
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    printf '  [dry-run]'; printf ' %q' "$@"; printf '\n'
    return 0
  fi
  if [[ "${MISTBORN_EMBEDDED_TERMINAL:-0}" == 1 ]]; then
    "$@"
    return
  fi
  if ! mistborn_has_terminal; then
    ui_error "This step requires a terminal. Run the downloaded installer directly or use its non-interactive option."
    return 1
  fi
  "$@" </dev/tty
}

mistborn_require_root() {
  [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]] && return 0
  if [[ "$EUID" -ne 0 ]]; then
    ui_error "Run this installer through sudo."
    return 1
  fi
}

mistborn_target_user() {
  if [[ -n "${MISTBORN_USER:-}" ]]; then
    printf '%s\n' "$MISTBORN_USER"
  elif [[ -n "${SUDO_USER:-}" && "$SUDO_USER" != root ]]; then
    printf '%s\n' "$SUDO_USER"
  else
    printf '%s\n' root
  fi
}

mistborn_user_home() {
  if command -v getent >/dev/null 2>&1; then
    getent passwd "$1" | cut -d: -f6
  else
    awk -F: -v user="$1" '$1 == user { print $6 }' /etc/passwd
  fi
}

mistborn_apt_install() {
  DEBIAN_FRONTEND=noninteractive mistborn_run apt-get install -y --no-install-recommends "$@"
}
# shellcheck shell=bash

module_common_description="System prerequisites"

module_common_apply() {
  ui_step "$module_common_description"
  mistborn_require_root
  if mistborn_task_selected apt-index; then
    mistborn_task_start apt-index; mistborn_run apt-get update; mistborn_task_complete apt-index
  fi
  if mistborn_task_selected base-packages; then
    mistborn_task_start base-packages; mistborn_apt_install ca-certificates curl git; mistborn_task_complete base-packages
  fi
  ui_success "$module_common_description"
}
# shellcheck shell=bash

module_zsh_description="Zsh, Oh My Zsh, and Powerlevel10k"

module_zsh_apply() {
  local user home custom_dir zshrc
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }

  ui_step "$module_zsh_description for $user"
  if mistborn_task_selected packages; then
  mistborn_task_start packages
  mistborn_apt_install zsh git
  mistborn_task_complete packages
  fi
  custom_dir="$home/.oh-my-zsh"
  if mistborn_task_selected oh-my-zsh; then
  mistborn_task_start oh-my-zsh
  if [[ ! -d "$custom_dir/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/ohmyzsh/ohmyzsh.git "$custom_dir"
  else
    ui_info "Oh My Zsh already installed"
  fi
  mistborn_task_complete oh-my-zsh
  fi
  if mistborn_task_selected powerlevel10k; then
  mistborn_task_start powerlevel10k
  if [[ ! -d "$custom_dir/custom/themes/powerlevel10k/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/romkatv/powerlevel10k.git \
      "$custom_dir/custom/themes/powerlevel10k"
  else
    ui_info "Powerlevel10k already installed"
  fi
  mistborn_task_complete powerlevel10k
  fi

  if mistborn_task_selected configuration; then
  mistborn_task_start configuration
  zshrc="$home/.zshrc"
  if [[ -f "$zshrc" && ! -f "$zshrc.mistborn-backup" ]]; then
    mistborn_run cp -a "$zshrc" "$zshrc.mistborn-backup"
  fi
  if [[ ! -f "$zshrc" || "${MISTBORN_REPLACE_ZSHRC:-0}" == 1 ]]; then
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would write $zshrc"
    else
      # These variables must expand when Zsh starts, not during installation.
      # shellcheck disable=SC2016
      printf '%s\n' 'export ZSH="$HOME/.oh-my-zsh"' 'ZSH_THEME="powerlevel10k/powerlevel10k"' \
        'plugins=(git sudo)' 'source "$ZSH/oh-my-zsh.sh"' '[[ -f ~/.p10k.zsh ]] && source ~/.p10k.zsh' >"$zshrc"
      chown "$user":"$(id -gn "$user")" "$zshrc"
    fi
  else
    ui_warn "Keeping existing $zshrc; set MISTBORN_REPLACE_ZSHRC=1 to replace it"
  fi
  mistborn_task_complete configuration
  mistborn_run chsh -s "$(command -v zsh)" "$user"
  fi
  ui_success "$module_zsh_description"
}
main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run) MISTBORN_DRY_RUN=1 ;;
      --yes) MISTBORN_YES=1 ;;
      --user) shift; MISTBORN_USER="${1:?--user requires a value}" ;;
      --only) shift; MISTBORN_ONLY="${1:?--only requires a module name}" ;;
      --tasks) shift; MISTBORN_TASKS="${1:?--tasks requires a comma-separated task list}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME] [--only MODULE] [--tasks IDS]\n' "$0"; return ;;
      *) ui_error "Unknown argument: $1"; return 2 ;;
    esac
    shift
  done
  if [[ -n "$MISTBORN_ONLY" ]]; then
    local known=0 module
    for module in "${MISTBORN_MODULES[@]}"; do
      [[ "$module" == "$MISTBORN_ONLY" ]] && known=1
    done
    [[ "$known" == 1 ]] || { ui_error "Unknown module: $MISTBORN_ONLY"; return 2; }
  fi
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Mistborn shell setup"
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != common ]] || module_common_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != zsh ]] || module_zsh_apply
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Setup complete"
}
main "$@"
