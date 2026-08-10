#!/usr/bin/env bash
#
# Run every failure-path probe and check each goes red FOR THE REASON IT NAMES.
#
# ⛔ NOT A TEST, AND DELIBERATELY NOT ONE. Each probe is a full TLC run, and
# `bazel test //...` already takes long enough that adding thirteen more model
# checks to every commit is how the suite stops being run. This is the sweep to
# run after changing a spec.
#
# ⚠ THE EXIT CODE IS NOT THE CHECK. Every one of these exits non-zero — that is
# what a probe is. What matters is WHICH invariant TLC reported, because a
# missing constant, a parse error and a violated invariant all fail identically.
# `//tla:probes_test` catches the configuration half of that statically and runs
# in CI; this catches the half only TLC can see: a dial that stopped mattering.
#
# Usage:  tla/probes.sh            from the workspace root
set -uo pipefail

cd "$(dirname "$0")/.."

red=0
green=0

# Every manual-tagged tla_check, paired with the invariant its cfg claims.
while read -r target cfg; do
  want=$(sed -nE 's/^\\\* ([A-Za-z_][A-Za-z0-9_]*) MUST go red\..*/\1/p' "tla/$cfg" | head -1)
  if [ -z "$want" ]; then
    echo "  x $target — its cfg names no invariant; //tla:probes_test should have caught this"
    red=$((red + 1))
    continue
  fi
  out=$(bazel test "//tla:$target" --test_output=errors 2>&1)
  if grep -qE "(Invariant|property) $want is violated" <<<"$out"; then
    echo "  ok  $target — $want went red"
    green=$((green + 1))
  elif grep -q "Temporal properties were violated" <<<"$out" &&
       grep -q "$want" <<<"$out"; then
    echo "  ok  $target — $want went red"
    green=$((green + 1))
  else
    echo "  x $target — expected $want to be violated. TLC said:"
    grep -E "^Error:|is violated|Temporal properties" <<<"$out" | head -3 | sed 's/^/        /'
    red=$((red + 1))
  fi
done < <(python3 - <<'PY'
import re, pathlib
build = pathlib.Path("tla/BUILD.bazel").read_text()
for block in re.findall(r"tla_check\(\s*(.*?)\n\)", build, re.S):
    if "manual" not in block:
        continue
    name = re.search(r'name\s*=\s*"([^"]+)"', block)
    cfg = re.search(r'config\s*=\s*"([^"]+)"', block)
    if name and cfg:
        print(name.group(1), cfg.group(1))
PY
)

echo
if [ "$red" -ne 0 ]; then
  echo "$red probe(s) did not break what they name — an invariant has stopped checking"
  exit 1
fi
echo "$green probes, every one red for the reason it names"
