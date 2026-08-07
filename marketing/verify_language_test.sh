#!/usr/bin/env bash
#
# Bazel wrapper for verify_language.py, so the licensing sweep runs with every
# other test rather than only when somebody remembers. The .tex files arrive as
# runfiles; the roots are derived from them so adding a new document tree needs
# only a BUILD edit, not a change here.
set -euo pipefail

CHECKER="$1"; shift
# Every remaining argument is a .tex path. Reduce to the distinct top-level
# directories they live in, which is what the checker takes.
ROOTS="$(for f in "$@"; do echo "${f%%/*}"; done | sort -u)"
# shellcheck disable=SC2086
exec python3 "$CHECKER" $ROOTS
