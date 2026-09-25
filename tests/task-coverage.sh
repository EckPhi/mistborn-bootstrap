#!/usr/bin/env bash
set -Eeuo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
coverage="$root/docs/desired-state-task-coverage.tsv"
temporary="$(mktemp -d)"
trap 'rm -rf -- "$temporary"' EXIT

awk -F '\t' '
  NR == 1 {
    if ($0 != "collection\tstage\ttask\tclassification\trust_remediations\trationale") {
      print "unexpected coverage ledger header" > "/dev/stderr"
      exit 1
    }
    next
  }
  NF != 6 || $1 == "" || $2 == "" || $3 == "" || $5 == "" || $6 == "" {
    printf "invalid coverage row %d\n", NR > "/dev/stderr"
    exit 1
  }
  $4 != "migrated" && $4 != "retained Bash adapter" && $4 != "intentionally unsupported" {
    printf "invalid classification on coverage row %d: %s\n", NR, $4 > "/dev/stderr"
    exit 1
  }
  $4 == "migrated" && $5 == "-" {
    printf "migrated task has no Rust remediation on row %d\n", NR > "/dev/stderr"
    exit 1
  }
  $4 != "migrated" && $5 != "-" {
    printf "non-migrated task claims Rust remediation on row %d\n", NR > "/dev/stderr"
    exit 1
  }
  { print $1 "\t" $2 "\t" $3 }
' "$coverage" > "$temporary/ledger.keys"

for collection in server shell; do
  awk -F '"' -v collection="$collection" '
    /^\[\[stages\]\]$/ { mode = "stage"; next }
    /^\[\[stages\.tasks\]\]$/ { mode = "task"; next }
    mode == "stage" && /^id = "/ { stage = $2; next }
    mode == "task" && /^id = "/ { print collection "\t" stage "\t" $2; next }
  ' "$root/plans/$collection.toml" > "$temporary/$collection.keys"
  sort "$temporary/$collection.keys" | uniq -d > "$temporary/$collection.duplicates"
  if [[ -s "$temporary/$collection.duplicates" ]]; then
    printf 'duplicate collection task in %s plan\n' "$collection" >&2
    exit 1
  fi
done

cat "$temporary/server.keys" "$temporary/shell.keys" > "$temporary/plan.keys"

if [[ "$(sort "$temporary/ledger.keys" | uniq -d | wc -l | tr -d ' ')" != 0 ]]; then
  printf 'duplicate collection task in desired-state coverage ledger\n' >&2
  exit 1
fi

sort -u "$temporary/ledger.keys" > "$temporary/ledger.sorted"
sort -u "$temporary/plan.keys" > "$temporary/plan.sorted"
if ! diff -u "$temporary/plan.sorted" "$temporary/ledger.sorted"; then
  printf 'coverage ledger must classify every current collection task exactly once\n' >&2
  exit 1
fi

awk -F '"' '
  /Self::[[:alnum:]]+ => "/ {
    variant = $1
    sub(/^.*Self::/, "", variant)
    sub(/[[:space:]]*=>[[:space:]]*$/, "", variant)
    print variant "\t" $2
  }
' "$root/src/domain.rs" > "$temporary/remediation-ids.tsv"

awk '
  /const REGISTRY:/ { in_registry = 1; next }
  in_registry && /^\];/ { in_registry = 0 }
  in_registry && /id: RemediationId::[[:alnum:]]+/ {
    variant = $0
    sub(/^.*id: RemediationId::/, "", variant)
    sub(/[^[:alnum:]].*$/, "", variant)
    print variant
  }
' "$root/src/reconciliation.rs" | sort -u > "$temporary/registry-variants"

awk -F '\t' '
  FNR == NR { slug[$1] = $2; next }
  { registered[$1] = 1 }
  END {
    for (variant in slug) {
      if (registered[variant]) print slug[variant]
    }
  }
' "$temporary/remediation-ids.tsv" "$temporary/registry-variants" \
  | sort -u > "$temporary/registered-remediations"

awk -F '\t' '
  NR > 1 && $4 == "migrated" {
    count = split($5, ids, ",")
    for (i = 1; i <= count; i++) {
      gsub(/^[[:space:]]+|[[:space:]]+$/, "", ids[i])
      print ids[i]
    }
  }
' "$coverage" | sort -u > "$temporary/migrated-remediations"

comm -23 "$temporary/migrated-remediations" "$temporary/registered-remediations" \
  > "$temporary/unknown-remediations"
if [[ -s "$temporary/unknown-remediations" ]]; then
  printf 'coverage ledger references remediation IDs missing from the Rust enum or registry:\n' >&2
  sed 's/^/  /' "$temporary/unknown-remediations" >&2
  exit 1
fi

printf 'Classified %s collection tasks.\n' "$(wc -l < "$temporary/plan.sorted" | tr -d ' ')"
