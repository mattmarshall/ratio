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

# The console's fund list is a directory OF books, which is a different thing
# from the single book the other screens read. Both are baked in and both are
# copied, because /tmp is the only writable place and the demo posts to one of
# them.
FUNDS="${RATIO_FUNDS:-/tmp/funds}"
if [ ! -d "$FUNDS" ] && [ -d /opt/demo-funds ]; then
  cp -r /opt/demo-funds "$FUNDS"
fi

exec /usr/local/bin/ratio watch --book "$BOOK" --port "${AWS_LWA_PORT:-8080}"
