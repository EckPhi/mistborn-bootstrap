# Mistborn Bootstrap

Composable Bash provisioning for Runtipi servers, with a consistent terminal
UI and reproducible, single-file installers.

The server collection installs Docker, Zsh/Oh My Zsh/Powerlevel10k,
Tailscale, Runtipi, rclone, and the `mistborn` host-management command.
Security hardening is included but deliberately opt-in.

## Install

Review the script, then run the pinned release:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.1.0/dist/server.sh | sudo bash
```

Use `--dry-run`, `--yes`, or `--user NAME` after `bash -s --`:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.1.0/dist/server.sh | sudo bash -s -- --dry-run --user phil
```

## Development

Collections are ordered lists in `collections/*.modules`. Each name resolves
to `modules/<name>.sh`; shared UI and system helpers live in `lib/`.

```bash
./tools/build.sh
sudo ./dist/server.sh --dry-run --yes --user "$USER"
```

Generated files in `dist/` are committed so release URLs remain simple and
auditable. CI rebuilds them and fails when the checked-in output has drifted.

## Host management

The server collection installs `/usr/local/bin/mistborn` with the operational
features formerly owned by the standalone Runtipi Companion CLI:

```bash
sudo mistborn doctor
sudo mistborn security-status
sudo mistborn tailscale-status
sudo mistborn rclone-config
sudo mistborn update-apps
sudo mistborn update-core latest
sudo mistborn update-appstores
```

App and core updates create native Runtipi app snapshots first. Pass
`--no-backup` immediately after the update command to opt out.

Enable hardening only after confirming key-based SSH access:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/main/dist/server.sh \
  | sudo env MISTBORN_HARDEN=1 MISTBORN_SSH_PORT=22 bash
```
