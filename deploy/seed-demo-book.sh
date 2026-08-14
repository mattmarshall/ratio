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
per_instrument = true
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
per_instrument = true

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

# Marking to market. A POSTING, not an assignment: the position moves by the
# difference between what the book holds it at and what it is worth, and the
# contra is unrealized gain. The amount is not in the file — a valuation
# computes it, because only it knows the carrying value.
[[rule]]
id = "mark_to_market"
kind = "mark"
description = "Revalue a position, with the movement to unrealized gain"
[[rule.posting]]
account = 1
weight = 1
per_instrument = true
[[rule.posting]]
account = 21
weight = -1

# ── how big a difference has to be before it stops the NAV ────────────────
#
# In the same configuration, and for the same reason: a fund's tolerance is a
# term of its administration agreement, not a property of the software, and a
# break cites the digest it was graded under. DECLARED rather than left out, so
# the console reports a term somebody agreed to instead of the numbers a book
# gets by custom — the distinction `lot_method` already keeps.
#
# ⚠ 100000 IS LOAD-BEARING FOR THE DEMO. positions.csv below is deliberately
# 2,000.00 light, which is 200000 minor units; raising this above that would
# leave the blocked fund unblocked and the whole story without its exception.
[tolerance]
below_notice = 500
blocks_nav = 100000

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

  # How an admitted fact reaches the journal. The template says WHICH rule
  # applies — this counterparty's `B` means a purchase — and the rule, approved
  # separately, decides what that does to the books.
  [template.fact.posts]
  by = "side"
  amount = "consideration"
  rules = { buy = "equity_purchase", sell = "disposal_proceeds" }

  # Prices. REFERENCE DATA: no `posts` block, so these are recorded, resolved
  # and citable, and never touch the books until a valuation uses them.
  [[template]]
  id = "vendor_eod_prices"
  reads = "csv"

    [[template.entity]]
    name = "instrument"
    kind = "instrument"
    absent = "pend"
    by = [
      { attribute = "isin", column = "ISIN" },
      { attribute = "ticker", column = "Ticker", within = { attribute = "exchange", column = "Exchange" } },
    ]

    [template.fact]
    kind = "price"
    reference = "PriceRef"
    entities = { instrument = "instrument" }

    [[template.fact.value]]
    field = "asOf"
    as = "date"
    column = "ValuationDate"
    format = "DD/MM/YYYY"

    [[template.fact.value]]
    field = "price"
    as = "money"
    column = "Price"
    currency = "Currency"
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

# ── and, on a book that asks for it, somebody explains the break ──────────
#
# ⚠ THE CALLER'S CHOICE, for the same reason `LEAVE_ONE_PENDING` is. A fund
# where every break is explained and one where none is are two different
# stories, and a seeder that told only one of them would leave the console with
# no way to show what accepting an explanation does. Chosen per fund in
# seed-demo-funds.sh.
#
# ⛔ The break is EXPLAINED, NOT CLEARED. It keeps its URL, its figures and its
# place in the queue, with a name against it — which is the whole distinction
# `ratio accept` exists to draw.
if [ -n "${EXPLAIN_THE_BREAK:-}" ]; then
  RATIO_ACTOR="${RATIO_ACTOR:-e.marsh}" "$RATIO" accept 1 \
    --because "Custodian has not settled the 26 Feb dividend; it clears T+2 and \
was confirmed by their operations desk on the phone." \
    --book "$OUT" >/dev/null
fi

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

# End-of-day prices from the vendor. Deliberately DD/MM/YYYY and keyed on
# ticker+exchange, so the identity ladder has to fall through the ISIN rung it
# tries first — which is what a ladder is for.
cat > eod-prices.csv <<'CSV'
PriceRef,ValuationDate,ISIN,Ticker,Exchange,Price,Currency
P-0001,26/02/2026,,VTI,ARCX,262.50,USD
P-0002,26/02/2026,,VOO,ARCX,441.75,USD
CSV
"$RATIO" ingest eod-prices.csv --template vendor_eod_prices --book "$OUT" >/dev/null

# Post what resolves, then value it.
#
# ⚠ Without these the demo has facts and no POSITIONS — every row on the
# positions screen reads "Not attributed", and marking has nothing to mark.
# Reading a file and never admitting it is a real mode (a shadow run), but it
# is not the one a first look should land on.
"$RATIO" admit --book "$OUT" >/dev/null
"$RATIO" mark --as-of 2026-02-26 --book "$OUT" >/dev/null

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
# ⚠ NOT an exact entry count. It used to assert "7 entrie(s)", and admitting
# facts and marking positions changed it — correctly. An assertion that has to
# be edited every time the demo gains a step is one that will eventually be
# edited without being read. These are the properties that actually matter.
grep -q "^Difference  *0.00  *0.00" <<<"$BAL" || { echo "book does not tie:"; echo "$BAL"; exit 1; }
ENTRIES="$(grep -c . "$OUT/journal.jsonl")"
[ "$ENTRIES" -ge 7 ] || { echo "expected at least 7 entries, got $ENTRIES"; exit 1; }
# A position, so the positions screen is not five rows of "Not attributed".
grep -q '"instrument"' "$OUT/journal.jsonl" \
  || { echo "no posting carries an instrument, so there are no positions"; exit 1; }
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
