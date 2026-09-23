# Mistborn Bootstrap

Composable Bash provisioning for Runtipi servers, with a consistent terminal
UI and reproducible, single-file installers.

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
