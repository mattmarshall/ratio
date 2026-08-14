#!/usr/bin/env bash
#
# The scale screen offers three shapes to fold. This checks they are the shapes
# HANDOFF.md actually measured.
#
# ⛔ THE DEFECT THIS IS FOR ALREADY HAPPENED ONCE, IN THE PLAN FOR THIS SCREEN.
# The twenty-million-lot row HANDOFF records is 10,000 securities × 2,000 lots.
# `ratio closure` — and therefore the estimate panel on the same screen — dials
# 500 × 40,000. Both are twenty million open tax lots and they are NOT the same
# fund: the mark phase reads one price per SECURITY, so the recorded shape marks
# ten thousand names and the dialled one marks five hundred. Estimating one and
# running the other, both captioned "twenty million tax lots", produces two
# figures that each tie and describe different books.
#
# ⚠ WHAT THIS CANNOT DO, STATED PLAINLY. It is a cross-check between two
# DECLARED lists — the `SHAPES` table in watch.rs and the table in HANDOFF.md —
# in the same spirit as `plan_refusals_test.sh`, and with the same honest
# limitation: it cannot tell you either list is TRUE. What it buys is that the
# two cannot drift apart in silence, which is the failure that costs a day.
#
# The truth of the numbers is checked by running them: `ratio bench` reproduces
# the entry counts exactly, because ratio-gen is deterministic.
set -euo pipefail

WATCH="$1"
HANDOFF="$2"

fail() { echo "  x $*" >&2; exit 1; }

[ -f "$WATCH" ] || fail "no watch.rs at $WATCH"
[ -f "$HANDOFF" ] || fail "no HANDOFF.md at $HANDOFF"

# The measurement table in HANDOFF, flattened. Each row is
# `lots/sec  open-lots  entries  COLD-BUILD ...`.
rows="$(grep -E '^ *[0-9]+ +[0-9]+ +[0-9]+ +[0-9.]+ s' "$HANDOFF" || true)"
[ -n "$rows" ] || fail "found no measurement rows in $HANDOFF — did the table change shape?"

# ⛔ EVERY SHAPE, AND THE COUNT TOO. Checking only the ones that happen to be
# present would pass a table somebody had quietly emptied.
checked=0
while read -r name securities lots_per open_lots entries; do
  # The table is keyed on open lots and entries, which are what identify a run;
  # `lots/sec` is `lots_per` and appears in the first column.
  hit="$(awk -v l="$lots_per" -v o="$open_lots" -v e="$entries" \
    '$1==l && $2==o && $3==e {print; exit}' <<<"$rows" || true)"
  [ -n "$hit" ] || fail "the $name shape ($securities x $lots_per → $open_lots lots, \
$entries entries) matches no row in HANDOFF.md — one of the two has moved"
  echo "  ok  $name: $open_lots lots over $entries entries, recorded in HANDOFF"
  checked=$((checked + 1))
done < <(
  # Pull the SHAPES table out of watch.rs: name, securities, lots_per,
  # open_lots, entries — in declaration order.
  awk '
    /^const SHAPES/       { inside = 1 }
    inside && /name: "/   { gsub(/.*name: "|".*/, ""); n = $0 }
    inside && /securities:/ { gsub(/[^0-9]/, ""); s = $0 }
    inside && /lots_per:/   { gsub(/[^0-9]/, ""); l = $0 }
    inside && /open_lots:/  { gsub(/[^0-9]/, ""); o = $0 }
    inside && /entries:/    { gsub(/[^0-9]/, ""); e = $0; print n, s, l, o, e }
    inside && /^\];/      { exit }
  ' "$WATCH"
)

[ "$checked" -eq 3 ] || fail "read $checked shapes out of watch.rs, expected 3 — \
the table moved and this check stopped seeing it, which is how a check like this \
stops working"

# ⭐ THE ONE THAT IS NOT INTERCHANGEABLE. The full shape must be the RECORDED
# twenty-million-lot fund, not the dialled one — 10,000 securities, not 500.
grep -A 8 'name: "full"' "$WATCH" | grep -q 'securities: 10_000' \
  || fail "the full shape is not 10,000 securities — that is ratio closure's \
500 x 40,000 dial, which is the same lot count and a twentieth of the mark cost"

echo "  ok  the full shape is the recorded 10,000 x 2,000, not the dialled 500 x 40,000"
echo "  ok  $checked shapes, all of them measured in HANDOFF.md"
