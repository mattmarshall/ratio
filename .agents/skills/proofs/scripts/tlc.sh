#!/usr/bin/env bash
#
# Run TLC on one TLA+ model directly, outside Bazel, and print what it found.
#
# Usage:  .agents/skills/proofs/scripts/tlc.sh tla/MCStrikeDay.cfg [tla/MCStrikeDay.tla]
#
# The module defaults to the cfg's name with .tla. Prints the verdict, the
# violated invariant if any, and the state counts.
#
# ⭐ WHY NOT JUST `bazel test //tla:<check>`: rules_tla runs TLC inside a build
# action and keeps its log only on failure, so a passing check says nothing
# about how much it explored. "PASSED in 0.0s" over a model that explored ten
# states is a green suite that tests nothing; this shows the count. Use Bazel
# for the gate, this for the number.
#
# It reuses the tla2tools.jar Bazel already pinned, so TLC's version matches CI.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

cfg="${1:?usage: tlc.sh CFG [MODULE]}"
module="${2:-${cfg%.cfg}.tla}"
[ -f "$cfg" ] || { echo "no such cfg: $cfg" >&2; exit 2; }
[ -f "$module" ] || { echo "no such module: $module" >&2; exit 2; }

find_jar() {
  find "$(bazel info output_base 2>/dev/null)/external" -path '*tla2tools*' -name tla2tools.jar 2>/dev/null | head -1
}
jar="$(find_jar)"
if [ -z "$jar" ]; then
  # Any tla_check pulls the pinned jar into the external repository cache.
  bazel fetch //tla:valuation_check >/dev/null 2>&1 || true
  jar="$(find_jar)"
fi
[ -n "$jar" ] || { echo "tla2tools.jar not found; does 'bazel fetch //tla:valuation_check' work?" >&2; exit 2; }
command -v java >/dev/null || { echo "TLC needs a JDK on PATH" >&2; exit 2; }

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
# TLC resolves EXTENDS against the module's own directory.
set +e
(cd "$(dirname "$module")" && java -XX:+UseParallelGC -Djava.io.tmpdir="$work" \
  -cp "$jar" tlc2.TLC -metadir "$work/md" -config "$(basename "$cfg")" "$(basename "$module")") \
  >"$work/log" 2>&1
status=$?
set -e

grep -E "^Error:|is violated|No error has been found|states generated|depth of the complete|Deadlock reached" \
  "$work/log" || tail -20 "$work/log"
exit "$status"
