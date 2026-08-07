#!/usr/bin/env bash
#
# The shadow run, end to end. PLAN.md Stage 3.
#
# The unit tests in `ratio-recon` cover the gate and the comparison. This
# covers what they cannot: the three exit codes, that `--post` writes nothing
# for a refused file, and that the advice printed in a report names commands
# that exist. Those are the parts that break by drifting rather than by
# computing the wrong answer.
#
# Usage:  demo/shadow-run.sh path/to/ratio
set -euo pipefail

RATIO="${1:?usage: shadow-run.sh path/to/ratio-binary}"
[ -x "$RATIO" ] || { echo "not executable: $RATIO" >&2; exit 1; }
RATIO="$(cd "$(dirname "$RATIO")" && pwd)/$(basename "$RATIO")"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

pass() { printf '  ok  %s\n' "$1"; }
fail() { printf '  XX  %s\n' "$1" >&2; exit 1; }
has()  { grep -qF -- "$2" <<<"$1" || fail "$3"; }

# `set -e` would kill the script on a non-zero exit that is the point of the
# check, so run and capture instead.
code() { set +e; "$@" >/dev/null 2>&1; local c=$?; set -e; echo "$c"; }

# First line of a command's output, without closing a pipe on it. Piping into
# `head` would kill the producer with SIGPIPE, which `pipefail` then reports as
# a failed pipeline — the exact hazard the last check in this script is about.
first_line() { local out; out="$("$@")"; printf '%s\n' "${out%%$'\n'*}"; }

"$RATIO" init --book book >/dev/null

echo "== a configuration a human approved =="
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
[[rule.posting]]
account = 2
weight = 1
[[rule.posting]]
account = 30
weight = -1
TOML
"$RATIO" config set rules.toml --book book >/dev/null
pass "four rules active"

# A quarter of a small single-currency equity fund.
#   investments  250000 + 180000 - 80000        =  350000
#   cash        -250000 - 180000 + 1240 + 95000 + 880 = -332880
#   dividends    1240 + 880                     =    2120  (credit)
#   realized     95000 - 80000                  =   15000  (credit)
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
Investments at fair value,350000.00
Cash and equivalents,-332880.00
Dividend income,-2120.00
Realized gain on investments,-15000.00
CSV

echo "== the period reconciles =="
OUT="$("$RATIO" recon txns.csv positions.csv --book book || true)"
has "$OUT" "RECONCILED — no differences" "a period that agrees should reconcile"
has "$OUT" "book ties  yes"              "the replayed book must tie"
has "$OUT" "excludes"                    "a clean report must still state its scope"
[ "$(code "$RATIO" recon txns.csv positions.csv --book book)" = 0 ] \
  || fail "a clean reconciliation should exit 0"
pass "zero differences, exit 0, scope stated"

echo "== a disagreement is reported, not smoothed over =="
sed 's/350000.00/348000.00/' positions.csv > wrong.csv
OUT="$("$RATIO" recon txns.csv wrong.csv --book book || true)"
has "$OUT" "BREAKS"                    "a difference should be reported"
has "$OUT" "2000.00"                   "the difference should be stated exactly"
has "$OUT" "figures differ"            "the break should name its cause"
CONFIG="$(first_line "$RATIO" config show --book book | sed -n 's/^config *//p')"
has "$OUT" "$CONFIG"                   "the report must cite the config that produced its figures"
[ "$(code "$RATIO" recon txns.csv wrong.csv --book book)" = 2 ] \
  || fail "breaks should exit 2, distinct from both clean and refused"
pass "break reported with cause and config, exit 2"

echo "== a file it cannot model is refused whole =="
cat > messy.csv <<'CSV'
id,date,type,security,amount,currency,basis
t1,2026-01-02,buy,VTI,250000.00,USD,
t2,2026-01-08,buy,VOD,90000.00,GBP,
t3,2026-01-20,split,VTI,0.00,USD,
t4,2026-02-03,sell,VTI,95000.00,USD,
CSV
OUT="$("$RATIO" recon messy.csv positions.csv --book book || true)"
has "$OUT" "NOT RECONCILED"   "an unmodelable file must not read as a result"
# Every out-of-scope row is named in one pass, so the next file can be fixed
# once rather than one refusal at a time.
has "$OUT" "t2"               "the foreign-currency row should be named"
has "$OUT" "t3"               "the corporate action should be named"
has "$OUT" "t4"               "the disposal without a basis should be named"
grep -qF "BREAKS" <<<"$OUT" && fail "a refused file must report no breaks at all"
[ "$(code "$RATIO" recon messy.csv positions.csv --book book)" = 3 ] \
  || fail "a refusal should exit 3, distinct from breaks"
pass "three reasons named in one pass, no breaks, exit 3"

echo "== --post on a refused file writes nothing =="
BEFORE="$(first_line "$RATIO" balance --book book)"
"$RATIO" recon messy.csv positions.csv --book book --post >/dev/null 2>&1 || true
AFTER="$(first_line "$RATIO" balance --book book)"
[ "$BEFORE" = "$AFTER" ] || fail "a refused file reached the book: '$BEFORE' -> '$AFTER'"
has "$AFTER" "0 entrie(s)" "the book should still be empty"
pass "nothing from a refused file reaches the book"

echo "== --post on a clean run, and the advice it prints is true =="
OUT="$("$RATIO" recon txns.csv positions.csv --book book --post --out report.pb || true)"
has "$OUT" "posted 6 entrie(s)" "the disposal should contribute two of the six"
[ -s report.pb ] || fail "no proto artifact written"
# The report tells the reader to run this. It has to exist and it has to work.
EXPLAIN="$("$RATIO" explain "Realized gain on investments" --book book)"
has "$EXPLAIN" "t4-proceeds"  "the proceeds half should be readable back"
has "$EXPLAIN" "t4-basis"     "the basis half should be readable back"
has "$EXPLAIN" "-15000.00"    "the two halves should net to the realized gain"
pass "entries posted, artifact written, explain reads them back"

echo
echo "a real period reconciled to zero differences, and the one file it could"
echo "not model was refused whole rather than half-answered."
