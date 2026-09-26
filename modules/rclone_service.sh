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
  help="$(rclone help flags 2>&1)" || { ui_error "Cannot inspect rclone global options."; return 1; }
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
