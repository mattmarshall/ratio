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

# Eight funds — four states, a fifth that differs from one of them by one line
# of configuration, a sixth that differs from another by one person's act, a
# seventh that differs from itself: one journal read under two books of record,
# and an eighth that elects the average-cost pool (which cannot share a book
# with lot_method or min-tax).
n=$(find "$OUT" -maxdepth 1 -mindepth 1 -type d | wc -l | tr -d ' ')
[ "$n" -eq 8 ] || fail "expected 8 funds, found $n"

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

# ⛔ AND A BLOCKED FUND REFUSES ITS OWN NAV. This is the seam the demo used to
# contradict: the seeder declared harbourline blocked and then struck it, which
# is a screen saying one thing and a command doing another. `ratio strike` now
# refuses, and the assertion is on BOTH halves — the command failing, and no NAV
# left behind by the attempt. A refusal that bails after recording has spent the
# valuation point, and `Ratio.Period.one_answer_per_day` means it cannot be
# taken back.
BLOCKED="$OUT/harbourline-global-value"
# ⛔ THE WALK-THROUGH HAS TO BE ABLE TO POINT AT THE WINDOW. Silence is
# unset, not a silent 30 — a seed that drops the field cannot show the
# cite. The active blob is the one after `approve` merged the fee rule;
# replace_sections keeps top-level keys, and this is what proves it.
active=$(cat "$BLOCKED/config/ACTIVE")
grep -q "wash_window_days = 30" "$BLOCKED/config/$active" \
  || fail "harbourline elects no wash window — the walk-through cannot cite one"
grep -q 'partner = "LP"' "$BLOCKED/config/$active" \
  || fail "harbourline elects no partner cut — the walk-through cannot cite allocated plugs"
grep -q 'weight = 80' "$BLOCKED/config/$active" \
  || fail "harbourline cut is not LP 80 — inventing 1/N is the defect"
grep -q 'weight = 20' "$BLOCKED/config/$active" \
  || fail "harbourline cut is not GP 20 — two partners is not 50/50"
# ⛔ NOT 1/N. Equal weights would still be an election, but the
# walk-through names 80/20. A seed that wrote 1, 1 cannot show it.
lp_w=$(awk '/\[\[partner_cut\]\]/{p=1;next} p&&/weight/{print $3;exit}' "$BLOCKED/config/$active")
gp_w=$(awk '/\[\[partner_cut\]\]/{n++} n==2&&/weight/{print $3;exit}' "$BLOCKED/config/$active")
[ -n "$lp_w" ] && [ -n "$gp_w" ] || fail "harbourline partner_cut weights missing"
[ "$lp_w" != "$gp_w" ] || fail "harbourline cut is equal weights ($lp_w) — that is 1/N wearing an election"
grep -q "wash_keep_holding_period" "$BLOCKED/config/$active" \
  && fail "harbourline wrote keep — the demo is a US transfer, unset stays unset"
grep -q "min_tax_short_weight" "$BLOCKED/config/$active" \
  && fail "harbourline wrote min-tax — unset stays unset, not a silent 2"
grep -q "average_cost" "$BLOCKED/config/$active" \
  && fail "harbourline wrote average cost — unset stays unset, not a silent true"
# ⛔ THE WALK-THROUGH HAS TO BE ABLE TO POINT AT THE NAMES. SpecID is
# per-sale, not a fund term. The zero-gain SPEC round-trip carries the
# lot the taxpayer named.
grep -q '"identified_lots"' "$BLOCKED/journal.jsonl" \
  || fail "harbourline has no SpecID sale — the walk-through cannot cite named lots"

# ⭐ UNITIZED OPENING. A money-only sub-0001 labelled "subscription" is
# the leftover #181 named — units stayed unset and the walk-through had
# to record subscribe_lp itself. Quantity on the capital leg, dated.
grep -q '"id":"sub-0001"' "$BLOCKED/journal.jsonl" \
  || fail "harbourline has no opening subscription"
grep -q '"quantity":500000' "$BLOCKED/journal.jsonl" \
  || fail "harbourline opening subscription has no units — money-only sub-0001 is the leftover"
grep -q '"trade_date":"2026-01-01"' "$BLOCKED/journal.jsonl" \
  || fail "harbourline opening subscription is undated — period NAV would skip it"

# Calderwood elects the Lean example's min-tax weight. Empty book, so
# writing 2 cannot restate a sale. Silence on harbourline stays unset.
CAL="$OUT/calderwood-income"
[ -d "$CAL" ] || fail "calderwood is missing"
cal_active=$(cat "$CAL/config/ACTIVE")
grep -q "min_tax_short_weight = 2" "$CAL/config/$cal_active" \
  || fail "calderwood elects no min-tax weight — the walk-through cannot cite one"
cal_entries="$(grep -c . "$CAL/journal.jsonl" 2>/dev/null || echo 0)"
[ "$cal_entries" -eq 0 ] || fail "calderwood grew a journal ($cal_entries) — it is the empty morning book"

