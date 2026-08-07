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
"$HERE/seed-demo-book.sh" "$RATIO" "$OUT/harbourline-global-value" >/dev/null
"$RATIO" strike --book "$OUT/harbourline-global-value" >/dev/null

# ── 2. Struck: the same book, reconciled against figures that agree ────────
B="$OUT/northstar-multi-strategy"
"$HERE/seed-demo-book.sh" "$RATIO" "$B" "$WORK/agree.csv" >/dev/null
"$RATIO" strike --book "$B" >/dev/null

# ── 3. Awaiting prices: a chart and a configuration, no entries ────────────
#    Which is every fund at nine in the morning.
"$RATIO" init --book "$OUT/calderwood-income" >/dev/null

echo "seeded 3 funds at $OUT"
