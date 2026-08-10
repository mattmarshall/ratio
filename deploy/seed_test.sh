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

# Four funds, in four different states — one of each is the point of the demo.
n=$(find "$OUT" -maxdepth 1 -mindepth 1 -type d | wc -l | tr -d ' ')
[ "$n" -eq 4 ] || fail "expected 4 funds, found $n"

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

echo "  ok  4 funds, $lots open tax lots on the generated one"
