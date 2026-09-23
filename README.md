# Mistborn Bootstrap

Composable Bash provisioning for Runtipi servers, with a consistent terminal
UI and reproducible, single-file installers.

The server collection installs Docker, Zsh/Oh My Zsh/Powerlevel10k,
Tailscale, Runtipi, rclone, and the `mistborn` host-management command.
Security hardening is included but deliberately opt-in.

## Install

Review the script, then run the pinned release:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.1/install.sh | sudo bash -s -- server
```

The launcher downloads the CI-built binary for x86-64 or ARM64, verifies its
release checksum, and reconnects the dashboard to the controlling terminal.
For unattended Tailscale
enrollment, provide a [pre-authentication key](https://tailscale.com/kb/1085/auth-keys):

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.1/install.sh \
  | sudo env TAILSCALE_AUTH_KEY='tskey-auth-...' bash -s -- server
```

Pass the collection first, followed by `--dry-run`, `--yes`, or `--user NAME`:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.1/install.sh | sudo bash -s -- server --dry-run --user phil
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
Release tags compile static Linux binaries for x86-64 and ARM64 in GitHub
Actions, publish SHA-256 checksums, and create the GitHub release. Rust is not
required on the target server.

## Resumable runner

The Rust runner displays collection and module progress with elapsed times,
writes structured JSON Lines lifecycle events, and skips completed modules
when the same collection is resumed. Its output remains plain text when
redirected, while child processes retain direct terminal access:

```bash
cargo run -- run server --dry-run --yes --user "$USER"
```

State defaults to `/var/lib/mistborn-bootstrap/<collection>.json`; run logs
default to `/var/log/mistborn-bootstrap/<run-id>.jsonl`. During development,
use `--state-dir` and `--log-dir` to select writable temporary directories.

## Host management

The server collection installs `/usr/local/bin/mistborn` with the operational
features formerly owned by the standalone Runtipi Companion CLI:

```bash
sudo mistborn doctor
sudo mistborn status
sudo mistborn fix
sudo mistborn update
sudo mistborn security-status
sudo mistborn tailscale-status
sudo mistborn rclone-config
sudo mistborn update-apps
sudo mistborn update-core latest
sudo mistborn update-appstores
```

App and core updates create native Runtipi app snapshots first. Pass
`--no-backup` immediately after the update command to opt out.

`mistborn status` summarizes installed tools, services, and Tailscale
connectivity. `sudo mistborn fix` enables and starts installed Docker and
Tailscale services; it leaves firewall and SSH configuration untouched.
`sudo mistborn update` updates the Runtipi core, app stores, and apps, creating
app snapshots before core and app updates.

Enable hardening only after confirming key-based SSH access:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.1/install.sh \
  | sudo env MISTBORN_HARDEN=1 MISTBORN_SSH_PORT=22 bash -s -- server
```
