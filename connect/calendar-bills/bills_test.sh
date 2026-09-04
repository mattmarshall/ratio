#!/usr/bin/env bash
# Hands the bills app its contract files. A green run is the refusals,
# not a token that opens a book and not a calendar invention in core.
set -euo pipefail
BILLS="$1"
TEST="$2"
APP="$3"
BOOK_RS="$4"
CATALOG="$5"
SCREENS="$6"
PROTO="$7"
export PYTHONPATH="$(dirname "$BILLS")"
exec python3 "$TEST" "$APP" "$BOOK_RS" "$CATALOG" "$SCREENS" "$PROTO"
