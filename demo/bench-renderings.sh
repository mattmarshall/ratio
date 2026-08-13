#!/usr/bin/env bash
#
# `ratio bench` reports the same run two ways, and this checks they agree.
#
# ⛔ THE FAILURE THIS IS FOR IS A SECOND MEASUREMENT, NOT A BAD ONE. `--json`
# exists so a task can read a period end back; the cheap way to build it would
# have been a second pass that computed the figures again for the machine. Two
# passes over one book give two answers the moment anything is stateful — and
# both would tie, both would look right, and the pair would disagree by an
# amount nobody could attribute. The text report and the JSON are one
# computation with two renderings, and this is what holds them to that.
#
# ⚠ FIGURES ONLY, NEVER DURATIONS. Two renderings of one run print the SAME
# net asset value and the SAME trial balance; they cannot print the same
# nanoseconds, because the second run of anything is a different run. A test
# that compared timings here would be flaky for a reason that is not a defect.
set -euo pipefail

RATIO="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
BOOK="${TEST_TMPDIR:-/tmp}/bench-renderings-book"

fail() { echo "  x $*" >&2; exit 1; }

rm -rf "$BOOK"
"$RATIO" gen --book "$BOOK" --securities 8 --lots-per 20 --currencies 3 --seed 3 >/dev/null

text="$("$RATIO" bench --fold --book "$BOOK" 2>/dev/null)"
json="$("$RATIO" bench --fold --book "$BOOK" --json 2>/dev/null)"

# ⛔ THE JSON MUST BE THE WHOLE OF STDOUT UNDER `--json`. A stray report line
# above it would still parse out of a `grep`, and would break every consumer
# that reads stdout as one document.
echo "$json" | python3 -c 'import json,sys; json.load(sys.stdin)' \
  || fail "--json did not print a single JSON document on stdout"

# Pull one figure out of each rendering and insist they match.
#
# `field <label>` reads the text report; `key <name>` reads the JSON.
field() { echo "$text" | grep -E "^  $1 " | grep -oE '\-?[0-9]+' | head -1; }
key() { echo "$json" | python3 -c "import json,sys; print(json.load(sys.stdin)['$1'])"; }

check() {
  local label="$1" name="$2" a b
  a="$(field "$label")"; b="$(key "$name")"
  [ -n "$a" ] || fail "the text report has no '$label' line to compare"
  [ "$a" = "$b" ] || fail "$label: the report says $a and --json says $b"
  echo "  ok  $label = $a, both ways"
}

check "journal entries" journal_entries
check "open positions"  open_positions
check "open tax lots"   open_lots
check "trial balance"   trial_balance
check "basis relieved"  basis_relieved
check "realized gain"   realized_gain

# ⭐ THE TRIAL BALANCE TIES, which is what makes the rest of it worth comparing.
# Two renderings agreeing about a book that did not balance would be two
# renderings of a wrong answer.
[ "$(key trial_balance)" = "0" ] || fail "the generated book does not tie: $(key trial_balance)"

# ⛔ BOTH CURVES, OR THE JSON IS AN OVERCLAIM WAITING TO HAPPEN. The text report
# cannot show the strike without the cold build; neither may this one. A
# consumer able to read `strike_ns` alone is one screen away from publishing
# "twenty million lots, twelve microseconds" as the whole story.
for k in cold_build_ns parse_ns fold_ns strike_ns nav_ns; do
  echo "$json" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert '$k' in d, '$k missing from --json'
assert isinstance(d['$k'], int), '$k is not a number'
" || fail "--json is missing $k — both curves must survive this rendering"
done
echo "  ok  both curves present: cold build and strike"

# The shape is carried, so a result can say which fund it is about.
echo "$json" | python3 -c "
import json,sys
s=json.load(sys.stdin)['shape']
assert s['securities']==8 and s['lots_per']==20 and s['currencies']==3 and s['seed']==3, s
assert s['method']=='fifo', s
" || fail "--json did not carry the shape the book was generated from"
echo "  ok  the shape came back: 8 securities, 20 lots each, 3 currencies, seed 3"

echo "  ok  one measurement, two renderings, no disagreement"
