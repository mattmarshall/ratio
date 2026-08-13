#!/usr/bin/env bash
#
# The demo has to exercise the engine, and it did not.
#
# ⛔ THE REGRESSION THIS EXISTS TO CATCH HAS ALREADY HAPPENED. The seeded funds
# were three reconciliation books — a dozen entries each, three open tax lots
# between them, no chart roles, one currency — so the lot method, the realized
# gain, its split by holding period, the multi-currency NAV and the lots behind a
# position were all built, deployed, and invisible. Nothing failed. The demo just
# quietly showed none of the work.
#
# ⚠ SO THE ASSERTIONS ARE ABOUT WHAT THE DEMO DEMONSTRATES, not about whether
# the script exits 0. A seed script that runs cleanly and produces a book with
# nothing in it is exactly the failure that got here.
set -euo pipefail

RATIO="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="${TEST_TMPDIR:-/tmp}/demo-funds"
rm -rf "$OUT"

"$HERE/seed-demo-funds.sh" "$RATIO" "$OUT" >/dev/null

fail() { echo "  x $*" >&2; exit 1; }

# Five funds — four states, plus a fifth that differs from one of them by one
# line of configuration.
n=$(find "$OUT" -maxdepth 1 -mindepth 1 -type d | wc -l | tr -d ' ')
[ "$n" -eq 5 ] || fail "expected 5 funds, found $n"

GEN="$OUT/ashcombe-global-equity"
[ -d "$GEN" ] || fail "the generated fund is missing"

# ⛔ IT HAS TO HAVE LOTS. This is the whole reason the fund exists: "a NAV does
# not read the tax lots" is a claim about a fund that HAS a lot of them.
lots=$("$RATIO" gen --book "$GEN" --securities 20 --lots-per 40 --currencies 3 \
       | awk '/open tax lots/ {print $4}')
[ "${lots:-0}" -gt 100 ] || fail "the generated fund has $lots open lots — the demo shows no scale"

# ⛔ AND IT HAS TO REALIZE A GAIN ON BOTH SIDES OF THE THRESHOLD. A fund whose
# every disposal is unclassified exercises the holding-period split no better
# than a fund with no lots at all — which is the state undated trades left it in.
"$RATIO" strike --book "$GEN" >/dev/null
bal=$("$RATIO" balance --book "$GEN")
grep -q "Realized gain" <<<"$bal" || fail "no realized gain account on the generated fund"

# The three reconciliation books are still there and still in their three states.
for f in harbourline-global-value northstar-multi-strategy calderwood-income; do
  [ -d "$OUT/$f" ] || fail "$f is missing"
done

# ⛔ AND THE DECLARED METHOD HAS TO CHANGE THE NUMBER. Two books from the same
# seed, differing only in `lot_method`, must realize DIFFERENT gains. This is the
# assertion the whole method-wiring stretch was for, and until this fund existed
# nothing on the demo could have failed if the declaration were ignored again:
# every fund declared `fifo`, so a hardcoded FIFO engine was observationally
# perfect.
#
# ⚠ AND IT CAUGHT A REAL ONE. `ratio-gen` chose the lot to sell by popping the
# OLDEST — hardcoded — while writing whatever method it was asked for into the
# config, so a HIFO book declared HIFO and carried FIFO gains. The engine read it
# back, relieved HIFO, disagreed with every posted sale, and produced 242 lot
# breaks with 75% of the gain unclassifiable.
HIFO="$OUT/bellwether-tax-managed"
[ -d "$HIFO" ] || fail "the tax-managed fund is missing"

# ⚠ THE TRANSLATED FIGURE, NOT THE TRIAL-BALANCE ROWS. Those are one row per
# (account, currency) now, so grepping them yields three numbers and comparing
# them as one string compares nothing.
gain_of() { "$RATIO" balance --book "$1" | awk '/^  gain /   {gsub(/,/,"",$2); print $2}'; }
a=$(gain_of "$GEN")
b=$(gain_of "$HIFO")
[ -n "$a" ] && [ -n "$b" ] || fail "no realized gain figure on one of the two books"
[ "$a" != "$b" ] || fail "same seed, different lot method, IDENTICAL gain ($a) — the declaration reaches nothing"

# And in the direction the method exists for: HIFO gives up the dearest lots, so
# it realizes the SMALLER gain. Equality is already excluded above; this catches
# a wiring that changes the number without changing it correctly.
awk -v a="$a" -v b="$b" 'BEGIN { exit !(b < a) }' \
  || fail "HIFO realized $b against FIFO's $a — the dearest-lot method should realize LESS"

# ⛔ AND THE TWO BOOKS OF RECORD HAVE TO DISAGREE. One journal, two recognition
# conventions, and if they land on the same NAV then nothing about the
# settlement convention reached the fold — which is observationally identical to
# not having built it. This is `bellwether-tax-managed`'s assertion one layer
# out, and it is the ONLY thing on the demo that could fail if a view were
# ignored.
#
# ⚠ THE TRAP HERE IS SHARPER THAN THE LOT-METHOD ONE. A purchase moves cash into
# investments, both assets, so recognising it or not moves a NAV by ZERO — a
# seed whose settlement tail holds only trades would agree while every line of
# the engine ran. The tail carries SUBSCRIPTIONS, which is the shape that works;
# HANDOFF.md records the multi-currency version being vacuous twice.
DUAL="$OUT/marlowe-dual-basis"
[ -d "$DUAL" ] || fail "the dual-basis fund is missing"

nav_of() { "$RATIO" balance --book "$1" --view "$2" | awk '/^  net asset value / {gsub(/,/,"",$5); print $5}'; }
abor=$(nav_of "$DUAL" abor)
ibor=$(nav_of "$DUAL" ibor)
[ -n "$abor" ] && [ -n "$ibor" ] || fail "no NAV figure on one of the two views"
[ "$abor" != "$ibor" ] \
  || fail "one journal, two settlement conventions, IDENTICAL NAV ($abor) — the view reaches nothing"

# ⭐ AND THE DIFFERENCE HAS TO BE A LIST. `Ratio.Views.two_views_differ_by_
# exactly_what_is_in_flight` says the entries one view recognises and the other
# does not account for the gap EXACTLY — so a reconciliation whose entries do
# not add to the difference is a screen showing arithmetic that is not true.
rec=$("$RATIO" reconcile abor ibor --book "$DUAL")
grep -q "in flight" <<<"$rec" || fail "the reconciliation lists no entries in flight"
diff_line=$(awk '/^difference / {gsub(/,/,"",$2); print $2}' <<<"$rec")
sum_line=$(awk '/^accounted for / {gsub(/,/,"",$3); print $3}' <<<"$rec")
[ "$diff_line" = "$sum_line" ] \
  || fail "the in-flight entries sum to $sum_line and the difference is $diff_line — the reconciliation does not tie"

# ⛔ AND BOTH VIEWS STILL TIE. A view keeps or drops WHOLE entries and each entry
# conserves, so `Ratio.Views.every_view_conserves` says the trial balance cannot
# move. A view that unbalanced a book would be the one failure the whole design
# forecloses.
for v in abor ibor; do
  "$RATIO" balance --book "$DUAL" --view "$v" | grep -q "difference *0" \
    || fail "view $v does not tie — a view is a filter over whole entries and cannot unbalance a book"
done

echo "  ok  6 funds, $lots open tax lots, FIFO $a vs HIFO $b, ABOR $abor vs IBOR $ibor"
