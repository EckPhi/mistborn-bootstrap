#!/usr/bin/env bash
set -Eeuo pipefail
trap 'ui_error "Setup failed on line $LINENO"' ERR
MISTBORN_DRY_RUN=0
MISTBORN_YES=0
MISTBORN_ONLY=""
MISTBORN_TASKS=""
MISTBORN_MODULES=( common docker zsh tailscale rclone rclone_service runtipi security toolset )
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

module_docker_description="Docker Engine"

module_docker_apply() {
  ui_step "$module_docker_description"
  if mistborn_task_selected engine; then
  mistborn_task_start engine
  if command -v docker >/dev/null 2>&1; then
    ui_info "Docker already installed"
  else
    mistborn_apt_install docker.io
  fi
  mistborn_task_complete engine
  fi
  if mistborn_task_selected service; then
    mistborn_task_start service; mistborn_run systemctl enable --now docker; mistborn_task_complete service
  fi
  ui_success "$module_docker_description"
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
# shellcheck shell=bash

module_tailscale_description="Tailscale"

module_tailscale_apply() {
  local sysctl_file=/etc/sysctl.d/99-mistborn-tailscale.conf
  ui_step "$module_tailscale_description"
  if mistborn_task_selected install; then
  mistborn_task_start install
  if command -v tailscale >/dev/null 2>&1; then
    ui_info "Tailscale already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install Tailscale from packages.tailscale.com"
  else
    curl -fsSL https://tailscale.com/install.sh | sh
  fi
  mistborn_task_complete install
  fi
  if mistborn_task_selected forwarding && [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]]; then
    mistborn_task_start forwarding
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would enable persistent IPv4 and IPv6 forwarding in $sysctl_file"
    else
      printf '%s\n' 'net.ipv4.ip_forward = 1' 'net.ipv6.conf.all.forwarding = 1' >"$sysctl_file"
      sysctl -p "$sysctl_file"
    fi
    mistborn_task_complete forwarding
  elif mistborn_task_selected forwarding; then
    mistborn_task_skip forwarding
  fi
  local args=(up)
  [[ "${MISTBORN_TAILSCALE_SSH:-0}" == 1 ]] && args+=(--ssh)
  [[ "${MISTBORN_TAILSCALE_EXIT_NODE:-0}" == 1 ]] && args+=(--advertise-exit-node)
  if mistborn_task_selected connect; then
  mistborn_task_start connect
  if [[ -n "${TAILSCALE_AUTH_KEY:-}" ]]; then
    args+=(--auth-key "$TAILSCALE_AUTH_KEY")
    mistborn_run tailscale "${args[@]}"
  else
    mistborn_run_interactive tailscale "${args[@]}"
  fi
  mistborn_task_complete connect
  fi
  if mistborn_task_selected auto-update && [[ "${MISTBORN_TAILSCALE_AUTO_UPDATE:-0}" == 1 ]]; then
    mistborn_task_start auto-update
    mistborn_run tailscale set --auto-update
    mistborn_task_complete auto-update
  elif mistborn_task_selected auto-update; then
    mistborn_task_skip auto-update
  fi
  ui_success "$module_tailscale_description"
}
# shellcheck shell=bash

module_rclone_description="rclone"

mistborn_rclone_min_service_version="1.69.0"

