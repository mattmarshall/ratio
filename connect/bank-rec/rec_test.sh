#!/usr/bin/env bash
# Hands the bank-rec app its contract files. A green run is the
# refusals, not a token that opens a book and not a kernel BankRec RPC.
set -euo pipefail
REC="$1"
TEST="$2"
APP="$3"
BOOK_RS="$4"
CATALOG="$5"
SCREENS="$6"
PROTO="$7"
TYPES="$8"
export PYTHONPATH="$(dirname "$REC"):$(dirname "$(dirname "$REC")")"
exec python3 "$TEST" "$APP" "$BOOK_RS" "$CATALOG" "$SCREENS" "$PROTO" "$TYPES"
