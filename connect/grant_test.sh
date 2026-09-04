#!/usr/bin/env bash
# Hands the shared Connect grant helper its own module. A green run
# is the fence (ConnectApiUrl, never DemoUrl, never RATIO_DEMO_OPEN,
# never org:{id}), not a live WorkOS dashboard registration.
set -euo pipefail
GRANT="$1"
TEST="$2"
export PYTHONPATH="$(dirname "$GRANT")"
exec python3 "$TEST"
