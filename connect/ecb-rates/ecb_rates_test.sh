#!/usr/bin/env bash
set -euo pipefail

APP="$1"
TEST="$2"
MANIFEST="$3"
BOOK="$4"
SCOPES="$5"

python3 "$TEST" "$APP" "$MANIFEST" "$BOOK" "$SCOPES"
