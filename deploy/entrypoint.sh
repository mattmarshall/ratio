#!/usr/bin/env bash
#
# Copy the baked demo book somewhere writable, then serve.
#
# A Lambda filesystem is read-only apart from /tmp, and the demo posts entries.
# Copying on start also means every cold start resets the demo to a known
# state — which is what you want in front of a customer, not a book carrying
# whatever the last visitor did to it.
set -euo pipefail

BOOK="${RATIO_BOOK:-/tmp/book}"
if [ ! -d "$BOOK" ]; then
  cp -r /opt/demo-book "$BOOK"
fi

exec /usr/local/bin/ratio watch --book "$BOOK" --port "${AWS_LWA_PORT:-8080}"
