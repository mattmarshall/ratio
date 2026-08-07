#!/usr/bin/env bash
#
# Build the demo book the deployed image serves.
#
# Generated, never committed. A book is a directory of journal lines, config
# blobs and reports; checked in as bytes it would rot silently the first time a
# format changed, and nobody reviews a binary diff. Rebuilding it from the CLI
# on every image build means the demo can only contain states the product can
# actually produce.
#
# Usage:  deploy/seed-demo-book.sh path/to/ratio path/to/output-book
set -euo pipefail

RATIO="${1:?usage: seed-demo-book.sh <ratio-binary> <out-dir>}"
OUT="${2:?usage: seed-demo-book.sh <ratio-binary> <out-dir>}"

# Resolve BOTH paths before the `cd` below, and resolve them absolutely.
#
# This script changes directory into a scratch dir to keep its intermediate
# CSVs out of the caller's tree. A relative $OUT then resolves *inside* that
# scratch dir — so the book was built somewhere the trap deleted, the caller
# was left with the empty directory `mkdir -p` had made, and the script's own
# assertions passed because they checked the same relative path. It reported
# success and shipped nothing. Local runs did not catch it because they were
# given an absolute path; CI passed `deploy/demo-book`.
RATIO="$(cd "$(dirname "$RATIO")" && pwd)/$(basename "$RATIO")"
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
rm -rf "${OUT:?}"/*

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

"$RATIO" init --book "$OUT" >/dev/null

# The configuration a person approved: a long-only single-currency equity fund.
cat > rules.toml <<'TOML'
[[rule]]
id = "equity_purchase"
kind = "trade"
description = "Buy: investments up, cash down"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1

[[rule]]
id = "disposal_proceeds"
kind = "trade"
description = "Sale, proceeds half: cash in against realized gain"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 31
weight = -1

[[rule]]
id = "disposal_basis"
kind = "trade"
description = "Sale, basis half: relieve the investment at cost"
[[rule.posting]]
account = 31
weight = 1
[[rule.posting]]
account = 1
weight = -1

[[rule]]
id = "cash_dividend"
kind = "dividend"
description = "Cash dividend received"
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 30
weight = -1
TOML
"$RATIO" config set rules.toml --book "$OUT" >/dev/null

# The fund takes capital before it buys anything.
#
# Without this the seeded fund has negative cash and a NAV of 17,120 against
# 350,000 of investments — arithmetically correct and a terrible advertisement.
# An opening subscription is posted directly rather than through a rule: a
# fund's opening balance is not one of the transaction types the recon scope
# covers, and pretending it were would widen the scope to flatter the demo.
cat > opening.json <<'JSON'
[{"id":"sub-0001","memo":"Opening subscription","postings":[
  {"dim":2,"amount":50000000000},
  {"dim":20,"amount":-50000000000}]}]
JSON
"$RATIO" post opening.json --book "$OUT" >/dev/null

# A quarter of transactions, and the positions the incumbent reported. The
# investments figure is deliberately 2,000 light so the break screen has a real
# break on it — an empty break report demos nothing.
cat > txns.csv <<'CSV'
id,date,type,security,amount,currency,basis
t1,2026-01-02,buy,VTI,250000.00,USD,
t2,2026-01-08,buy,VOO,180000.00,USD,
t3,2026-01-15,dividend,VTI,1240.00,USD,
t4,2026-02-03,sell,VTI,95000.00,USD,80000.00
t5,2026-02-20,dividend,VOO,880.00,USD,
CSV
cat > positions.csv <<'CSV'
account,amount
Investments at fair value,348000.00
Cash and equivalents,-332880.00
Dividend income,-2120.00
Realized gain on investments,-15000.00
CSV
# `|| true` because a run that finds breaks exits 2 by design, and that is the
# run we want.
"$RATIO" recon txns.csv positions.csv --book "$OUT" --post >/dev/null || true

# A proposal nobody has approved, so the rules screen shows both columns — the
# gap between them is what the demo is about.
mkdir -p "$OUT/proposals"
cat > "$OUT/proposals/performance_fee.toml" <<'TOML'
[[rule]]
id = "performance_fee"
kind = "accrual"
description = "Performance fee, 20% over a 6% hurdle"
rate_bp = 2000
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1
TOML

# Assert the book is in the state the demo needs, rather than trusting that it
# is. A demo image that boots to an empty screen is found by a customer.
BAL="$("$RATIO" balance --book "$OUT")"
grep -q "7 entrie(s)" <<<"$BAL" || { echo "expected 7 entries:"; echo "$BAL"; exit 1; }
grep -q "^Difference  *0.00  *0.00" <<<"$BAL" || { echo "book does not tie:"; echo "$BAL"; exit 1; }
[ -n "$(ls -A "$OUT/reports" 2>/dev/null)" ] || { echo "no break report stored"; exit 1; }
[ -f "$OUT/proposals/performance_fee.toml" ] || { echo "no pending proposal"; exit 1; }

# Everything above checked $OUT. Check it once more the way the caller will —
# by absolute path, after the cd — so "the script said it worked" and "there is
# a book where I asked for one" cannot come apart again.
[ -s "$OUT/accounts.json" ] || { echo "no accounts.json at $OUT"; exit 1; }
[ -s "$OUT/journal.jsonl" ] || { echo "no journal at $OUT"; exit 1; }

echo "demo book ready at $OUT"
echo "  $(grep -c . "$OUT"/journal* 2>/dev/null || echo 6) journal line(s)"
echo "  $(ls "$OUT/reports" | wc -l | tr -d ' ') report(s), $(ls "$OUT/proposals" | wc -l | tr -d ' ') proposal(s)"
