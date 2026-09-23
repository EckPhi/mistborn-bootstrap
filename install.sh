#!/usr/bin/env bash
set -Eeuo pipefail

readonly MISTBORN_DEFAULT_VERSION="v0.5.5"
readonly MISTBORN_REPOSITORY="EckPhi/mistborn-bootstrap"

fail() {
  printf 'mistborn-bootstrap: %s\n' "$*" >&2
  exit 1
}

download() {
  local url="$1"
  local destination="$2"
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL --retry 3 --output "$destination" "$url"
  elif command -v wget >/dev/null 2>&1; then
    wget -q --tries=3 --output-document="$destination" "$url"
  else
    fail "curl or wget is required"
  fi
}

[[ $# -ge 1 ]] || fail "usage: install.sh COLLECTION [OPTIONS]"
collection="$1"
shift
case "$collection" in
  server | shell) ;;
  *) fail "unsupported collection: $collection" ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) architecture="x86_64" ;;
  aarch64 | arm64) architecture="aarch64" ;;
  *) fail "unsupported architecture: $(uname -m)" ;;
esac

version="${MISTBORN_VERSION:-$MISTBORN_DEFAULT_VERSION}"
release_base="${MISTBORN_RELEASE_BASE_URL:-https://github.com/$MISTBORN_REPOSITORY/releases/download/$version}"
raw_base="${MISTBORN_RAW_BASE_URL:-https://raw.githubusercontent.com/$MISTBORN_REPOSITORY/$version}"
asset="mistborn-bootstrap-linux-$architecture"
work_dir="$(mktemp -d)"
trap 'rm -rf -- "$work_dir"' EXIT
mkdir -p "$work_dir/collections" "$work_dir/dist" "$work_dir/plans"

printf 'Downloading Mistborn Bootstrap %s for %s...\n' "$version" "$architecture"
download "$release_base/$asset" "$work_dir/$asset"
download "$release_base/SHA256SUMS" "$work_dir/SHA256SUMS"
download "$raw_base/collections/$collection.modules" "$work_dir/collections/$collection.modules"
download "$raw_base/dist/$collection.sh" "$work_dir/dist/$collection.sh"
download "$raw_base/plans/$collection.toml" "$work_dir/plans/$collection.toml"

command -v sha256sum >/dev/null 2>&1 || fail "sha256sum is required"
checksum_line="$(awk -v asset="$asset" '$2 == asset || $2 == "*" asset { print }' "$work_dir/SHA256SUMS")"
[[ -n "$checksum_line" ]] || fail "release checksum for $asset is missing"
[[ "$(printf '%s\n' "$checksum_line" | wc -l)" -eq 1 ]] || fail "release checksum for $asset is ambiguous"
(cd "$work_dir" && printf '%s\n' "$checksum_line" | sha256sum --check --status) || fail "binary checksum verification failed"
chmod 0755 "$work_dir/$asset"

runner=("$work_dir/$asset" run "$collection" --root "$work_dir" "$@")
if [[ -r /dev/tty && -w /dev/tty ]]; then
  "${runner[@]}" </dev/tty
else
  "${runner[@]}"
fi
