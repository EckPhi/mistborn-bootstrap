# Mistborn Bootstrap

Composable Bash provisioning for Runtipi servers, with a consistent terminal
UI and reproducible, single-file installers.

The server collection installs Docker, Zsh/Oh My Zsh/Powerlevel10k,
Tailscale, Runtipi, rclone, and the `mistborn` host-management command.
Security hardening is included but deliberately opt-in.

## Install

Run the installer from the latest published release:

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh | sudo bash -s -- server
```

To pin a specific payload version, fetch that tag's installer and set
`MISTBORN_VERSION`:

```bash
curl -fsSL https://raw.githubusercontent.com/EckPhi/mistborn-bootstrap/v0.5.6/install.sh \
  | sudo env MISTBORN_VERSION=v0.5.6 bash -s -- server
```

The launcher downloads the CI-built binary for x86-64 or ARM64, verifies its
release checksum, and reconnects the dashboard to the controlling terminal.
For unattended Tailscale
enrollment, provide a [pre-authentication key](https://tailscale.com/kb/1085/auth-keys):

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh \
  | sudo env TAILSCALE_AUTH_KEY='tskey-auth-...' bash -s -- server
```

Pass the collection first, followed by `--dry-run`, `--yes`, or `--user NAME`:

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh | sudo bash -s -- server --dry-run --user phil
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
Release tags set the Cargo version from the tag, compile static Linux binaries
for x86-64 and ARM64 in GitHub Actions, and publish a version-matched installer
with SHA-256 checksums. The Pages release picker is generated from published
GitHub releases. Version files need no manual bump commit, and Rust is not
required on the target server.

## Revisioned and resumable runner

The Rust runner displays collection and module progress with elapsed times,
writes structured JSON Lines lifecycle events, and skips tasks whose revision
and declared configuration inputs still match. Its output remains plain text when
redirected, while child processes retain direct terminal access:

```bash
cargo run -- run server --dry-run --yes --user "$USER"
cargo run -- plan server
cargo run -- apply server security/plex-firewall --dry-run --yes
```

State defaults to `/var/lib/mistborn-bootstrap/<collection>.json`; run logs
default to `/var/log/mistborn-bootstrap/<run-id>.jsonl`. During development,
use `--state-dir` and `--log-dir` to select writable temporary directories.

Stage order and dashboard text are defined in `plans/*.toml`; each plan lists
weighted task IDs and their Bash action names. Bash modules still own the
system changes and emit task lifecycle events, so the runner can show and log
substage progress without parsing command output. `plans/update.toml` describes
the local `mistborn upgrade` flow. The `.modules` files remain the bundle's
source-file list, and the runner checks that their order matches the TOML plan.

Each task may declare a `revision`, the environment-variable names that affect
its desired state, and whether a changed or newly added task requires an
explicit apply on an existing installation. State files from the original
module-level model are migrated automatically to version 2. Before the first
v2 write, the runner preserves the original as `<collection>.json.v1.bak`.
`plan` is read-only and reports `current`, `pending`, `changed`, or `stale`.
`run` remains a compatibility alias for `apply`; a `STAGE` target reruns that
stage's tasks, while `STAGE/TASK` changes only one task.

The installer uses the setup dashboard with its module checklist and weighted
progress. Each `mistborn` host command uses a separate command dashboard with
operation-specific help, live terminal output, and interactive key forwarding.
Both dashboards remain visible at completion until a key is pressed.

## Host management

The server collection installs `/usr/local/bin/mistborn` with the operational
features formerly owned by the standalone Runtipi Companion CLI:

```bash
sudo mistborn doctor
sudo mistborn status
sudo mistborn fix
sudo mistborn upgrade
sudo mistborn update-runtipi
sudo mistborn security-status
sudo mistborn tailscale-status
sudo mistborn rclone-config
sudo mistborn update-apps
sudo mistborn update-core latest
sudo mistborn update-appstores
```

Run `mistborn` or `mistborn help` without a subcommand to open the interactive
command picker. Use the arrow keys (or `j`/`k`) to select an operation, Enter
to run it, and `q` or Esc to return to the shell. On a completed command or
server setup screen, press `h` or Home to return to the command picker.

App and core updates create native Runtipi app snapshots first. Pass
`--no-backup` immediately after the update command to opt out.

`mistborn status` reports installed component versions, service health,
bootstrap state, and effective SSH, UFW, fail2ban, Plex, and Tailscale
configuration. It verifies required managed firewall rules but permits
unrelated administrator rules. `sudo mistborn fix` enables and starts installed Docker and
Tailscale services; it leaves firewall and SSH configuration untouched.
`sudo mistborn upgrade` refreshes the installed Mistborn runner and command to
the latest stable bootstrap release. Existing completed setup stages remain
skipped while the host-tool stage is refreshed. `sudo mistborn update-runtipi`
updates Runtipi core, app stores, and apps with snapshots.

`sudo mistborn update` is retained as an alias for `upgrade`. Hosts installed
with v0.5.1 or earlier need a one-time installer rerun before the new command
is available: `curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh | sudo bash -s -- server --yes`.

Enable hardening only after confirming key-based SSH access:

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh \
  | sudo env MISTBORN_HARDEN=1 MISTBORN_SSH_PORT=22 bash -s -- server
```

Plex firewall support is opt-in with hardening. It exposes only Plex's required
remote-access port (TCP 32400) publicly. Supplying a LAN CIDR additionally
allows the discovery, Companion, and DLNA ports from that IPv4 network only:

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh \
  | sudo env MISTBORN_HARDEN=1 MISTBORN_PLEX_UFW=1 \
      MISTBORN_PLEX_LAN_CIDR=192.168.1.0/24 bash -s -- server
```

Set `MISTBORN_PLEX_TAILSCALE=1` to allow those same local-service ports only
through the `tailscale0` interface. Tailnet policy still controls which peers
may reach the server:

```bash
sudo env MISTBORN_HARDEN=1 MISTBORN_PLEX_UFW=1 \
  MISTBORN_PLEX_TAILSCALE=1 mistborn-bootstrap apply server security/plex-firewall
```

The application profile follows the referenced
[Plex UFW profile](https://gist.github.com/nmaggioni/45dcca7695d37e6109276b1a6ad8c9c9),
while rule scope follows
[Plex's firewall guidance](https://support.plex.tv/articles/201543147-what-network-ports-do-i-need-to-allow-through-my-firewall/)
not to expose the optional local-network ports on a public interface.

To add this to a server that completed an older bootstrap, explicitly target
only the new task. The prior SSH, base-firewall, and fail2ban tasks are not
rerun:

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh \
  | sudo env MISTBORN_HARDEN=1 MISTBORN_PLEX_UFW=1 \
      MISTBORN_PLEX_LAN_CIDR=192.168.1.0/24 bash -s -- \
      server security/plex-firewall --yes
```

The official Runtipi Plex app currently uses host networking, so these UFW
rules apply to it. A custom bridge-mode Plex deployment with Docker-published
ports needs Docker-aware filtering or explicit host-IP bindings because Docker
published ports can bypass UFW.
