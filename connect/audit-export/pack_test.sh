#!/usr/bin/env bash
# Hands the audit-export packer its contract files. A green run is the
# refusals and the pack shape, not a token that opens a book and not
# a live ZIP against /v1.
set -euo pipefail
PACK="$1"
TEST="$2"
APP="$3"
RULES_RS="$4"
CATALOG="$5"
SCREENS="$6"
PROTO="$7"
TYPES="$8"
RECON="$9"
export PYTHONPATH="$(dirname "$PACK"):$(dirname "$(dirname "$PACK")")"
exec python3 "$TEST" "$APP" "$RULES_RS" "$CATALOG" "$SCREENS" "$PROTO" "$TYPES" "$RECON"