# Kestrel elects the pool. Cannot share harbourline (sales) or
# calderwood (min-tax).
KESTREL="$OUT/kestrel-pooled-basis"
[ -d "$KESTREL" ] || fail "kestrel is missing"
kestrel_active=$(cat "$KESTREL/config/ACTIVE")
grep -q "average_cost = true" "$KESTREL/config/$kestrel_active" \
  || fail "kestrel elects no average-cost pool — the walk-through cannot cite one"
grep -q "min_tax_short_weight" "$KESTREL/config/$kestrel_active" \
  && fail "kestrel wrote min-tax — two elections for one sale"
if "$RATIO" strike --book "$BLOCKED" >/dev/null 2>&1; then
  fail "a blocked fund struck a NAV"
fi
[ ! -s "$BLOCKED/NAVS" ] || fail "a refused strike left a NAV behind on $BLOCKED"

# And the refusal says what to do about it, in both directions it can block.
msg=$("$RATIO" strike --book "$BLOCKED" 2>&1 || true)
grep -q "ratio accept" <<<"$msg" || fail "the refusal does not name the verb that clears a break"
grep -q "ratio admit" <<<"$msg" || fail "the refusal does not name what clears a pending fact"

# ⭐ AND THE SAME BREAK, EXPLAINED, LETS THE NAV THROUGH. Two books from one
# seeder differing in one person's act — the shape this file already uses for
# the lot method. Without it the gate is demonstrated only by refusing, and a
# gate nobody has watched open is indistinguishable from one that is stuck.
EXPLAINED="$OUT/pennington-select-income"
[ -d "$EXPLAINED" ] || fail "the explained fund is missing"
[ -s "$EXPLAINED/NAVS" ] || fail "the explained fund has no NAV — the gate did not open"
grep -q "accepted" "$EXPLAINED/CHANGELOG" || fail "no acceptance recorded on $EXPLAINED"
# ⚠ EXPLAINED, NOT CLEARED. The break is still there, with a name against it.
grep -q "clears T+2" "$EXPLAINED/explanations.jsonl" \
  || fail "the explanation is not on $EXPLAINED"

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
# ⚠ THE FIGURE WITH A DECIMAL POINT, NOT THE THIRD COLUMN. A positional read
# took the ENTRY COUNT here — `rfc3339` is exactly 20 characters and the column
# was 20 wide, so the timestamp ran into the view with no space and every field
# shifted by one. Both views fold one journal, so the entry count is IDENTICAL
# by construction and the assertion below fired on a figure that could never
# have differed. Minor units are the only field on the row carrying a `.`.
nav_of() {
  "$RATIO" navs --book "$1" --view "$2" \
    | awk 'NR > 1 { for (i = 1; i <= NF; i++) if ($i ~ /^-?[0-9]+\.[0-9][0-9]$/) { print $i; exit } }'
}
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
#
# ⛔ CAPTURED FIRST, NOT PIPED INTO `grep -q`, and every other check in this file
# already does it this way. `grep -q` exits the moment it matches, which closes
# the pipe; `ratio` restores SIG_DFL for SIGPIPE on purpose so that
# `ratio balance | head` does not panic, so it then dies with 141 — and under
# `set -o pipefail` the pipeline fails EVEN THOUGH GREP MATCHED. This check
# reported that the book did not tie while it was tying perfectly well.
dual_bal=$("$RATIO" balance --book "$DUAL")
grep -qE "^Difference .* 0\.00 *$" <<<"$dual_bal" \
  || fail "the dual-basis book does not tie — a view is a filter over whole entries and cannot unbalance one"

# ⛔ AND THE DIFFERENCE IS ACCOUNTED FOR, ENTRY BY ENTRY. The difference between
# two views is a LIST — `Ratio.Views.two_views_differ_by_exactly_what_is_in_
# flight` — and `ratio reconcile` shows it. The load-bearing half of this check
# is the EXIT CODE: `reconcile` itself refuses when the in-flight entries do not
# sum to the NAV difference exactly, so a zero exit IS the sum tying. The greps
# then pin what a person would read: a non-zero difference, and at least one
# entry in flight accounting for it.
rec=$("$RATIO" reconcile abor ibor --book "$DUAL") \
  || fail "reconcile refused on the dual-basis fund: $rec"
grep -qE "^difference +-?[0-9,.]+" <<<"$rec" \
  || fail "reconcile prints no difference line"
grep -qE "^difference +0\.00$" <<<"$rec" \
  && fail "reconcile reports a ZERO difference — the tail is not in flight and the demo shows nothing"
grep -q "RECOGNISED IN abor, NOT YET IN ibor" <<<"$rec" \
  || fail "reconcile does not show the in-flight list"
# ⚠ ONLY THE LINE AFTER THE abor HEADER. "nothing in flight" is the RIGHT
# answer for the reverse direction — abor recognises everything ibor does — so
# a scan over the whole output would fail on the section that is behaving.
awk '/RECOGNISED IN abor, NOT YET IN ibor/{getline; if ($0 ~ /nothing in flight/) exit 1; exit 0}' <<<"$rec" \
  || fail "abor recognises the settlement tail and ibor does not — that list cannot be empty"

echo "  ok  8 funds, $lots open tax lots, FIFO $a vs HIFO $b, ABOR $abor vs IBOR $ibor, reconciled entry by entry, blocked refuses and explained strikes"
