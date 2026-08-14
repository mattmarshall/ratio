#!/usr/bin/env bash
#
# Seed the demo with six funds, each in a state the console has to render.
#
# Each is a REAL book built through the same code path — same chart, same rules,
# same recon, same striking. What differs is what HAPPENED to it, and the state
# follows from that: a fund is blocked because it holds a blocking break, not
# because a fixture wrote "blocked" somewhere. That distinction is the whole
# reason to seed rather than to mock.
set -euo pipefail

RATIO="${1:?usage: seed-demo-funds.sh <ratio-binary> <out-dir>}"
OUT="${2:?usage: seed-demo-funds.sh <ratio-binary> <out-dir>}"
RATIO="$(cd "$(dirname "$RATIO")" && pwd)/$(basename "$RATIO")"
HERE="$(cd "$(dirname "$0")" && pwd)"

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
rm -rf "${OUT:?}"/*
export RATIO_ACTOR="${RATIO_ACTOR:-e.marsh}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# The figures the five seeded transactions actually produce. A period that
# AGREES has to agree with this, and getting it wrong shows up as a break rather
# than as a broken script — which is how the first version of this file shipped
# a "struck" fund that was blocked.
cat > "$WORK/agree.csv" <<'CSV'
account,amount
Investments at fair value,350000.00
Cash and equivalents,-332880.00
Dividend income,-2120.00
Realized gain on investments,-15000.00
CSV

# ── 1. Blocked: a 2,000.00 break nobody has explained ─────────────────────
# This one carries the data-plane gap as well as the break: an instrument the
# master does not know, so a fact pends and the NAV is blocked for two
# independent reasons — which is what a real morning looks like.
#
# ⛔ AND IT IS NOT STRUCK, WHICH IS THE POINT OF THE FUND. It used to be: the
# seeder declared this book blocked and then took a NAV on it, which is the
# contradiction seed-demo-book.sh's own comment apologises for one file over.
# `ratio strike` now refuses a blocked fund, so striking here would fail the
# script — and the honest demo is better anyway. A fund that says BLOCKED and
# has no NAV is the product working; a fund that says BLOCKED and shows one is
# a screen nobody should believe.
LEAVE_ONE_PENDING=1 "$HERE/seed-demo-book.sh" "$RATIO" "$OUT/harbourline-global-value" >/dev/null

# ── 2. Struck: the same book, reconciled against figures that agree ────────
B="$OUT/northstar-multi-strategy"
"$HERE/seed-demo-book.sh" "$RATIO" "$B" "$WORK/agree.csv" >/dev/null
"$RATIO" strike --book "$B" >/dev/null

# ── 3. Awaiting prices: a chart and a configuration, no entries ────────────
#    Which is every fund at nine in the morning.
"$RATIO" init --book "$OUT/calderwood-income" >/dev/null

# ── 4. A fund with twenty years of trading behind it ───────────────────────
#
# ⛔ THE ONE THAT EXERCISES THE ENGINE, and until now nothing did. The three
# books above are reconciliation books: a dozen entries each, three open tax
# lots between them, no chart roles, one currency. Every figure the lot engine
# produces — the method in force, the realized gain, its split by holding
# period, the multi-currency NAV, the lots behind a position — was built,
# deployed, and invisible on the demo, because no book on it had any lots.
#
# ⚠ AND THE SCALE ARGUMENT WAS UNSHOWABLE FOR THE SAME REASON. "A NAV does not
# read the tax lots" is a claim about a fund with a lot of them.
#
# The dials are the ones `ratio bench` measures, so the fund on the screen and
# the fund in the benchmark are the same fund. ⚠ Kept modest deliberately: the
# journal is copied into a Lambda's /tmp on every cold start, and /tmp is 512 MB.
SHAPE="--securities 20 --lots-per 40 --currencies 3 --seed 1"

"$RATIO" gen --book "$OUT/ashcombe-global-equity" $SHAPE >/dev/null
"$RATIO" strike --book "$OUT/ashcombe-global-equity" >/dev/null

# ── 5. The same fund, administered under a different agreement ─────────────
#
# ⛔ THE ONE FIGURE THE DEMO COULD NOT SHOW. Every fund above declared
# `oldest-first`, so nothing on any screen demonstrated that the declared method
# reaches the engine — the exact invisibility the wiring fix was made to end,
# reproduced one layer out. A method that changes nothing observable is
# indistinguishable from a method nobody reads.
#
# ⭐ SAME SEED, SAME SECURITIES, SAME TRADES, SAME HOLDINGS. The two books differ
# in one line of configuration and roughly ten million dollars of taxable gain.
# `Ratio.Lots.Relief.the_method_changes_the_gain` is the theorem; this is the
# screen.
#
# ⚠ HIFO IS THE INTERESTING SIDE, not the flattering one: it gives up the
# dearest lots, so it realizes the SMALLEST gain — a loss, on this book — which
# is why a fund harvesting losses asks for it.
"$RATIO" gen --book "$OUT/bellwether-tax-managed" $SHAPE --method hifo >/dev/null
"$RATIO" strike --book "$OUT/bellwether-tax-managed" >/dev/null

# ── 6. The same break as fund 1, and somebody explained it ─────────────────
#
# ⭐ THE ONLY PLACE THE CLEARED GATE IS VISIBLE. Fund 1 has a 2,000.00 break and
# cannot strike; this book has the SAME 2,000.00 break, a person's note against
# it, and a NAV. One act separates them, which is the shape seed_test.sh already
# uses for the lot method: two books from one seeder differing in one line.
#
# ⚠ AND THE BREAK IS STILL THERE. It is explained, not cleared — same URL, same
# figures, same place in the queue, with a name and a reason against it. A demo
# where accepting an explanation made the exception disappear would be showing
# the one behaviour this product refuses.
EXPLAIN_THE_BREAK=1 "$HERE/seed-demo-book.sh" "$RATIO" "$OUT/pennington-select-income" >/dev/null
RATIO_ACTOR=e.marsh "$RATIO" strike --book "$OUT/pennington-select-income" >/dev/null

echo "seeded 6 funds at $OUT"
