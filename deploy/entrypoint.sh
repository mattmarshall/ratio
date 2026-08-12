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

# The tenant boundary (book_path / fund_ids) reads MEMBERSHIP.tsv from the funds
# directory: lines of `<subject>\t<fund-id>`. With RATIO_AUTH=required every
# signed-in visitor is the one invited demo user, so grant that identity every
# seeded fund — otherwise an authenticated visitor signs in successfully and
# sees an empty rail, which reads as "the demo is broken" rather than "you were
# granted nothing". Regenerated on each start from RATIO_DEMO_MEMBER (the email
# the Cognito user was created with; funds_for matches sub OR email) and the
# fund directories that actually exist, so the grant can never name a fund that
# is not there. Unset locally — where the caller is Subject::Local and
# unrestricted — so no file is written and none is read.
if [ -n "${RATIO_DEMO_MEMBER:-}" ] && [ -d "$FUNDS" ]; then
  : > "$FUNDS/MEMBERSHIP.tsv"
  for dir in "$FUNDS"/*/; do
    [ -f "$dir/accounts.json" ] || continue   # a fund is a book, not just a directory
    printf '%s\t%s\n' "$RATIO_DEMO_MEMBER" "$(basename "$dir")" >> "$FUNDS/MEMBERSHIP.tsv"
  done
fi

exec /usr/local/bin/ratio watch --book "$BOOK" --port "${AWS_LWA_PORT:-8080}"
