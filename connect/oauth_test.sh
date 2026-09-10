#!/usr/bin/env bash
set -euo pipefail

OAUTH="$1"
TEST="$2"
shift 2
python3 "$TEST" "$OAUTH" "$@"
