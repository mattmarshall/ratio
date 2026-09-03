#!/usr/bin/env bash
#
# Copy the baked demo book somewhere writable, then serve — or, given arguments,
# run those instead.
#
# A Lambda filesystem is read-only apart from /tmp, so the seeded chart and
# config have to land there. The journal does not: when RATIO_JOURNAL_BUCKET is
# set, FileBook appends to the object store (`tla/S3Journal.tla`) and this copy
# is only the seed that hydrates an empty prefix. A cold start without that
# store used to reset every ApplyEvent; two containers used to build two
# journals under one URL. That is issue #24.
set -euo pipefail

# ⛔ ARGUMENTS RUN THE BINARY, AND WITHOUT THIS THEY WERE SILENTLY DISCARDED.
# Docker appends a `docker run` command to ENTRYPOINT, so `docker run <image>
# bench --fold --json` arrived here as $1..$n and this script — which never
# mentioned "$@" — went on to exec `ratio watch` regardless. There is no CMD to
# override either. The container therefore ran a WEB SERVER when asked for a
# benchmark, and the only symptom was a task that never produced output and
# never exited: a hang, from a program working perfectly on a different job.
#
# ⚠ The seeding below is deliberately skipped in this mode. A one-shot run
# brings its own book and copying two demo books first would be ~30 MB of
# pointless IO before every measurement — and `RATIO_BOOK` would be seeded to
# somewhere the caller did not ask about.
if [ "$#" -gt 0 ]; then
  exec /usr/local/bin/ratio "$@"
fi

BOOK="${RATIO_BOOK:-/tmp/book}"
if [ ! -d "$BOOK" ]; then
  cp -r /opt/demo-book "$BOOK"
fi

# The console's fund list is a directory OF books, which is a different thing
# from the single book the other screens read. Both are baked in and both are
# copied, because the seeded chart and config still have to live somewhere
# writable. Writes to the journal go to RATIO_JOURNAL_BUCKET when set; /tmp
# is not the system of record for those.
FUNDS="${RATIO_FUNDS:-/tmp/funds}"
if [ ! -d "$FUNDS" ] && [ -d /opt/demo-funds ]; then
  cp -r /opt/demo-funds "$FUNDS"
fi

# The tenant boundary (book_path / fund_ids) reads MEMBERSHIP.tsv from the funds
# directory: lines of `<subject>\t<fund-id>`. With RATIO_AUTH=required every
# signed-in visitor is a demo member, so grant each of them every seeded fund —
# otherwise a visitor signs in successfully and sees an empty rail, which reads
# as "the demo is broken" rather than "you were granted nothing". Regenerated on
# each start from RATIO_DEMO_MEMBER and the fund directories that actually exist,
# so the grant can never name a fund that is not there.
#
# ⚠ RATIO_DEMO_MEMBER MAY NAME SEVERAL MEMBERS, comma-separated: the
# email/password demo user AND one or more federated (Google) emails. It has to,
# because a Google sign-in arrives as the caller's real Google email — not
# `demo@…` — and `funds_for` matches that email against this file. A single-value
# RATIO_DEMO_MEMBER is just a list of one. Unset locally — where the caller is
# Subject::Local and unrestricted — so no file is written and none is read.
if [ -n "${RATIO_DEMO_MEMBER:-}" ] && [ -d "$FUNDS" ]; then
  : > "$FUNDS/MEMBERSHIP.tsv"
  members="$(printf '%s' "$RATIO_DEMO_MEMBER" | tr ',' ' ')"
  for dir in "$FUNDS"/*/; do
    [ -f "$dir/accounts.json" ] || continue   # a fund is a book, not just a directory
    fund="$(basename "$dir")"
    for member in $members; do
      printf '%s\t%s\n' "$member" "$fund" >> "$FUNDS/MEMBERSHIP.tsv"
    done
  done
fi

exec /usr/local/bin/ratio watch --book "$BOOK" --port "${AWS_LWA_PORT:-8080}"
