#!/usr/bin/env bash
# Hands the program roll-up its contract files. A green run is the
# refusals, not a token that opens a book and not a mega-book.
set -euo pipefail
ROLLUP="$1"
TEST="$2"
APP="$3"
RULES_RS="$4"
CATALOG="$5"
SCREENS="$6"
PROTO="$7"
TYPES="$8"
export PYTHONPATH="$(dirname "$ROLLUP")"
exec python3 "$TEST" "$APP" "$RULES_RS" "$CATALOG" "$SCREENS" "$PROTO" "$TYPES"
