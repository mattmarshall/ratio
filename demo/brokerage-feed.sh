#!/usr/bin/env bash
#
# Personal brokerage statement ingest. Issue #167.
#
# The unit tests in ratio-console cover CreateBook → ingest → admit → recon.
# This covers what they cannot: the CLI verbs a walk-through actually types,
# the three exit codes on `--from-ingest`, and that the journal is not rewritten.
# It does not call `ratio strike` — a household Investments account is a
# transfer, not a fund NAV gate.
#
# Usage:  demo/brokerage-feed.sh path/to/ratio
set -euo pipefail

RATIO="${1:?usage: brokerage-feed.sh path/to/ratio-binary}"
[ -x "$RATIO" ] || { echo "not executable: $RATIO" >&2; exit 1; }
RATIO="$(cd "$(dirname "$RATIO")" && pwd)/$(basename "$RATIO")"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

pass() { printf '  ok  %s\n' "$1"; }
fail() { printf '  XX  %s\n' "$1" >&2; exit 1; }
has()  { grep -qF -- "$2" <<<"$1" || fail "$3"; }
code() { set +e; "$@" >/dev/null 2>&1; local c=$?; set -e; echo "$c"; }

echo "== a CreateBook Personal book, empty journal =="
"$RATIO" init --kind personal --book book >/dev/null
BAL="$("$RATIO" balance --book book)"
has "$BAL" "0 entrie(s)" "CreateBook must not invent journal history"
pass "empty journal, personal templates in force"

echo "== the master the statement will resolve against =="
"$RATIO" entity add --kind counterparty --id cp-prime --name "Household Brokerage" \
  --attr code=PRME --book book >/dev/null
"$RATIO" entity add --kind instrument --id inst-vti --name "Vanguard Total Stock Market ETF" \
  --attr isin=US9229087690 --attr ticker=VTI --attr exchange=ARCX --book book >/dev/null
"$RATIO" entity add --kind instrument --id inst-voo --name "Vanguard S&P 500 ETF" \
  --attr isin=US9229083632 --attr ticker=VOO --attr exchange=ARCX --book book >/dev/null
pass "broker, VTI, VOO"

cat > trades.csv <<'CSV'
TradeRef,ISIN,Symbol,Exch,Broker,B/S,Quantity,Price,Ccy,TradeDate
PB-0041,US9229087690,VTI,ARCX,PRME,B,1000,250.00,USD,02/24/2026
PB-0042,,VOO,ARCX,PRME,B,400,450.00,USD,02/25/2026
CSV
cat > positions.csv <<'CSV'
LineRef,AsOf,ISIN,Ticker,Exch,Quantity,MarketValue,Ccy
P-1,2026-02-26,US9229087690,VTI,ARCX,1000,262500.00,USD
P-2,2026-02-26,,VOO,ARCX,400,176700.00,USD
CSV

echo "== ingest the statement, admit the trades =="
"$RATIO" ingest trades.csv --template brokerage-statement --book book >/dev/null
"$RATIO" ingest positions.csv --template brokerage-positions --book book >/dev/null
OUT="$("$RATIO" admit --book book)"
has "$OUT" "posted     2" "the two USD buys must post"
has "$OUT" "recorded   2" "the holdings snapshot records and never posts"
pass "two transfers in the journal, two holdings recorded"

BEFORE="$(grep -c . book/journal.jsonl)"

echo "== live recon against the ingested holdings =="
OUT="$("$RATIO" recon --from-ingest --book book || true)"
has "$OUT" "BREAKS" "statement MV vs book cost must be a break"
has "$OUT" "9200.00" "the difference is 9,200.00"
has "$OUT" "Investments" "the difference names the household Investments account"
has "$OUT" "posts nothing" "the live path must not claim to write the journal"
has "$OUT" "brokerage-positions-live" "the scope must say it is the household path"
grep -qF "ratio strike" <<<"$OUT" && fail "a household recon must not pretend a NAV gate"
[ "$(code "$RATIO" recon --from-ingest --book book)" = 2 ] \
  || fail "a live break should exit 2, distinct from clean and refused"
AFTER="$(grep -c . book/journal.jsonl)"
[ "$BEFORE" = "$AFTER" ] || fail "live recon rewrote the journal: $BEFORE -> $AFTER"
[ -n "$(ls -A book/reports 2>/dev/null)" ] || fail "no break report stored"
pass "break stored, journal unchanged, exit 2"

echo "== an unidentified holding refuses the whole run =="
cat > messy.csv <<'CSV'
LineRef,AsOf,ISIN,Ticker,Exch,Quantity,MarketValue,Ccy
P-1,2026-02-26,US9229087690,VTI,ARCX,1000,262500.00,USD
P-9,2026-02-26,US0000000000,UNKN,XNAS,10,1000.00,USD
CSV
"$RATIO" init --kind personal --book other >/dev/null
"$RATIO" entity add --kind counterparty --id cp-prime --name "Household Brokerage" \
  --attr code=PRME --book other >/dev/null
"$RATIO" entity add --kind instrument --id inst-vti --name "VTI" \
  --attr isin=US9229087690 --attr ticker=VTI --attr exchange=ARCX --book other >/dev/null
"$RATIO" ingest messy.csv --template brokerage-positions --book other >/dev/null
OUT="$("$RATIO" recon --from-ingest --book other || true)"
has "$OUT" "NOT RECONCILED" "an unidentified holding must refuse the run"
has "$OUT" "P-9" "the unmatched line must be named"
grep -qF "BREAKS" <<<"$OUT" && fail "a refused live run must report no breaks"
[ "$(code "$RATIO" recon --from-ingest --book other)" = 3 ] \
  || fail "a refusal should exit 3, distinct from breaks"
[ -z "$(ls -A other/reports 2>/dev/null)" ] || fail "a refused run stored a report"
pass "unidentified holding refused whole, exit 3, no report"

echo "== a foreign-currency holding refuses the whole run =="
"$RATIO" init --kind personal --book fx >/dev/null
"$RATIO" entity add --kind instrument --id inst-vwrl --name "VWRL" \
  --attr isin=IE00B3RBWM25 --attr ticker=VWRL --attr exchange=XAMS --book fx >/dev/null
cat > euro.csv <<'CSV'
LineRef,AsOf,ISIN,Ticker,Exch,Quantity,MarketValue,Ccy
P-1,2026-02-26,IE00B3RBWM25,VWRL,XAMS,250,28100.00,EUR
CSV
"$RATIO" ingest euro.csv --template brokerage-positions --book fx >/dev/null
OUT="$("$RATIO" recon --from-ingest --book fx || true)"
has "$OUT" "NOT RECONCILED" "a foreign-currency holding must refuse the run"
has "$OUT" "EUR" "the refusal must name the currency"
grep -qF "BREAKS" <<<"$OUT" && fail "a refused FX run must report no breaks"
[ "$(code "$RATIO" recon --from-ingest --book fx)" = 3 ] \
  || fail "an FX refusal should exit 3"
[ -z "$(ls -A fx/reports 2>/dev/null)" ] || fail "an FX refusal stored a report"
pass "foreign-currency holding refused whole, exit 3, no report"

echo
echo "a walk-through ingested a brokerage file, admitted the transfers, and"
echo "live recon saw the Investments difference — without inventing journal"
echo "history, opening a lot, or striking a NAV."
