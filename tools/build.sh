#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$root/dist"

build_collection() {
  local collection="$1" output module
  output="$root/dist/$collection.sh"
  {
    # The generated trap intentionally expands LINENO when it runs.
    # shellcheck disable=SC2016
    printf '%s\n' '#!/usr/bin/env bash' 'set -Eeuo pipefail' \
      'trap '\''ui_error "Setup failed on line $LINENO"'\'' ERR' \
      'MISTBORN_DRY_RUN=0' 'MISTBORN_YES=0'
    cat "$root/lib/ui.sh" "$root/lib/system.sh"
    while IFS= read -r module; do
      [[ -z "$module" || "$module" == \#* ]] && continue
      [[ -f "$root/modules/$module.sh" ]] || { echo "Unknown module: $module" >&2; return 1; }
      cat "$root/modules/$module.sh"
    done <"$root/collections/$collection.modules"
    cat <<RUNNER
main() {
  while [[ \$# -gt 0 ]]; do
    case "\$1" in
      --dry-run) MISTBORN_DRY_RUN=1 ;;
      --yes) MISTBORN_YES=1 ;;
      --user) shift; MISTBORN_USER="\${1:?--user requires a value}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME]\n' "\$0"; return ;;
      *) ui_error "Unknown argument: \$1"; return 2 ;;
    esac
    shift
  done
  ui_header "Mistborn $collection setup"
RUNNER
    while IFS= read -r module; do
      [[ -z "$module" || "$module" == \#* ]] && continue
      printf '  module_%s_apply\n' "$module"
    done <"$root/collections/$collection.modules"
    printf '%s\n' '  ui_header "Setup complete"' '}' 'main "$@"'
  } >"$output"
  chmod +x "$output"
  bash -n "$output"
  echo "Built dist/$collection.sh"
}

if [[ $# -gt 0 ]]; then
  build_collection "$1"
else
  for file in "$root"/collections/*.modules; do build_collection "$(basename "$file" .modules)"; done
fi
