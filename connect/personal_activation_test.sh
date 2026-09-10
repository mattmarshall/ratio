#!/usr/bin/env bash
set -euo pipefail

ACTIVATION="$1"
TEST="$2"
DOC="$3"
CONNECT_DIR="$(dirname "$ACTIVATION")"
export PYTHONPATH="$CONNECT_DIR:$CONNECT_DIR/bank-feed:$CONNECT_DIR/calendar-bills"
exec python3 "$TEST" "$DOC"
