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

SHAPES_RS="$1"
HANDOFF="$2"

fail() { echo "  x $*" >&2; exit 1; }

[ -f "$SHAPES_RS" ] || fail "no scale.rs at $SHAPES_RS"
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
    /^(pub )?const SHAPES/ { inside = 1 }
    inside && /name: "/   { gsub(/.*name: "|".*/, ""); n = $0 }
    inside && /securities:/ { gsub(/[^0-9]/, ""); s = $0 }
    inside && /lots_per:/   { gsub(/[^0-9]/, ""); l = $0 }
    inside && /open_lots:/  { gsub(/[^0-9]/, ""); o = $0 }
    inside && /entries:/    { gsub(/[^0-9]/, ""); e = $0; print n, s, l, o, e }
    inside && /^\];/      { exit }
  ' "$SHAPES_RS"
)

[ "$checked" -eq 3 ] || fail "read $checked shapes out of $SHAPES_RS, expected 3 — \
the table moved and this check stopped seeing it, which is how a check like this \
stops working"

# ⭐ THE ONE THAT IS NOT INTERCHANGEABLE. The full shape must be the RECORDED
# twenty-million-lot fund, not the dialled one — 10,000 securities, not 500.
grep -A 8 'name: "full"' "$SHAPES_RS" | grep -q 'securities: 10_000' \
  || fail "the full shape is not 10,000 securities — that is ratio closure's \
500 x 40,000 dial, which is the same lot count and a twentieth of the mark cost"

echo "  ok  the full shape is the recorded 10,000 x 2,000, not the dialled 500 x 40,000"
echo "  ok  $checked shapes, all of them measured in HANDOFF.md"

# ⭐ STAGE E PROJECTION FOLD. Same geometry, not the 140M-entry journal.
# scale.rs, HANDOFF.md, and fold_scale.recorded.json must name one digest.
RECORDED="${3:-}"
[ -n "$RECORDED" ] && [ -f "$RECORDED" ] || fail "no fold_scale.recorded.json at ${RECORDED:-<unset>}"

digest="$(awk '/^pub const STAGE_E_FOLD_DIGEST/ { getline; gsub(/[^0-9a-f]/, ""); print; exit }' "$SHAPES_RS")"
[ "${#digest}" -eq 64 ] || fail "STAGE_E_FOLD_DIGEST is not a 64-hex digest in $SHAPES_RS"

grep -q 'STAGE_E_FOLD_SECURITIES: i64 = 10_000' "$SHAPES_RS" \
  || fail "STAGE_E_FOLD_SECURITIES is not 10,000 — that is the wrong geometry"
grep -q 'STAGE_E_FOLD_LOTS_PER: i64 = 2_000' "$SHAPES_RS" \
  || fail "STAGE_E_FOLD_LOTS_PER is not 2,000"
grep -q 'STAGE_E_FOLD_LOTS: i64 = 20_000_000' "$SHAPES_RS" \
  || fail "STAGE_E_FOLD_LOTS is not 20,000,000"

grep -q "$digest" "$HANDOFF" \
  || fail "HANDOFF.md does not cite Stage E digest $digest"
grep -q "$digest" "$RECORDED" \
  || fail "fold_scale.recorded.json does not cite Stage E digest $digest"
grep -q '"lots": 20000000' "$RECORDED" \
  || fail "recorded artifact is not 20,000,000 lots"
grep -q 'relieve_by' "$RECORDED" \
  || fail "recorded artifact dropped relieve_by"
grep -q 'journal.jsonl stays SoR' "$RECORDED" \
  || fail "recorded artifact dropped the journal-stays-SoR sentence"

echo "  ok  Stage E fold cite is 10,000 x 2,000 / 20,000,000 lots, digest $digest"
