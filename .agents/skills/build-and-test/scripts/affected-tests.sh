#!/usr/bin/env bash
#
# Run the Bazel tests that depend on what you changed, instead of all of //....
#
# Usage (from anywhere in the repository):
#   .agents/skills/build-and-test/scripts/affected-tests.sh            # vs origin/main
#   .agents/skills/build-and-test/scripts/affected-tests.sh BASE_REF   # vs another ref
#   .agents/skills/build-and-test/scripts/affected-tests.sh --list     # print, do not run
#
# "Changed" is the union of commits since the merge base, staged and unstaged
# edits, and untracked files.
#
# ⚠ NOT A SUBSTITUTE FOR `bazel test //...` BEFORE A PR. It finds tests that
# declare a dependency on a changed file. A check that reads a file it does not
# declare, or a change Bazel cannot see (console/, sdk/typescript/, site/), is
# not found here; those files are listed at the end so you know to check them
# another way.
#
# ⛔ manual-tagged targets are excluded on purpose. The TLA+ probes are meant to
# FAIL, and the live Postgres tests need a server.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

list_only=0
base="origin/main"
for arg in "$@"; do
  case "$arg" in
    --list) list_only=1 ;;
    *) base="$arg" ;;
  esac
done

mapfile -t changed < <(
  {
    git diff --name-only "$(git merge-base "$base" HEAD)" HEAD
    git diff --name-only
    git diff --name-only --cached
    git ls-files --others --exclude-standard
  } | sort -u
)

# Only files that still exist can be labels. A deleted file's dependents are
# found through the BUILD file that stopped naming it, which is also a change.
files=()
for f in "${changed[@]}"; do
  [ -f "$f" ] && files+=("$f")
done

if [ "${#files[@]}" -eq 0 ]; then
  echo "nothing changed relative to $base"
  exit 0
fi

# Resolve every path to a source label in ONE query. --keep_going drops the
# paths no package owns instead of failing on the first; mapping each label back
# to its path finds which ones those were.
mapfile -t labels < <(bazel query --noshow_progress --keep_going \
  "set(${files[*]})" 2>/dev/null | grep '^//' || true)
declare -A owned=()
for l in "${labels[@]}"; do
  pkg="${l#//}"; pkg="${pkg%%:*}"; name="${l#*:}"
  owned["${pkg:+$pkg/}$name"]=1
done
unowned=()
for f in "${files[@]}"; do
  [ -n "${owned[$f]:-}" ] || unowned+=("$f")
done

tests=""
if [ "${#labels[@]}" -gt 0 ]; then
  tests=$(bazel query --noshow_progress --keep_going \
    "tests(rdeps(//..., set(${labels[*]}))) except attr(tags, 'manual', //...)" 2>/dev/null | sort)
fi

echo "changed files: ${#files[@]} (${#labels[@]} in the Bazel graph)"
if [ -n "$tests" ]; then
  echo "affected tests: $(wc -l <<<"$tests")"
  sed 's/^/  /' <<<"$tests"
else
  echo "affected tests: none"
fi

if [ "${#unowned[@]}" -gt 0 ]; then
  echo
  echo "not in the Bazel graph, so no Bazel test covers them:"
  printf '  %s\n' "${unowned[@]}"
  echo "  console/** -> the console skill; site/** -> site/build.py + site/verify.py;"
  echo "  *.md read by a check -> run that check by name (see the build-and-test skill)."
fi

if [ "$list_only" -eq 1 ] || [ -z "$tests" ]; then
  exit 0
fi

# shellcheck disable=SC2086
exec bazel test --test_output=errors $tests
