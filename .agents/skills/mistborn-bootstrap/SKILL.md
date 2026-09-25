---
name: mistborn-bootstrap
description: Install, configure, migrate, diagnose, or operate Mistborn Bootstrap and its Runtipi-oriented server and shell collections.
---

# Mistborn Bootstrap

Use the repository's revisioned plans and generated installers instead of
assembling host commands independently.

## Choose the operation

- For a new host, use the latest release `install.sh` with the `server` or
  `shell` collection. Prefer a pinned release when reproducibility matters.
- Before changing an existing host, run `mistborn-bootstrap plan COLLECTION`.
- Apply one migration with `mistborn-bootstrap apply COLLECTION STAGE/TASK`.
  Use a stage target only when every task in that stage should rerun.
- Use `mistborn doctor`, `mistborn status`, and `mistborn security-status` for
  diagnostics. Do not treat `doctor` as proof that configuration has not
  drifted unless its checks explicitly cover the relevant setting.
- Use `mistborn upgrade` for the installed host tool and
  `mistborn update-runtipi` for Runtipi core, stores, and apps.

## Installation

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh \
  | sudo bash -s -- server
```

Inspect security-sensitive options before enabling them. SSH and firewall
changes require an explicit target when introduced or changed on an existing
installation. Keep a second verified SSH session open while changing access.

The runner stores progress under `/var/lib/mistborn-bootstrap` and logs under
`/var/log/mistborn-bootstrap`. Version-1 state is preserved as a `.v1.bak`
file before the first version-2 write. Do not delete or edit state merely to
force a rerun; target the intended stage or task.

## Plex firewall migrations

Apply only the Plex task on an existing server:

```bash
curl -fsSL https://github.com/EckPhi/mistborn-bootstrap/releases/latest/download/install.sh \
  | sudo env MISTBORN_HARDEN=1 MISTBORN_PLEX_UFW=1 \
      bash -s -- server security/plex-firewall --yes
```

Optional scopes:

- `MISTBORN_PLEX_LAN_CIDR=192.168.1.0/24` permits Plex discovery, Companion,
  and DLNA services only from that IPv4 LAN.
- `MISTBORN_PLEX_TAILSCALE=1` permits those services only through
  `tailscale0`. Tailnet grants or ACLs remain the authorization boundary.

TCP 32400 is the only Plex port intentionally opened publicly. Do not replace
the scoped rules with the complete Plex profile on all interfaces.

## Repository changes

Edit source modules and plans, then run `./tools/build.sh`; generated `dist/`
scripts are committed. A configuration-shape change needs matching website,
plan-input, documentation, and regression-test updates. Before a release run
the Rust tests, site tests, generated-script tests, ShellCheck, and
`git diff --check`. Release tags are `vMAJOR.MINOR.PATCH`; CI builds and
publishes the binaries and installer.
