#!/usr/bin/env bash
# Hands the vendor portal its contract files. A green run is the
# refusals, not a token that opens a book and not a vendor directory.
set -euo pipefail
PORTAL="$1"
TEST="$2"
APP="$3"
BOOK_RS="$4"
RULES_RS="$5"
CATALOG="$6"
SCREENS="$7"
PROTO="$8"
export PYTHONPATH="$(dirname "$PORTAL"):$(dirname "$(dirname "$PORTAL")")"
exec python3 "$TEST" "$APP" "$BOOK_RS" "$RULES_RS" "$CATALOG" "$SCREENS" "$PROTO"
