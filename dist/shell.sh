#!/usr/bin/env bash
set -Eeuo pipefail
trap 'ui_error "Setup failed on line $LINENO"' ERR
MISTBORN_DRY_RUN=0
MISTBORN_YES=0
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
# shellcheck shell=bash

mistborn_run() {
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    printf '  [dry-run]'; printf ' %q' "$@"; printf '\n'
    return 0
  fi
  "$@"
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
  mistborn_run apt-get update
  mistborn_apt_install ca-certificates curl git
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
  mistborn_apt_install zsh git
  custom_dir="$home/.oh-my-zsh"
  if [[ ! -d "$custom_dir/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/ohmyzsh/ohmyzsh.git "$custom_dir"
  else
    ui_info "Oh My Zsh already installed"
  fi
  if [[ ! -d "$custom_dir/custom/themes/powerlevel10k/.git" ]]; then
    mistborn_run sudo -u "$user" git clone --depth 1 https://github.com/romkatv/powerlevel10k.git \
      "$custom_dir/custom/themes/powerlevel10k"
  else
    ui_info "Powerlevel10k already installed"
  fi

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
  mistborn_run chsh -s "$(command -v zsh)" "$user"
  ui_success "$module_zsh_description"
}
main() {
  while [[ $# -gt 0 ]]; do
    case "$1" in
      --dry-run) MISTBORN_DRY_RUN=1 ;;
      --yes) MISTBORN_YES=1 ;;
      --user) shift; MISTBORN_USER="${1:?--user requires a value}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME]\n' "$0"; return ;;
      *) ui_error "Unknown argument: $1"; return 2 ;;
    esac
    shift
  done
  ui_header "Mistborn shell setup"
  module_common_apply
  module_zsh_apply
  ui_header "Setup complete"
}
main "$@"
