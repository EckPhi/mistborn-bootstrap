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
      'MISTBORN_DRY_RUN=0' 'MISTBORN_YES=0' 'MISTBORN_ONLY=""'
    printf 'MISTBORN_MODULES=('
    while IFS= read -r module; do
      [[ -z "$module" || "$module" == \#* ]] && continue
      printf ' %q' "$module"
    done <"$root/collections/$collection.modules"
    printf ' )\n'
    printf "export MISTBORN_TOOL_B64='%s'\n" "$(base64 <"$root/assets/mistborn" | tr -d '\n')"
    printf "export MISTBORN_UPDATE_PLAN_B64='%s'\n" "$(base64 <"$root/plans/update.toml" | tr -d '\n')"
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
      --only) shift; MISTBORN_ONLY="\${1:?--only requires a module name}" ;;
      -h|--help) printf 'Usage: %s [--dry-run] [--yes] [--user NAME] [--only MODULE]\n' "\$0"; return ;;
      *) ui_error "Unknown argument: \$1"; return 2 ;;
    esac
    shift
  done
  if [[ -n "\$MISTBORN_ONLY" ]]; then
    local known=0 module
    for module in "\${MISTBORN_MODULES[@]}"; do
      [[ "\$module" == "\$MISTBORN_ONLY" ]] && known=1
    done
    [[ "\$known" == 1 ]] || { ui_error "Unknown module: \$MISTBORN_ONLY"; return 2; }
  fi
  [[ -n "\$MISTBORN_ONLY" ]] || ui_header "Mistborn $collection setup"
RUNNER
    while IFS= read -r module; do
      [[ -z "$module" || "$module" == \#* ]] && continue
      # MISTBORN_ONLY belongs to the generated script, not this generator.
      # shellcheck disable=SC2016
      printf '  [[ -n "$MISTBORN_ONLY" && "$MISTBORN_ONLY" != %q ]] || module_%s_apply\n' "$module" "$module"
    done <"$root/collections/$collection.modules"
    # MISTBORN_ONLY belongs to the generated script, not this generator.
    # shellcheck disable=SC2016
    printf '%s\n' '  [[ -n "$MISTBORN_ONLY" ]] || ui_header "Setup complete"' '}' 'main "$@"'
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