mistborn_rclone_version_at_least() {
  local actual="$1" required="$2" actual_major actual_minor actual_patch required_major required_minor required_patch actual_suffix required_suffix pair actual_number required_number
  [[ "$actual" =~ ^v?([0-9]+)\.([0-9]+)\.([0-9]+)([-+].*)?$ ]] || return 2
  actual_major="${BASH_REMATCH[1]}" actual_minor="${BASH_REMATCH[2]}" actual_patch="${BASH_REMATCH[3]}" actual_suffix="${BASH_REMATCH[4]:-}"
  [[ "$required" =~ ^v?([0-9]+)\.([0-9]+)\.([0-9]+)([-+].*)?$ ]] || return 2
  required_major="${BASH_REMATCH[1]}" required_minor="${BASH_REMATCH[2]}" required_patch="${BASH_REMATCH[3]}" required_suffix="${BASH_REMATCH[4]:-}"

  for pair in "${actual_major}:${required_major}" "${actual_minor}:${required_minor}" "${actual_patch}:${required_patch}"; do
    actual_number="${pair%%:*}" required_number="${pair#*:}"
    actual_number=$((10#$actual_number)) required_number=$((10#$required_number))
    if (( actual_number > required_number )); then return 0; fi
    if (( actual_number < required_number )); then return 1; fi
  done
  if [[ "$actual_suffix" == -* && -z "$required_suffix" ]]; then return 1; fi
  return 0
}

mistborn_rclone_read_version() {
  local binary="$1" output
  output="$("$binary" --version 2>&1)" || { ui_error "Cannot inspect rclone at $binary: $output"; return 1; }
  output="${output%%$'\n'*}"
  [[ "$output" =~ ^rclone[[:space:]]+v?([0-9]+\.[0-9]+\.[0-9]+([-+][^[:space:]]*)?)$ ]] || {
    ui_error "Cannot parse rclone version from $binary: $output"; return 1;
  }
  printf '%s\n' "${BASH_REMATCH[1]}"
}

mistborn_rclone_verify_service_capability() {
  local binary="$1" rcd_help global_help
  rcd_help="$("$binary" rcd --help 2>&1)" || { ui_error "Cannot inspect rclone rcd support in $binary."; return 1; }
  for flag in --rc-addr --rc-user --rc-pass; do
    [[ "$rcd_help" == *"$flag"* ]] || { ui_error "Rclone at $binary lacks required service option $flag."; return 1; }
  done
  global_help="$("$binary" --help 2>&1)" || { ui_error "Cannot inspect rclone global options in $binary."; return 1; }
  [[ "$global_help" == *"--config"* ]] || { ui_error "Rclone at $binary lacks the required --config option."; return 1; }
}

mistborn_rclone_find_binary() {
  command -v rclone
}

mistborn_rclone_verify_installed() {
  local binary="$1" require_service="${2:-0}" version selected_path
  version="$(mistborn_rclone_read_version "$binary")" || return 1
  if [[ "$require_service" == 1 ]]; then
    mistborn_rclone_version_at_least "$version" "$mistborn_rclone_min_service_version" || {
      ui_error "rclone $version is too old for the RC socket service; $mistborn_rclone_min_service_version or newer is required."; return 1;
    }
    mistborn_rclone_verify_service_capability "$binary" || return 1
  fi
  selected_path="$(mistborn_rclone_find_binary)" || { ui_error "rclone is not on PATH after capability verification."; return 1; }
  [[ "$selected_path" == "$binary" ]] || {
    ui_error "PATH selects $selected_path instead of the verified rclone binary $binary."; return 1;
  }
}

mistborn_rclone_apt_installed() {
  [[ "$(dpkg-query -W -f='${Status}' rclone 2>/dev/null || true)" == 'install ok installed' ]]
}

mistborn_rclone_removal_is_bounded() {
  local preview="$1" action package found=0
  while read -r action package _; do
    [[ "$action" == Remv ]] || continue
    [[ "$package" == rclone || "$package" == rclone:* ]] || {
      ui_error "Refusing apt removal because it would also remove $package."; return 1;
    }
    found=1
  done <<<"$preview"
  [[ "$found" == 1 ]] || { ui_error "Apt did not preview removal of the installed rclone package."; return 1; }
}

mistborn_rclone_confirm_apt_removal() {
  local reply
  ui_warn "The official rclone installer replaces /usr/bin/rclone; apt ownership must be removed first. Existing rclone configuration is not purged."
  [[ -t 0 ]] || { ui_error "A terminal is required to approve removal of the apt-managed rclone package."; return 1; }
  read -r -p 'Type REMOVE RCLONE to remove only the apt package and install upstream rclone: ' reply
  [[ "$reply" == 'REMOVE RCLONE' ]] || { ui_error "Apt-managed rclone removal was not approved."; return 1; }
}

mistborn_rclone_download_installer() {
  curl --proto '=https' --tlsv1.2 -fsSL --retry 3 --output "$1" https://rclone.org/install.sh
}

mistborn_rclone_restore_apt() {
  [[ "$1" == 1 ]] || return 0
  ui_warn "Upstream rclone installation failed after apt removal; attempting to restore the apt package."
  apt-get install -y --no-install-recommends rclone || ui_error "Could not restore apt-managed rclone; repair the package manually before retrying."
}

mistborn_rclone_install_official() (
  set -Eeuo pipefail
  local work_dir installer removal_preview binary status had_apt=0
  mistborn_apt_install unzip
  command -v unzip >/dev/null 2>&1 || { ui_error "The official rclone installer requires unzip."; return 1; }
  command -v curl >/dev/null 2>&1 || { ui_error "The official rclone installer requires curl."; return 1; }
  work_dir="$(mktemp -d)" || return 1
  trap 'rm -rf -- "$work_dir"' EXIT
  installer="$work_dir/install.sh"
  mistborn_rclone_download_installer "$installer" || { ui_error "Could not download the official rclone installer."; return 1; }
  [[ -s "$installer" ]] || { ui_error "The downloaded rclone installer is empty."; return 1; }
  if mistborn_rclone_apt_installed; then
    had_apt=1
    removal_preview="$(LC_ALL=C apt-get -s remove rclone)" || { ui_error "Cannot preview apt removal of rclone."; return 1; }
    mistborn_rclone_removal_is_bounded "$removal_preview" || return 1
    mistborn_rclone_confirm_apt_removal || return 1
    apt-get remove -y rclone || {
      ui_error "Could not remove apt-managed rclone."
      mistborn_rclone_restore_apt "$had_apt"
      return 1
    }
  fi
  if bash "$installer"; then
    status=0
  else
    status=$?
  fi
  if [[ "$status" != 0 && "$status" != 3 ]]; then
    ui_error "The official rclone installer failed (exit $status)."
    mistborn_rclone_restore_apt "$had_apt"
    return 1
  fi
  hash -r
  binary="$(mistborn_rclone_find_binary)" || {
    ui_error "rclone is not on PATH after the official installer completed."
    mistborn_rclone_restore_apt "$had_apt"
    return 1
  }
  if ! mistborn_rclone_verify_installed "$binary" "${MISTBORN_RCLONE_SERVICE:-0}"; then
    mistborn_rclone_restore_apt "$had_apt"
    return 1
  fi
)

module_rclone_apply() {
  local user home
  user="$(mistborn_target_user)"
  home="$(mistborn_user_home "$user")"
  [[ -n "$home" ]] || { ui_error "Cannot resolve home directory for $user"; return 1; }
  ui_step "$module_rclone_description"
  if mistborn_task_selected package; then
    mistborn_task_start package
    if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
      ui_info "Would install unzip, download the official rclone installer, and inspect apt ownership before any confirmed package removal"
      ui_info "Would verify installed rclone and RC socket support when service mode is selected"
    else
      mistborn_rclone_install_official || return 1
    fi
    mistborn_task_complete package
  fi
  if mistborn_task_selected configuration && [[ "${MISTBORN_RCLONE_SERVICE:-0}" == 1 ]]; then
    mistborn_task_skip configuration
  elif mistborn_task_selected configuration && [[ "${MISTBORN_RCLONE_CONFIGURE:-0}" == 1 ]]; then
    mistborn_task_start configuration
    if [[ "$user" == root ]]; then
      mistborn_run_interactive env HOME="$home" rclone config
    else
      mistborn_run_interactive runuser -u "$user" -- env HOME="$home" rclone config
    fi
    mistborn_task_complete configuration
  elif mistborn_task_selected configuration; then
    mistborn_task_skip configuration
  fi
  ui_success "$module_rclone_description"
}
# shellcheck shell=bash

module_rclone_service_description="rclone authenticated Unix-socket RC service"

mistborn_rclone_no_symlink_ancestors() {
  local path="$1" current=/ component
  local -a parts
  IFS=/ read -r -a parts <<<"${path#/}"
  for component in "${parts[@]}"; do
    [[ -n "$component" ]] || continue
    current="${current%/}/$component"
    [[ ! -L "$current" ]] || { ui_error "Refusing symlink in managed path: $current"; return 1; }
  done
}

mistborn_rclone_check_owned_file() {
  local path="$1" owner="$2" marker="$3" mode
  [[ ! -e "$path" && ! -L "$path" ]] && return 0
  [[ -f "$path" && ! -L "$path" ]] || { ui_error "Refusing non-regular or symlinked managed path: $path"; return 1; }
  [[ "$(stat -c '%u' "$path" 2>/dev/null || stat -f '%u' "$path")" == "$owner" ]] || {
    ui_error "Refusing to overwrite a file not owned by uid $owner: $path"; return 1;
  }
  grep -Fq "$marker" "$path" || { ui_error "Refusing to overwrite an unowned file at $path."; return 1; }
  if [[ "$path" == */etc/rclone/rc.env ]]; then
    mode="$(stat -c '%a' "$path" 2>/dev/null || stat -f '%Lp' "$path")"
    [[ "$mode" == 600 ]] || { ui_error "Refusing credential file with unsafe permissions: $path"; return 1; }
  fi
}

mistborn_rclone_check_service_account() {
  local account uid home shell groups expected_home="${MISTBORN_RCLONE_ROOT:-}/var/lib/mistborn-rclone"
  if ! account="$(getent passwd mistborn-rclone 2>/dev/null)"; then return 0; fi
  IFS=: read -r _ _ uid _ _ home shell <<<"$account"
  if ! [[ "$uid" =~ ^[0-9]+$ ]] || ! ((uid > 0 && uid < 1000)) \
    || ! [[ "$home" == "$expected_home" && "$shell" == */nologin ]]; then
    ui_error "Existing mistborn-rclone account is not a non-root system account with the expected home and nologin shell."; return 1;
  fi
  groups="$(id -nG mistborn-rclone 2>/dev/null)" || { ui_error "Cannot inspect mistborn-rclone groups."; return 1; }
  [[ "$groups" == mistborn-rclone ]] || {
    ui_error "Existing mistborn-rclone account has supplementary groups; only its dedicated primary group is allowed."; return 1;
  }
  mistborn_rclone_no_symlink_ancestors "$home" || return 1
  if [[ -e "$home" ]]; then
    [[ -d "$home" && "$(stat -c '%u' "$home" 2>/dev/null || stat -f '%u' "$home")" == "$uid" ]] || {
      ui_error "Existing mistborn-rclone home is not a directory owned by the service account."; return 1;
    }
  fi
}

mistborn_rclone_service_preflight() {
  local help path
  command -v systemctl >/dev/null 2>&1 || { ui_error "systemd is required for the rclone service."; return 1; }
  command -v runuser >/dev/null 2>&1 || { ui_error "runuser is required to configure rclone as its dedicated account."; return 1; }
  command -v curl >/dev/null 2>&1 || { ui_error "curl is required to verify RC authentication over the Unix socket."; return 1; }
  command -v rclone >/dev/null 2>&1 || { ui_error "rclone is not installed."; return 1; }
  help="$(rclone rcd --help 2>&1)" || { ui_error "Installed rclone does not provide the rcd command."; return 1; }
  [[ "$help" == *"--rc-addr"* && "$help" == *"--rc-user"* && "$help" == *"--rc-pass"* ]] || {
    ui_error "Installed rclone rcd lacks Unix socket and RC authentication flags."; return 1;
  }
  help="$(rclone --help 2>&1)" || { ui_error "Cannot inspect rclone global options."; return 1; }
  [[ "$help" == *"--config"* ]] || { ui_error "Installed rclone lacks --config support."; return 1; }
  mistborn_rclone_check_service_account
  local home="${MISTBORN_RCLONE_ROOT:-}/var/lib/mistborn-rclone"
  local config="$home/.config/rclone/rclone.conf" env_path="${MISTBORN_RCLONE_ROOT:-}/etc/rclone/rc.env"
  local unit_path="${MISTBORN_RCLONE_ROOT:-}/etc/systemd/system/mistborn-rclone.service"
  for path in "$home" "$home/.config" "$home/.config/rclone" "$config" "${MISTBORN_RCLONE_ROOT:-}/etc/rclone" "$env_path" "$unit_path"; do
    mistborn_rclone_no_symlink_ancestors "$path" || return 1
  done
  local env_dir="${MISTBORN_RCLONE_ROOT:-}/etc/rclone"
  if [[ -e "$env_dir" ]]; then
    [[ -d "$env_dir" && "$(stat -c '%u' "$env_dir" 2>/dev/null || stat -f '%u' "$env_dir")" == 0 ]] || {
      ui_error "Refusing non-directory or non-root-owned rclone credential directory: $env_dir"; return 1;
    }
  fi
  local service_uid=0
  if id mistborn-rclone >/dev/null 2>&1; then service_uid="$(id -u mistborn-rclone)"; fi
  if [[ "$service_uid" == 0 && ( -e "$config" || -L "$config" ) ]]; then
    ui_error "Rclone service config exists without its dedicated account; refusing to take ownership."; return 1
  fi
  mistborn_rclone_check_owned_file "$env_path" 0 '# Managed by Mistborn Bootstrap: rclone RC credentials.' || return 1
  mistborn_rclone_check_owned_file "$unit_path" 0 '# Managed by Mistborn Bootstrap: rclone service.' || return 1
  if [[ -e "$config" || -L "$config" ]]; then
    [[ -f "$config" && ! -L "$config" && "$(stat -c '%u' "$config" 2>/dev/null || stat -f '%u' "$config")" == "$service_uid" ]] || {
      ui_error "Refusing unowned or unsafe rclone service config at $config."; return 1;
    }
    [[ "$(stat -c '%a' "$config" 2>/dev/null || stat -f '%Lp' "$config")" == 600 ]] || {
      ui_error "Rclone service config must be mode 0600."; return 1;
    }
  fi
  if [[ "$service_uid" != 0 ]]; then
    for path in "$home/.config" "$home/.config/rclone"; do
      if [[ -e "$path" ]]; then
        [[ -d "$path" && ! -L "$path" && "$(stat -c '%u' "$path" 2>/dev/null || stat -f '%u' "$path")" == "$service_uid" ]] || {
          ui_error "Refusing rclone service directory not owned by mistborn-rclone: $path"; return 1;
        }
      fi
    done
  fi
}

mistborn_rclone_service_credentials() {
  local username password confirmation
  [[ -r /dev/tty && -w /dev/tty ]] || { ui_error "Rclone service credentials must be entered interactively on a terminal."; return 1; }
  IFS= read -r -p "rclone RC username (letters, numbers, dot, underscore, hyphen): " username </dev/tty
  [[ "$username" =~ ^[A-Za-z0-9_.-]{3,64}$ ]] || { ui_error "Username must be 3-64 safe characters."; return 1; }
  IFS= read -r -s -p "rclone RC password (at least 32 letters, numbers, underscore, hyphen, or dot): " password </dev/tty
  printf '\n' >/dev/tty
  IFS= read -r -s -p "Repeat rclone RC password: " confirmation </dev/tty
  printf '\n' >/dev/tty
  mistborn_rclone_credentials_valid "$username" "$password" || { unset password confirmation; ui_error "Password must be 32-128 safe characters."; return 1; }
  [[ "$password" == "$confirmation" ]] || { unset password confirmation; ui_error "Passwords did not match."; return 1; }
  MISTBORN_RCLONE_SERVICE_USER="$username"
  MISTBORN_RCLONE_SERVICE_PASS="$password"
  unset confirmation
}

mistborn_rclone_credentials_valid() {
  [[ "$1" =~ ^[A-Za-z0-9_.-]{3,64}$ && "$2" =~ ^[A-Za-z0-9_.-]{32,128}$ ]]
}

MISTBORN_RCLONE_TX_ACTIVE=0
MISTBORN_RCLONE_TX_CONFIG_BACKUP=''
MISTBORN_RCLONE_TX_ENV_BACKUP=''
MISTBORN_RCLONE_TX_UNIT_BACKUP=''
MISTBORN_RCLONE_TX_CONFIG_EXISTED=0
MISTBORN_RCLONE_TX_ENV_EXISTED=0
MISTBORN_RCLONE_TX_UNIT_EXISTED=0
MISTBORN_RCLONE_TX_STAGE_CONFIG=''
MISTBORN_RCLONE_TX_STAGE_ENV=''
MISTBORN_RCLONE_TX_STAGE_UNIT=''

mistborn_rclone_snapshot_file() {
  local path="$1" key="$2" backup
  local backup_var="MISTBORN_RCLONE_TX_${key}_BACKUP" existed_var="MISTBORN_RCLONE_TX_${key}_EXISTED"
  if [[ -e "$path" ]]; then
    backup="$(mktemp "${path}.mistborn-backup.XXXXXX")" || return 1
    cp -p "$path" "$backup" || return 1
    printf -v "$backup_var" '%s' "$backup"
    printf -v "$existed_var" 1
  else
    printf -v "$backup_var" ''
    printf -v "$existed_var" 0
  fi
}

mistborn_rclone_restore_one() {
  local key="$1" path="$2" backup="" existed=0
  case "$key" in
    CONFIG) backup="$MISTBORN_RCLONE_TX_CONFIG_BACKUP"; existed="$MISTBORN_RCLONE_TX_CONFIG_EXISTED" ;;
    ENV) backup="$MISTBORN_RCLONE_TX_ENV_BACKUP"; existed="$MISTBORN_RCLONE_TX_ENV_EXISTED" ;;
    UNIT) backup="$MISTBORN_RCLONE_TX_UNIT_BACKUP"; existed="$MISTBORN_RCLONE_TX_UNIT_EXISTED" ;;
  esac
  if [[ "$existed" == 1 ]]; then mv -f "$backup" "$path"; else rm -f -- "$path"; fi
}

