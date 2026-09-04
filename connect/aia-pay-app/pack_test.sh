#!/usr/bin/env bash
# Hands the pay-app packer its contract files. A green run is the
# refusals, not a token that opens a book and not a licensed AIA PDF.
set -euo pipefail
PACK="$1"
TEST="$2"
APP="$3"
RULES_RS="$4"
CATALOG="$5"
SCREENS="$6"
PROTO="$7"
export PYTHONPATH="$(dirname "$PACK")"
exec python3 "$TEST" "$APP" "$RULES_RS" "$CATALOG" "$SCREENS" "$PROTO"
