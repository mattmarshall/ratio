#!/usr/bin/env bash
#
# Two benchmarks running at once must not delete each other's book.
#
# ⛔ THIS IS A REGRESSION TEST FOR A MEASUREMENT THAT WAS PUBLISHED WRONG.
# `ratio bench` generated into `$TMPDIR/ratio-bench-book` — one fixed name for
# every run — and `ratio_gen::generate` opens a book with `remove_dir_all`. So a
# second bench starting while a first was folding deleted the directory out from
# under it, and the first went on to measure whatever survived.
#
# ⚠ AND THE WRONG ANSWER TIED, WHICH IS WHY IT NEEDS A TEST RATHER THAN A
# COMMENT. A 500 x 2000 run measured 1,022,625 open lots over 94.7 s alone, and
# 224,852 over 20.7 s beside a small one: 22% of the lots in 22% of the time,
# entry count correct, trial balance 0, no error on any stream. Nothing about it
# looked like a broken run — it looked like a fund with less fragmentation than
# expected, which is a thing funds are.
#
# HANDOFF.md names this trap for TESTS — "two tests naming the same book wipe
# each other's directory" — and it was in the benchmark the whole time, which is
# the one place a number gets quoted at people.
set -euo pipefail

RATIO="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
OUT="${TEST_TMPDIR:-/tmp}"

fail() { echo "  x $*" >&2; exit 1; }

# ⚠ SMALL, BUT NOT SO SMALL THE RACE CANNOT HAPPEN. The window is between one
# run's generate and its fold, so both runs need to be long enough to overlap.
SHAPE="--securities 200 --lots-per 300 --currencies 3 --seed 1"

lots_of() {
  "$RATIO" bench $SHAPE --json 2>/dev/null \
    | python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["open_lots"], d["journal_entries"], d["trial_balance"])'
}

# The truth: one run, on its own, with nothing to race against.
alone="$(lots_of)"
[ -n "$alone" ] || fail "a lone bench produced no JSON"
echo "  ok  alone: $alone"

# ⛔ THE SAME SHAPE, TWICE, AT ONCE. Identical dials and seed, so ratio-gen is
# required to produce byte-identical books and both runs must report identical
# figures. Any difference is one run reading the other's directory.
lots_of >"$OUT/a.txt" &
pid_a=$!
lots_of >"$OUT/b.txt" &
pid_b=$!
wait "$pid_a" || fail "the first concurrent bench failed"
wait "$pid_b" || fail "the second concurrent bench failed"

a="$(cat "$OUT/a.txt")"
b="$(cat "$OUT/b.txt")"

[ "$a" = "$alone" ] || fail "run A beside another reported '$a', alone it reports '$alone' — \
one book was deleted while the other was being folded"
[ "$b" = "$alone" ] || fail "run B beside another reported '$b', alone it reports '$alone' — \
one book was deleted while the other was being folded"

echo "  ok  A beside B: $a"
echo "  ok  B beside A: $b"

# ⭐ AND THE BOOKS TIE, because two runs agreeing on a figure that does not
# balance would be two readings of the same wrong answer.
[ "${a##* }" = "0" ] || fail "the concurrent run does not tie: trial balance ${a##* }"

echo "  ok  two benches at once, identical figures, both tying"
