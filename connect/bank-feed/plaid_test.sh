#!/usr/bin/env bash
set -euo pipefail
PLAID="$1"
TEST="$2"
export PYTHONPATH="$(dirname "$PLAID"):$(dirname "$(dirname "$PLAID")")"
exec python3 "$TEST"