mistborn_rclone_transaction_cleanup() {
  local original_status=$?
  trap - EXIT INT TERM HUP
  set +e
  if [[ "$MISTBORN_RCLONE_TX_ACTIVE" == 1 ]]; then
    systemctl stop mistborn-rclone.service >/dev/null 2>&1
    mistborn_rclone_restore_one CONFIG "$MISTBORN_RCLONE_TX_CONFIG_PATH"
    mistborn_rclone_restore_one ENV "$MISTBORN_RCLONE_TX_ENV_PATH"
    mistborn_rclone_restore_one UNIT "$MISTBORN_RCLONE_TX_UNIT_PATH"
    systemctl daemon-reload >/dev/null 2>&1
    if [[ "$MISTBORN_RCLONE_TX_WAS_ACTIVE" == 1 ]]; then
      systemctl restart mistborn-rclone.service >/dev/null 2>&1 || systemctl start mistborn-rclone.service >/dev/null 2>&1
      systemctl is-active --quiet mistborn-rclone.service || ui_warn "Could not restore the previously active rclone service; inspect systemctl status mistborn-rclone."
    elif [[ "$MISTBORN_RCLONE_TX_WAS_ENABLED" == 1 ]]; then
      systemctl enable mistborn-rclone.service >/dev/null 2>&1
      systemctl is-active --quiet mistborn-rclone.service && systemctl stop mistborn-rclone.service >/dev/null 2>&1
    else
      systemctl disable --now mistborn-rclone.service >/dev/null 2>&1
    fi
    MISTBORN_RCLONE_TX_ACTIVE=0
  fi
  rm -f -- "$MISTBORN_RCLONE_TX_STAGE_CONFIG" "$MISTBORN_RCLONE_TX_STAGE_ENV" "$MISTBORN_RCLONE_TX_STAGE_UNIT" \
    "$MISTBORN_RCLONE_TX_CONFIG_BACKUP" "$MISTBORN_RCLONE_TX_ENV_BACKUP" "$MISTBORN_RCLONE_TX_UNIT_BACKUP"
  unset MISTBORN_RCLONE_SERVICE_PASS
  return "$original_status"
}

