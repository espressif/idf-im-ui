#!/usr/bin/env bash
# Fails when any Rust function exceeds the cognitive or cyclomatic complexity
# limits, as measured by rust-code-analysis-cli. Closures are counted as part
# of the function that contains them.
#
# Usage: check_complexity.sh [SRC_DIR] [--report N]
#   SRC_DIR     Rust sources to analyze (default: src-tauri/src)
#   --report N  Print the N most complex functions and exit 0 instead of gating
#
# Limits can be overridden with MAX_COGNITIVE and MAX_CYCLOMATIC. Functions
# listed in .github/scripts/complexity_allowlist.txt ("path::function" per
# line, '#' comments allowed) are reported but do not fail the check.
set -euo pipefail

SRC_DIR="${1:-src-tauri/src}"
REPORT=""
if [ "${2:-}" = "--report" ]; then
  REPORT="${3:-30}"
fi
MAX_COGNITIVE="${MAX_COGNITIVE:-25}"
MAX_CYCLOMATIC="${MAX_CYCLOMATIC:-25}"
ALLOWLIST="$(dirname "$0")/complexity_allowlist.txt"

out_dir="$(mktemp -d)"
trap 'rm -rf "$out_dir"' EXIT

rust-code-analysis-cli -m -O json -p "$SRC_DIR" -o "$out_dir" >/dev/null

# One TSV row per function: cognitive, cyclomatic, sloc, "file::name", lines.
rows="$(find "$out_dir" -name '*.json' -print0 | xargs -0 jq -r '
  .name as $file
  | .. | objects | select(.kind? == "function" and .name != "<anonymous>")
  | [ .metrics.cognitive.sum, .metrics.cyclomatic.sum, .metrics.loc.sloc,
      "\($file | sub("^.*?src/"; ""))::\(.name)", "\(.start_line)-\(.end_line)" ]
  | @tsv' | sort -t$'\t' -k1,1nr -k2,2nr)"

if [ -n "$REPORT" ]; then
  printf 'cognitive\tcyclomatic\tsloc\tfunction\tlines\n'
  head -n "$REPORT" <<<"$rows"
  exit 0
fi

allowed=""
[ -f "$ALLOWLIST" ] && allowed="$(grep -v '^\s*\(#\|$\)' "$ALLOWLIST" || true)"

violations=0
while IFS=$'\t' read -r cognitive cyclomatic sloc name lines; do
  [ -z "$name" ] && continue
  if awk -v a="$cognitive" -v b="$cyclomatic" -v ma="$MAX_COGNITIVE" -v mb="$MAX_CYCLOMATIC" \
    'BEGIN { exit !(a > ma || b > mb) }'; then
    if grep -qxF "$name" <<<"$allowed"; then
      echo "allowed  $name ($lines): cognitive ${cognitive%.*}, cyclomatic ${cyclomatic%.*}"
    else
      echo "TOO COMPLEX  $name ($lines): cognitive ${cognitive%.*} (max $MAX_COGNITIVE), cyclomatic ${cyclomatic%.*} (max $MAX_CYCLOMATIC)"
      violations=$((violations + 1))
    fi
  fi
done <<<"$rows"

if [ "$violations" -gt 0 ]; then
  echo "$violations function(s) exceed the complexity limits. Split them into smaller functions."
  exit 1
fi
echo "All functions are within cognitive <= $MAX_COGNITIVE and cyclomatic <= $MAX_CYCLOMATIC."
