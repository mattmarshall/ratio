#!/usr/bin/env bash
# Hands the mapper its contract files. A green run is the refusals,
# not a token that opens a book.
set -euo pipefail
MAPPER="$1"
TEST="$2"
APP="$3"
BOOK_RS="$4"
CATALOG="$5"
export PYTHONPATH="$(dirname "$MAPPER"):$(dirname "$(dirname "$MAPPER")")"
exec python3 "$TEST" "$APP" "$BOOK_RS" "$CATALOG"