mistborn_rclone_transaction_signal() {
  local status="$1"
  mistborn_rclone_transaction_cleanup
  exit "$status"
}

mistborn_rclone_verify_socket_auth() {
  local socket="$1" status attempt
  for ((attempt=0; attempt<20; attempt++)); do
    if status="$(curl --silent --show-error --output /dev/null --write-out '%{http_code}' \
      --max-time 3 --noproxy '*' --unix-socket "$socket" --request POST http://localhost/rc/noop 2>/dev/null)"; then
      [[ "$status" == 401 ]] && return 0
      ui_error "rclone RC socket returned HTTP $status without credentials; expected HTTP 401."
      return 1
    fi
    sleep 0.1
  done
  ui_error "Could not verify authentication on the rclone RC socket: $socket"
  return 1
}

mistborn_rclone_commit_staged_files() {
  local staged_config="$1" staged_env="$2" staged_unit="$3"
  local config_path="$4" env_path="$5" unit_path="$6" was_active="$7" was_enabled="$8"
  local verify_socket="${9:-}"
  MISTBORN_RCLONE_TX_CONFIG_PATH="$config_path"
  MISTBORN_RCLONE_TX_ENV_PATH="$env_path"
  MISTBORN_RCLONE_TX_UNIT_PATH="$unit_path"
  MISTBORN_RCLONE_TX_WAS_ACTIVE="$was_active"
  MISTBORN_RCLONE_TX_WAS_ENABLED="$was_enabled"
  MISTBORN_RCLONE_TX_STAGE_CONFIG="$staged_config"
  MISTBORN_RCLONE_TX_STAGE_ENV="$staged_env"
  MISTBORN_RCLONE_TX_STAGE_UNIT="$staged_unit"
  trap 'mistborn_rclone_transaction_cleanup' EXIT
  trap 'mistborn_rclone_transaction_signal 130' INT
  trap 'mistborn_rclone_transaction_signal 143' TERM
  trap 'mistborn_rclone_transaction_signal 129' HUP
  mistborn_rclone_snapshot_file "$config_path" CONFIG || return 1
  mistborn_rclone_snapshot_file "$env_path" ENV || return 1
  mistborn_rclone_snapshot_file "$unit_path" UNIT || return 1
  MISTBORN_RCLONE_TX_ACTIVE=1
  mv -f "$staged_config" "$config_path" || { mistborn_rclone_transaction_cleanup; return 1; }
  mv -f "$staged_env" "$env_path" || { mistborn_rclone_transaction_cleanup; return 1; }
  mv -f "$staged_unit" "$unit_path" || { mistborn_rclone_transaction_cleanup; return 1; }
  systemctl daemon-reload || { mistborn_rclone_transaction_cleanup; return 1; }
  if [[ "$was_active" == 1 ]]; then
    systemctl restart mistborn-rclone.service || { mistborn_rclone_transaction_cleanup; return 1; }
  else
    systemctl enable --now mistborn-rclone.service || { mistborn_rclone_transaction_cleanup; return 1; }
  fi
  systemctl is-active --quiet mistborn-rclone.service || { mistborn_rclone_transaction_cleanup; return 1; }
  if [[ -n "$verify_socket" ]]; then
    local attempt socket_ready=0
    for ((attempt=0; attempt<50; attempt++)); do
      if [[ -S "$verify_socket" ]]; then socket_ready=1; break; fi
      sleep 0.1
    done
    [[ "$socket_ready" == 1 ]] || { ui_error "rclone service is active but its Unix socket did not appear within 5 seconds: $verify_socket"; mistborn_rclone_transaction_cleanup; return 1; }
    mistborn_rclone_verify_socket_auth "$verify_socket" || { mistborn_rclone_transaction_cleanup; return 1; }
  fi
  MISTBORN_RCLONE_TX_ACTIVE=0
  rm -f -- "$MISTBORN_RCLONE_TX_CONFIG_BACKUP" "$MISTBORN_RCLONE_TX_ENV_BACKUP" "$MISTBORN_RCLONE_TX_UNIT_BACKUP"
  MISTBORN_RCLONE_TX_CONFIG_BACKUP=''; MISTBORN_RCLONE_TX_ENV_BACKUP=''; MISTBORN_RCLONE_TX_UNIT_BACKUP=''
  MISTBORN_RCLONE_TX_STAGE_CONFIG=''; MISTBORN_RCLONE_TX_STAGE_ENV=''; MISTBORN_RCLONE_TX_STAGE_UNIT=''
  trap - EXIT INT TERM HUP
}

