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
# Usage:  deploy/seed-demo-book.sh <ratio> <out-dir> [positions.csv]
#
# The third argument is the positions to reconcile against. It defaults to a set
# that disagrees by 2,000.00, because a break report with no break demonstrates
# nothing — pass agreeing positions to seed a fund that reconciles clean.
set -euo pipefail

RATIO="${1:?usage: seed-demo-book.sh <ratio-binary> <out-dir> [positions.csv]}"
OUT="${2:?usage: seed-demo-book.sh <ratio-binary> <out-dir> [positions.csv]}"
POSITIONS="${3:-}"
[ -n "$POSITIONS" ] && POSITIONS="$(cd "$(dirname "$POSITIONS")" && pwd)/$(basename "$POSITIONS")"

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

# ── the mapping, in the SAME configuration as the rules above ─────────────
#
# That is the claim, made real: one digest fixes how a file becomes an event
# and how an event becomes postings. A NAV cites it and the whole derivation is
# determined, from the broker's bytes onward.
[[template]]
id = "prime_equity_trades"
reads = "csv"

  [[template.entity]]
  name = "security"
  kind = "instrument"
  absent = "pend"
  by = [
    { attribute = "isin", column = "ISIN" },
    { attribute = "ticker", column = "Symbol", within = { attribute = "exchange", column = "Exch" } },
  ]

  [[template.entity]]
  name = "broker"
  kind = "counterparty"
  absent = "pend"
  by = [{ attribute = "code", column = "Broker" }]

  [template.fact]
  kind = "trade"
  reference = "TradeRef"
  entities = { security = "security", broker = "broker" }

  [[template.fact.value]]
  field = "side"
  as = "enum"
  column = "B/S"
  map = { B = "buy", S = "sell" }

  [[template.fact.value]]
  field = "quantity"
  as = "decimal"
  column = "Quantity"

  [[template.fact.value]]
  field = "price"
  as = "money"
  column = "Price"
  currency = "Ccy"

  [[template.fact.value]]
  field = "traded"
  as = "date"
  column = "TradeDate"
  format = "MM/DD/YYYY"
TOML
"$RATIO" config set rules.toml --book "$OUT" >/dev/null

# A second configuration, promoted the way a real one is: proposed as a file,
# approved by a named person, recorded in the CHANGELOG.
#
# `config set` above promotes directly and writes no approval line, which is
# right for a book's opening configuration — nobody approved it, it is where
# the book started. Everything after it goes through `approve`, so the console
# can say who put each rule in force. Without this the configuration panel
# reads "no recorded approver" for every version, which demonstrates the
# absence of the feature rather than the feature.
#
# It is approved BEFORE any entry is posted, so every entry in the book cites
# the configuration that is still active at the end. A rule promoted afterwards
# would leave the fund's active digest disagreeing with the digest on its own
# entries — true to life, but a different demo.
mkdir -p "$OUT/proposals"
cat > "$OUT/proposals/management_fee.toml" <<'TOML'
[[rule]]
id = "management_fee"
kind = "accrual"
description = "Management fee, 75bp per annum on net assets"
rate_bp = 75
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1
TOML
RATIO_ACTOR="${RATIO_ACTOR:-e.marsh}" "$RATIO" approve management_fee --book "$OUT" >/dev/null

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
# `|| true` because a run that finds breaks exits 2 by design, and for the
# default positions that is the run we want.
"$RATIO" recon txns.csv "${POSITIONS:-positions.csv}" --book "$OUT" --post >/dev/null || true

# ── the data plane, with a gap in the master on purpose ───────────────────
#
# Three trades arrive from the prime broker.
#
# ⚠ Whether the third instrument is in the master is the CALLER'S choice, and
# it has to be, because a pending fact BLOCKS the NAV. Giving every fund the
# gap made the reconciled fund blocked too — the smoke test caught it with "no
# fund in state STRUCK", which is the second time a seeder here has quietly
# produced a fund whose state contradicted the story it was seeded to tell.
"$RATIO" entity add --kind counterparty --id cp-prime --name "Prime Brokerage" \
  --attr code=PRME --book "$OUT" >/dev/null
"$RATIO" entity add --kind instrument --id inst-vti --name "Vanguard Total Stock Market ETF" \
  --attr isin=US9229087690 --attr ticker=VTI --attr exchange=ARCX --book "$OUT" >/dev/null
"$RATIO" entity add --kind instrument --id inst-voo --name "Vanguard S&P 500 ETF" \
  --attr isin=US9229083632 --attr ticker=VOO --attr exchange=ARCX --book "$OUT" >/dev/null

cat > prime-trades.csv <<'CSV'
TradeRef,ISIN,Symbol,Exch,Broker,B/S,Quantity,Price,Ccy,TradeDate
PB-0041,US9229087690,VTI,ARCX,PRME,B,1000,250.00,USD,02/24/2026
PB-0042,,VOO,ARCX,PRME,B,400,450.00,USD,02/25/2026
PB-0043,IE00B3RBWM25,VWRL,XAMS,PRME,B,250,112.40,EUR,02/26/2026
CSV
# The instrument the third trade needs. Present unless the caller wants the
# pending state on this book.
if [ -z "${LEAVE_ONE_PENDING:-}" ]; then
  "$RATIO" entity add --kind instrument --id inst-vwrl \
    --name "Vanguard FTSE All-World UCITS ETF" \
    --attr isin=IE00B3RBWM25 --attr ticker=VWRL --attr exchange=XAMS \
    --book "$OUT" >/dev/null
fi
"$RATIO" ingest prime-trades.csv --template prime_equity_trades --book "$OUT" >/dev/null

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
