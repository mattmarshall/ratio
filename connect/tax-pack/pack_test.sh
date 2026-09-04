#!/usr/bin/env bash
# Hands the pack builder its contract files. A green run is the
# refusals, not a token that opens a book and not an IRS e-file.
set -euo pipefail
PACK="$1"
TEST="$2"
APP="$3"
RULES_RS="$4"
CATALOG="$5"
SCREENS="$6"
export PYTHONPATH="$(dirname "$PACK")"
exec python3 "$TEST" "$APP" "$RULES_RS" "$CATALOG" "$SCREENS"
