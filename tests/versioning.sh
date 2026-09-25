#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT
cp "$root/Cargo.toml" "$root/Cargo.lock" "$root/tools/set-version.py" "$temporary/"

(
  cd "$temporary"
  python3 set-version.py 9.8.7
)
grep -Fq 'version = "9.8.7"' "$temporary/Cargo.toml"
grep -A2 -F 'name = "mistborn-bootstrap"' "$temporary/Cargo.lock" | grep -Fq 'version = "9.8.7"'

if (cd "$temporary" && python3 set-version.py invalid); then
  printf 'invalid release version unexpectedly succeeded\n' >&2
  exit 1
fi
