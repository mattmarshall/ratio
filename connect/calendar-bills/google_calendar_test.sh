#!/usr/bin/env bash
set -euo pipefail
MODULE="$1"
TEST="$2"
export PYTHONPATH="$(dirname "$MODULE"):$(dirname "$(dirname "$MODULE")")"
exec python3 "$TEST"