module_rclone_service_apply() {
  local root="${MISTBORN_RCLONE_ROOT:-}"
  local home="$root/var/lib/mistborn-rclone" config_dir="$root/var/lib/mistborn-rclone/.config/rclone"
  local config_file="$config_dir/rclone.conf" env_file="$root/etc/rclone/rc.env" unit="$root/etc/systemd/system/mistborn-rclone.service"
  [[ "${MISTBORN_RCLONE_SERVICE:-0}" == 1 ]] || { mistborn_task_skip service; return 0; }
  ui_step "$module_rclone_service_description"
  mistborn_require_root
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would verify rclone rcd Unix-socket and authentication support"
    ui_info "Would create the unprivileged mistborn-rclone account and configure its private remotes"
    ui_info "Would write root-only RC credentials and install a service using /run/rclone/rc.sock"
    ui_info "No TCP listener or firewall rule would be created"
    mistborn_task_complete service
    return 0
  fi
  mistborn_rclone_service_preflight
  ui_warn "This starts the authenticated rclone RC API on a Unix socket. RC access is equivalent to shell access as the service user."
  ui_info "Rclone will run as mistborn-rclone and use $config_file. No mount will be created in a container."
  ui_confirm "Continue with the rclone Unix-socket service setup?" || { ui_error "Rclone service setup requires explicit confirmation."; return 1; }
  local rclone_binary
  rclone_binary="$(command -v rclone)"
  mistborn_rclone_verify_installed "$rclone_binary" 1 || return 1
  mistborn_rclone_service_credentials
  mistborn_task_start service
  trap 'mistborn_rclone_transaction_cleanup' EXIT
  trap 'mistborn_rclone_transaction_signal 130' INT
  trap 'mistborn_rclone_transaction_signal 143' TERM
  trap 'mistborn_rclone_transaction_signal 129' HUP
  local was_active=0
  local was_enabled=0
  if systemctl is-active --quiet mistborn-rclone.service; then was_active=1; fi
  if systemctl is-enabled --quiet mistborn-rclone.service; then was_enabled=1; fi
  if ! id mistborn-rclone >/dev/null 2>&1; then
    useradd --system --user-group --home-dir "$home" --create-home --shell /usr/sbin/nologin mistborn-rclone
  else
    local account
    account="$(getent passwd mistborn-rclone)" || { ui_error "Cannot inspect existing mistborn-rclone account."; return 1; }
    [[ "$(cut -d: -f3 <<<"$account")" -gt 0 && "$(cut -d: -f6 <<<"$account")" == "$home" ]] || {
      ui_error "Existing mistborn-rclone account does not match the expected unprivileged home."; return 1;
    }
  fi
  mistborn_rclone_check_service_account
  chmod 0700 "$home"
  install -d -o mistborn-rclone -g mistborn-rclone -m 0700 "$config_dir"
  local staged_config staged_env staged_unit
  staged_config="$(mktemp "$config_dir/.rclone.conf.mistborn.XXXXXX")"
  MISTBORN_RCLONE_TX_STAGE_CONFIG="$staged_config"
  chown mistborn-rclone:mistborn-rclone "$staged_config"
  chmod 0600 "$staged_config"
  if [[ -f "$config_file" ]]; then cp -p "$config_file" "$staged_config"; chown mistborn-rclone:mistborn-rclone "$staged_config"; chmod 0600 "$staged_config"; fi
  ui_info "Configure the remotes that this service account will expose to the Runtipi app."
  mistborn_run_interactive runuser -u mistborn-rclone -- env HOME="$home" rclone --config "$staged_config" config
  [[ -f "$staged_config" && ! -L "$staged_config" && "$(stat -c '%u' "$staged_config" 2>/dev/null || stat -f '%u' "$staged_config")" == "$(id -u mistborn-rclone)" ]] || {
    ui_error "rclone config did not leave a regular service-account-owned staged file."; return 1;
  }
  chmod 0600 "$staged_config"

  install -d -m 0755 "$root/etc/systemd/system"
  if [[ ! -d "$root/etc/rclone" ]]; then install -d -o root -g root -m 0700 "$root/etc/rclone"; fi
  staged_env="$(mktemp "$root/etc/rclone/.rc.env.XXXXXX")"
  staged_unit="$(mktemp "$root/etc/systemd/system/.mistborn-rclone.service.XXXXXX")"
  MISTBORN_RCLONE_TX_STAGE_ENV="$staged_env"
  MISTBORN_RCLONE_TX_STAGE_UNIT="$staged_unit"
  chmod 0600 "$staged_env"
  printf '# Managed by Mistborn Bootstrap: rclone RC credentials.\nRCLONE_RC_USER=%s\nRCLONE_RC_PASS=%s\n' "$MISTBORN_RCLONE_SERVICE_USER" "$MISTBORN_RCLONE_SERVICE_PASS" >"$staged_env"
  chown root:root "$staged_env"
  local rclone_bin
  rclone_bin="$(command -v rclone)"
  [[ "$rclone_bin" == /* && "$rclone_bin" != *[[:space:]]* ]] || { ui_error "rclone executable path cannot be represented safely in the systemd unit."; return 1; }
  cat >"$staged_unit" <<UNIT
# Managed by Mistborn Bootstrap: rclone service.
[Unit]
Description=Mistborn host rclone RC API (Unix socket)
Before=docker.service
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=mistborn-rclone
Group=mistborn-rclone
EnvironmentFile=$env_file
RuntimeDirectory=rclone
RuntimeDirectoryMode=0750
RuntimeDirectoryPreserve=restart
UMask=0007
ExecStart=$rclone_bin --config=$config_file rcd --rc-addr=/run/rclone/rc.sock
Restart=on-failure
RestartSec=5
NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ProtectKernelTunables=true
ProtectControlGroups=true
ReadWritePaths=$home
ReadWritePaths=/run/rclone

[Install]
WantedBy=multi-user.target
UNIT
  chown root:root "$staged_unit"
  chmod 0644 "$staged_unit"

  # Leave an existing daemon untouched until the interactive config and all replacement files are ready.
  mistborn_rclone_commit_staged_files "$staged_config" "$staged_env" "$staged_unit" "$config_file" "$env_file" "$unit" "$was_active" "$was_enabled" /run/rclone/rc.sock || {
    ui_error "rclone service update failed; previous files and service state were restored where possible."; return 1;
  }
  unset MISTBORN_RCLONE_SERVICE_PASS
  ui_success "Authenticated rclone RC socket: /run/rclone/rc.sock"
  ui_info "The socket is available to group members via mode 0750 directory and umask 0007; no TCP listeners were configured."
  mistborn_task_complete service
}
# shellcheck shell=bash

module_runtipi_description="Runtipi"

module_runtipi_apply() {
  ui_step "$module_runtipi_description"
  mistborn_task_selected install || return 0
  mistborn_task_start install
  if command -v runtipi-cli >/dev/null 2>&1 || [[ -x /opt/runtipi/runtipi-cli ]]; then
    ui_info "Runtipi already installed"
  elif [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would run the official Runtipi installer"
  else
    curl -fsSL https://setup.runtipi.io | bash
  fi
  mistborn_task_complete install
  ui_success "$module_runtipi_description"
}
# shellcheck shell=bash

module_security_description="SSH, UFW, and fail2ban hardening"

mistborn_valid_ipv4_cidr() {
  local cidr="$1" address prefix octet
  local -a octets
  [[ "$cidr" == */* ]] || return 1
  address="${cidr%/*}"
  prefix="${cidr##*/}"
  [[ "$prefix" =~ ^[0-9]+$ ]] && ((10#$prefix <= 32)) || return 1
  IFS=. read -r -a octets <<<"$address"
  [[ "${#octets[@]}" -eq 4 ]] || return 1
  for octet in "${octets[@]}"; do
    [[ "$octet" =~ ^[0-9]+$ ]] && ((10#$octet <= 255)) || return 1
  done
}

mistborn_configure_plex_ufw() {
  local profile=/etc/ufw/applications.d/plexmediaserver temporary lan_cidr
  lan_cidr="${MISTBORN_PLEX_LAN_CIDR:-}"
  if [[ -n "$lan_cidr" ]] && ! mistborn_valid_ipv4_cidr "$lan_cidr"; then
    ui_error "MISTBORN_PLEX_LAN_CIDR must be an IPv4 CIDR such as 192.168.1.0/24"
    return 1
  fi

  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would install the Plex UFW application profiles at $profile"
  else
    temporary="$(mktemp)"
    printf '%s\n' \
      '[plexmediaserver]' \
      'title=Plex Media Server (Standard)' \
      'description=The Plex Media Server' \
      'ports=32400/tcp|3005/tcp|5353/udp|8324/tcp|32410:32414/udp' \
      '' \
      '[plexmediaserver-dlna]' \
      'title=Plex Media Server (DLNA)' \
      'description=The Plex Media Server (additional DLNA capability only)' \
      'ports=1900/udp|32469/tcp' \
      '' \
      '[plexmediaserver-all]' \
      'title=Plex Media Server (Standard + DLNA)' \
      'description=The Plex Media Server (with additional DLNA capability)' \
      'ports=32400/tcp|3005/tcp|5353/udp|8324/tcp|32410:32414/udp|1900/udp|32469/tcp' \
      >"$temporary"
    if install -m 0644 "$temporary" "$profile"; then
      rm -f -- "$temporary"
    else
      rm -f -- "$temporary"
      return 1
    fi
  fi

  mistborn_run ufw app update plexmediaserver
  mistborn_run ufw allow 32400/tcp comment 'Plex remote access'
  if [[ -n "$lan_cidr" ]]; then
    mistborn_run ufw allow from "$lan_cidr" to any app plexmediaserver-all
  fi
  if [[ "${MISTBORN_PLEX_TAILSCALE:-0}" == 1 ]]; then
    mistborn_run ufw allow in on tailscale0 to any app plexmediaserver-all
  fi
}

module_security_apply() {
  local ssh_port="${MISTBORN_SSH_PORT:-22}" ssh_config=/etc/ssh/sshd_config backup
  ui_step "$module_security_description"
  if [[ "${MISTBORN_HARDEN:-0}" != 1 ]]; then
    ui_warn "Security hardening is opt-in; re-run with MISTBORN_HARDEN=1 after testing SSH keys"
    for task in packages ssh firewall plex-firewall fail2ban tailscale-only; do
      mistborn_task_selected "$task" && mistborn_task_skip "$task"
    done
    return 0
  fi
  if mistborn_task_selected ssh && [[ "${MISTBORN_DISABLE_PASSWORD_AUTH:-1}" == 1 && "${MISTBORN_DRY_RUN:-0}" != 1 ]]; then
    local user home
    user="$(mistborn_target_user)"; home="$(mistborn_user_home "$user")"
    if [[ ! -s "$home/.ssh/authorized_keys" && ! -s /root/.ssh/authorized_keys && "${MISTBORN_FORCE_SSH:-0}" != 1 ]]; then
      ui_error "No authorized_keys found; refusing to disable password authentication"
      return 1
    fi
  fi
  if mistborn_task_selected packages; then
    mistborn_task_start packages; mistborn_apt_install ufw fail2ban; mistborn_task_complete packages
  fi
  if mistborn_task_selected ssh; then
  mistborn_task_start ssh
  if [[ "${MISTBORN_DRY_RUN:-0}" == 1 ]]; then
    ui_info "Would harden $ssh_config and validate it before restart"
  else
    backup="${ssh_config}.bak-mistborn"
    [[ -e "$backup" ]] || cp -a "$ssh_config" "$backup"
    sed -i -E 's/^[#[:space:]]*PasswordAuthentication.*/PasswordAuthentication no/' "$ssh_config"
    sed -i -E 's/^[#[:space:]]*PermitRootLogin.*/PermitRootLogin no/' "$ssh_config"
    if grep -Eq '^[#[:space:]]*Port[[:space:]]+' "$ssh_config"; then
      sed -i -E "s/^[#[:space:]]*Port[[:space:]]+.*/Port $ssh_port/" "$ssh_config"
    else
      printf '\nPort %s\n' "$ssh_port" >>"$ssh_config"
    fi
    if ! sshd -t; then cp -a "$backup" "$ssh_config"; ui_error "Invalid sshd configuration; restored backup"; return 1; fi
    systemctl restart sshd
    install -m 0644 /dev/null /etc/fail2ban/jail.local
    printf '[sshd]\nenabled = true\nport = ssh\nmaxretry = %s\nbantime = %s\n' \
      "${MISTBORN_FAIL2BAN_MAXRETRY:-3}" "${MISTBORN_FAIL2BAN_BANTIME:-3600}" >/etc/fail2ban/jail.local
  fi
  mistborn_task_complete ssh
  fi
  if mistborn_task_selected firewall; then
  mistborn_task_start firewall
  mistborn_run ufw allow "$ssh_port/tcp"
  for port in ${MISTBORN_ALLOWED_TCP_PORTS:-}; do mistborn_run ufw allow "$port/tcp"; done
  mistborn_run ufw default deny incoming
  mistborn_run ufw --force enable
  mistborn_task_complete firewall
  fi
  if mistborn_task_selected plex-firewall && [[ "${MISTBORN_PLEX_UFW:-0}" == 1 ]]; then
    command -v ufw >/dev/null 2>&1 || mistborn_apt_install ufw
    mistborn_task_start plex-firewall
    mistborn_configure_plex_ufw
    mistborn_task_complete plex-firewall
  elif mistborn_task_selected plex-firewall; then
    mistborn_task_skip plex-firewall
  fi
  if mistborn_task_selected fail2ban; then
    mistborn_task_start fail2ban; mistborn_run systemctl enable --now fail2ban; mistborn_task_complete fail2ban
  fi
  if mistborn_task_selected tailscale-only && [[ "${MISTBORN_TAILSCALE_ONLY:-0}" == 1 ]]; then
    mistborn_task_start tailscale-only
    mistborn_run tailscale set --ssh=true
    mistborn_run ufw allow in on tailscale0
    mistborn_run ufw allow "${MISTBORN_TAILSCALE_PORT:-41641}/udp"
    mistborn_run ufw delete allow "$ssh_port/tcp" || true
    mistborn_task_complete tailscale-only
    ui_warn "Confirm a new Tailscale SSH session before disconnecting"
  elif mistborn_task_selected tailscale-only; then
    mistborn_task_skip tailscale-only
  fi
  ui_success "$module_security_description"
}
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

  if [[ "${MISTBORN_HARDEN+x}" == x && "${MISTBORN_HARDEN:-0}" == 1 ]] \
    && mistborn_config_task_applied security/ssh \
    && mistborn_config_task_applied security/fail2ban; then
    local maxretry="${MISTBORN_FAIL2BAN_MAXRETRY:-3}" bantime="${MISTBORN_FAIL2BAN_BANTIME:-3600}"
    [[ "$maxretry" =~ ^[0-9]+$ ]] && ((10#$maxretry > 0)) || return 1
    [[ "$bantime" =~ ^[0-9]+$ ]] && ((10#$bantime > 0)) || return 1
    printf '\n[fail2ban]\nenabled = true\n\n[fail2ban.sshd]\nenabled = true\nmaxretry = %s\nbantime = %s\n' \
      "$maxretry" "$bantime" >>"$output"
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
    [[ -x /usr/local/bin/mistborn-bootstrap ]] || {
      ui_error "Cannot install mistborn: Rust runner is missing"
      return 1
    }
    mistborn_publish_desired_config /etc/mistborn /usr/local/share/mistborn "${MISTBORN_RUNNER_BINARY:-/usr/local/bin/mistborn-bootstrap}"
    install -m 0755 /usr/local/bin/mistborn-bootstrap /usr/local/bin/mistborn
    mistborn_task_complete command
    fi
  fi
  ui_success "$module_toolset_description"
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
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Mistborn server setup"
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != common ]] || module_common_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != docker ]] || module_docker_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != zsh ]] || module_zsh_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != tailscale ]] || module_tailscale_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != rclone ]] || module_rclone_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != rclone_service ]] || module_rclone_service_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != runtipi ]] || module_runtipi_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != security ]] || module_security_apply
  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != toolset ]] || module_toolset_apply
  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Setup complete"
}
main "$@"
