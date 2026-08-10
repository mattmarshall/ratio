#!/usr/bin/env bash
#
# Seed the demo with three funds in three different states.
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
LEAVE_ONE_PENDING=1 "$HERE/seed-demo-book.sh" "$RATIO" "$OUT/harbourline-global-value" >/dev/null
"$RATIO" strike --book "$OUT/harbourline-global-value" >/dev/null

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
"$RATIO" gen --book "$OUT/ashcombe-global-equity" \
  --securities 20 --lots-per 40 --currencies 3 >/dev/null
"$RATIO" strike --book "$OUT/ashcombe-global-equity" >/dev/null

echo "seeded 4 funds at $OUT"
