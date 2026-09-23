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
