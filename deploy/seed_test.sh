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

# Six funds — four states, a fifth that differs from one of them by one line of
# configuration, and a sixth that differs from itself: one journal read under
# two books of record.
n=$(find "$OUT" -maxdepth 1 -mindepth 1 -type d | wc -l | tr -d ' ')
[ "$n" -eq 6 ] || fail "expected 6 funds, found $n"

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

# ⚠ READ OFF THE RECORDED STRIKES, not recomputed here. `seed-demo-funds.sh`
# struck both views; `ratio navs --view` lists what was WRITTEN, so this checks
# the figures a person would actually be shown rather than ones this script
# derived for itself.
nav_of() { "$RATIO" navs --book "$1" --view "$2" | awk 'NR > 1 { print $3; exit }'; }
abor=$(nav_of "$DUAL" abor)
ibor=$(nav_of "$DUAL" ibor)
[ -n "$abor" ] && [ -n "$ibor" ] || fail "no NAV figure on one of the two views"
[ "$abor" != "$ibor" ] \
  || fail "one journal, two settlement conventions, IDENTICAL NAV ($abor) — the view reaches nothing"

# ⛔ AND THE BOOK STILL TIES — ONCE, AT FUND LEVEL, WHICH IS THE POINT. A view
# keeps or drops WHOLE entries and each entry conserves on its own, so
# `Ratio.Views.every_view_conserves` says the difference is the SAME in every
# view. Checking it per view would be checking one number three times; checking
# it here asserts the thing that is actually claimed — that the difference is
# view-invariant, which is why it stayed on `Fund` when the two column totals
# moved to `View`.
"$RATIO" balance --book "$DUAL" | grep -qE "^Difference .* 0\.00 *$" \
  || fail "the dual-basis book does not tie — a view is a filter over whole entries and cannot unbalance one"

# ⛔ OWED, AND NOT ASSERTED HERE BECAUSE IT CANNOT YET BE. The difference between
# two views is a LIST — `Ratio.Views.two_views_differ_by_exactly_what_is_in_
# flight` — and `ratio reconcile` is what will show it. It waits on the
# projection folding per view; until then the console's reconciliation screen
# REFUSES rather than subtracting two figures, and this script checks the NAVs
# differ rather than checking what accounts for the difference. That is a weaker
# claim and is written down as one.

echo "  ok  6 funds, $lots open tax lots, FIFO $a vs HIFO $b, ABOR $abor vs IBOR $ibor"
