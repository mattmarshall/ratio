#!/usr/bin/env bash
# Hands the fund-ops-alerts packer its contract files. A green run is
# the refusals and the cite shape, not a token that opens a book and
# not a live Slack / email / PagerDuty delivery.
set -euo pipefail
ALERTS="$1"
TEST="$2"
APP="$3"
RULES_RS="$4"
CATALOG="$5"
SCREENS="$6"
PROTO="$7"
TYPES="$8"
RECON="$9"
export PYTHONPATH="$(dirname "$ALERTS"):$(dirname "$(dirname "$ALERTS")")"
exec python3 "$TEST" "$APP" "$RULES_RS" "$CATALOG" "$SCREENS" "$PROTO" "$TYPES" "$RECON"
