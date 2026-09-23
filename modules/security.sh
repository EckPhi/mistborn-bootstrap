# shellcheck shell=bash

module_security_description="SSH, UFW, and fail2ban hardening"

module_security_apply() {
  local ssh_port="${MISTBORN_SSH_PORT:-22}" ssh_config=/etc/ssh/sshd_config backup
  ui_step "$module_security_description"
  if [[ "${MISTBORN_HARDEN:-0}" != 1 ]]; then
    ui_warn "Security hardening is opt-in; re-run with MISTBORN_HARDEN=1 after testing SSH keys"
    for task in packages ssh firewall fail2ban; do mistborn_task_skip "$task"; done
    mistborn_task_skip tailscale-only
    return 0
  fi
  if [[ "${MISTBORN_DISABLE_PASSWORD_AUTH:-1}" == 1 && "${MISTBORN_DRY_RUN:-0}" != 1 ]]; then
    local user home
    user="$(mistborn_target_user)"; home="$(mistborn_user_home "$user")"
    if [[ ! -s "$home/.ssh/authorized_keys" && ! -s /root/.ssh/authorized_keys && "${MISTBORN_FORCE_SSH:-0}" != 1 ]]; then
      ui_error "No authorized_keys found; refusing to disable password authentication"
      return 1
    fi
  fi
  mistborn_task_start packages
  mistborn_apt_install ufw fail2ban
  mistborn_task_complete packages
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
  mistborn_task_start firewall
  mistborn_run ufw allow "$ssh_port/tcp"
  for port in ${MISTBORN_ALLOWED_TCP_PORTS:-}; do mistborn_run ufw allow "$port/tcp"; done
  mistborn_run ufw default deny incoming
  mistborn_run ufw --force enable
  mistborn_task_complete firewall
  mistborn_task_start fail2ban
  mistborn_run systemctl enable --now fail2ban
  mistborn_task_complete fail2ban
  if [[ "${MISTBORN_TAILSCALE_ONLY:-0}" == 1 ]]; then
    mistborn_task_start tailscale-only
    mistborn_run tailscale set --ssh=true
    mistborn_run ufw allow in on tailscale0
    mistborn_run ufw allow "${MISTBORN_TAILSCALE_PORT:-41641}/udp"
    mistborn_run ufw delete allow "$ssh_port/tcp" || true
    mistborn_task_complete tailscale-only
    ui_warn "Confirm a new Tailscale SSH session before disconnecting"
  else
    mistborn_task_skip tailscale-only
  fi
  ui_success "$module_security_description"
}
