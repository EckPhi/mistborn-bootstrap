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
