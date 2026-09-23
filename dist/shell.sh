#!/usr/bin/env bash
set -Eeuo pipefail
trap 'ui_error "Setup failed on line $LINENO"' ERR
MISTBORN_DRY_RUN=0
MISTBORN_YES=0
MISTBORN_ONLY=""
MISTBORN_MODULES=( common zsh )
export MISTBORN_TOOL_B64='IyEvdXNyL2Jpbi9lbnYgYmFzaApzZXQgLUVldW8gcGlwZWZhaWwKClJVTlRJUElfUEFUSD0iJHtSVU5USVBJX1BBVEg6LS9vcHQvcnVudGlwaX0iCgpkaWUoKSB7IHByaW50ZiAnZXJyb3I6ICVzXG4nICIkKiIgPiYyOyBleGl0IDE7IH0KcnVudGlwaV9jbGkoKSB7CiAgaWYgW1sgLXggIiRSVU5USVBJX1BBVEgvcnVudGlwaS1jbGkiIF1dOyB0aGVuIHByaW50ZiAnJXNcbicgIiRSVU5USVBJX1BBVEgvcnVudGlwaS1jbGkiCiAgZWxpZiBjb21tYW5kIC12IHJ1bnRpcGktY2xpID4vZGV2L251bGw7IHRoZW4gY29tbWFuZCAtdiBydW50aXBpLWNsaQogIGVsc2UgZGllICJydW50aXBpLWNsaSBub3QgZm91bmQgdW5kZXIgJFJVTlRJUElfUEFUSCBvciBQQVRIIjsgZmkKfQpydW5fcnVudGlwaSgpIHsgIiQocnVudGlwaV9jbGkpIiAiJEAiOyB9CmFwcF9yZWZzKCkgewogIGxvY2FsIHN0b3JlIGFwcAogIGZvciBzdG9yZSBpbiAiJFJVTlRJUElfUEFUSCIvYXBwcy8qOyBkbwogICAgW1sgLWQgIiRzdG9yZSIgXV0gfHwgY29udGludWUKICAgIGZvciBhcHAgaW4gIiRzdG9yZSIvKjsgZG8gW1sgLWQgIiRhcHAiIF1dICYmIHByaW50ZiAnJXM6JXNcbicgIiQoYmFzZW5hbWUgIiRhcHAiKSIgIiQoYmFzZW5hbWUgIiRzdG9yZSIpIjsgZG9uZQogIGRvbmUKfQpzbmFwc2hvdF9hcHBzKCkgeyBsb2NhbCByZWY7IHdoaWxlIElGUz0gcmVhZCAtciByZWY7IGRvIHJ1bl9ydW50aXBpIGFwcCBiYWNrdXAgIiRyZWYiOyBkb25lOyB9CmRvY3RvcigpIHsKICBsb2NhbCBmYWlsdXJlcz0wCiAgcHJpbnRmICdNaXN0Ym9ybiBob3N0IGRpYWdub3N0aWNzXG4nCiAgZm9yIGNvbW1hbmQgaW4gZG9ja2VyIHRhaWxzY2FsZSByY2xvbmUgdWZ3IGZhaWwyYmFuLWNsaWVudDsgZG8KICAgIGlmIGNvbW1hbmQgLXYgIiRjb21tYW5kIiA+L2Rldi9udWxsIDI+JjE7IHRoZW4gcHJpbnRmICcgIFBBU1MgICVzIGluc3RhbGxlZFxuJyAiJGNvbW1hbmQiOyBlbHNlIHByaW50ZiAnICBXQVJOICAlcyBtaXNzaW5nXG4nICIkY29tbWFuZCI7IGZpCiAgZG9uZQogIGlmIFtbIC1kICIkUlVOVElQSV9QQVRIIiBdXTsgdGhlbiBwcmludGYgJyAgUEFTUyAgUnVudGlwaSBkaXJlY3Rvcnk6ICVzXG4nICIkUlVOVElQSV9QQVRIIjsgZWxzZSBwcmludGYgJyAgRkFJTCAgUnVudGlwaSBkaXJlY3RvcnkgbWlzc2luZ1xuJzsgZmFpbHVyZXM9MTsgZmkKICBpZiBkb2NrZXIgaW5mbyA+L2Rldi9udWxsIDI+JjE7IHRoZW4gcHJpbnRmICcgIFBBU1MgIERvY2tlciBkYWVtb24gcmVhY2hhYmxlXG4nOyBlbHNlIHByaW50ZiAnICBGQUlMICBEb2NrZXIgZGFlbW9uIHVucmVhY2hhYmxlXG4nOyBmYWlsdXJlcz0xOyBmaQogIGlmIHN5c3RlbWN0bCBpcy1hY3RpdmUgLS1xdWlldCBmYWlsMmJhbjsgdGhlbiBwcmludGYgJyAgUEFTUyAgZmFpbDJiYW4gYWN0aXZlXG4nOyBlbHNlIHByaW50ZiAnICBXQVJOICBmYWlsMmJhbiBpbmFjdGl2ZVxuJzsgZmkKICBpZiB1Zncgc3RhdHVzIDI+L2Rldi9udWxsIHwgZ3JlcCAtcSAnU3RhdHVzOiBhY3RpdmUnOyB0aGVuIHByaW50ZiAnICBQQVNTICBVRlcgYWN0aXZlXG4nOyBlbHNlIHByaW50ZiAnICBXQVJOICBVRlcgaW5hY3RpdmVcbic7IGZpCiAgcmV0dXJuICIkZmFpbHVyZXMiCn0KdXBkYXRlX2FwcHMoKSB7CiAgbG9jYWwgYmFja3VwPTEgcmVmcz0oKSByZWYKICBbWyAiJHsxOi19IiA9PSAtLW5vLWJhY2t1cCBdXSAmJiB7IGJhY2t1cD0wOyBzaGlmdDsgfQogIGlmIFtbICQjIC1ndCAwIF1dOyB0aGVuIHJlZnM9KCIkQCIpOyBlbHNlIG1hcGZpbGUgLXQgcmVmcyA8IDwoYXBwX3JlZnMpOyBmaQogIGZvciByZWYgaW4gIiR7cmVmc1tAXX0iOyBkbyBbWyAiJGJhY2t1cCIgPT0gMSBdXSAmJiBydW5fcnVudGlwaSBhcHAgYmFja3VwICIkcmVmIjsgcnVuX3J1bnRpcGkgYXBwIHVwZGF0ZSAiJHJlZiI7IGRvbmUKfQp1c2FnZSgpIHsKICBjYXQgPDwnRU9GJwpVc2FnZTogbWlzdGJvcm4gQ09NTUFORCBbQVJHU10KICBkb2N0b3IgICAgICAgICAgICAgICAgICAgICAgIGF1ZGl0IERvY2tlciwgUnVudGlwaSwgVGFpbHNjYWxlLCByY2xvbmUgYW5kIHNlY3VyaXR5CiAgc2VjdXJpdHktc3RhdHVzICAgICAgICAgICAgICBzaG93IFNTSCwgVUZXLCBmYWlsMmJhbiBhbmQgVGFpbHNjYWxlIHN0YXR1cwogIHRhaWxzY2FsZS1zdGF0dXMgICAgICAgICAgICAgc2hvdyBUYWlsc2NhbGUgc3RhdHVzCiAgcmNsb25lLWNvbmZpZyAgICAgICAgICAgICAgICBvcGVuIHJjbG9uZSdzIGNvbmZpZ3VyYXRpb24gVUkKICB1cGRhdGUtYXBwcyBbLS1uby1iYWNrdXBdIFtBUFA6U1RPUkUgLi4uXQogIHVwZGF0ZS1jb3JlIFstLW5vLWJhY2t1cF0gW1ZFUlNJT05dCiAgdXBkYXRlLWFwcHN0b3JlcwpFT0YKfQpjYXNlICIkezE6LX0iIGluCiAgZG9jdG9yKSBkb2N0b3IgOzsKICBzZWN1cml0eS1zdGF0dXMpIHNzaGQgLVQgMj4vZGV2L251bGwgfCBncmVwIC1FICdwYXNzd29yZGF1dGhlbnRpY2F0aW9ufHBlcm1pdHJvb3Rsb2dpbnxecG9ydCc7IHVmdyBzdGF0dXMgdmVyYm9zZTsgZmFpbDJiYW4tY2xpZW50IHN0YXR1cyBzc2hkIHx8IHRydWU7IHRhaWxzY2FsZSBzdGF0dXMgfHwgdHJ1ZSA7OwogIHRhaWxzY2FsZS1zdGF0dXMpIHRhaWxzY2FsZSBzdGF0dXMgOzsKICByY2xvbmUtY29uZmlnKSByY2xvbmUgY29uZmlnIDs7CiAgdXBkYXRlLWFwcHMpIHNoaWZ0OyB1cGRhdGVfYXBwcyAiJEAiIDs7CiAgdXBkYXRlLWNvcmUpIHNoaWZ0OyBiYWNrdXA9MTsgW1sgIiR7MTotfSIgPT0gLS1uby1iYWNrdXAgXV0gJiYgeyBiYWNrdXA9MDsgc2hpZnQ7IH07IFtbICIkYmFja3VwIiA9PSAxIF1dICYmIHNuYXBzaG90X2FwcHMgPCA8KGFwcF9yZWZzKTsgcnVuX3J1bnRpcGkgdXBkYXRlICIkezE6LWxhdGVzdH0iIDs7CiAgdXBkYXRlLWFwcHN0b3JlcykgcnVuX3J1bnRpcGkgYXBwc3RvcmUgdXBkYXRlIDs7CiAgLWh8LS1oZWxwfCcnKSB1c2FnZSA7OwogICopIGRpZSAidW5rbm93biBjb21tYW5kOiAkMSIgOzsKZXNhYwo='
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

mistborn_has_terminal() {
  [[ -c /dev/tty ]] && ( : </dev/tty ) 2>/dev/null
}

mistborn_run_interactive() {
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    printf '  [dry-run]'; printf ' %q' "$@"; printf '\n'
    return 0
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
      --only) shift; MISTBORN_ONLY="${1:?--only requires a module name}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME] [--only MODULE]\n' "$0"; return ;;
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
  ui_header "Mistborn shell setup"
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != common ]] || module_common_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != zsh ]] || module_zsh_apply
  ui_header "Setup complete"
}
main "$@"
