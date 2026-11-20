#!/usr/bin/env bash
#
# Build the ratio CLI and drive a throwaway book end to end, asserting the
# behaviors a change most often breaks without a unit test noticing.
#
# Usage:  .agents/skills/run-ratio/scripts/smoke.sh [--keep]
#
#   CLI     init → post (balanced) → post (unbalanced, must be REFUSED) →
#           balance (must tie) → strike --as-of today → navs → replay
#   server  `ratio server` on a free port → REST post (200) → unbalanced
#           post (400 FAILED_PRECONDITION) → trial balance (difference 0)
#
# --keep leaves the book directory behind and prints its path, for poking at.
#
# ⚠ A SMOKE TEST, NOT A SUITE. It proves the binary runs and the headline
# refusals hold on a real book. `bazel test //...` is still the gate.
set -euo pipefail

keep=0
[ "${1:-}" = "--keep" ] && keep=1

top="$(git rev-parse --show-toplevel)"
cd "$top"

echo "building //crates/ratio:ratio"
bazel build //crates/ratio:ratio >/dev/null 2>&1 || bazel build //crates/ratio:ratio
ratio="$top/bazel-bin/crates/ratio/ratio"

work="$(mktemp -d)"
server_pid=""
cleanup() {
  [ -n "$server_pid" ] && kill "$server_pid" 2>/dev/null || true
  if [ "$keep" -eq 1 ]; then echo "kept: $work"; else rm -rf "$work"; fi
}
trap cleanup EXIT

export RUST_BACKTRACE=0 RATIO_ACTOR=smoke
book="$work/fund-1"
pass() { echo "  ok    $*"; }
fail() { printf '  FAIL  %b\n' "$*" >&2; exit 1; }

echo "CLI on $book"
"$ratio" init --kind investment --book "$book" >/dev/null || fail "init failed"
pass "init --kind investment"

cat >"$work/ok.json" <<'EOF'
[{"id":"sub-1","postings":[{"dim":2,"amount":1000000},{"dim":20,"amount":-1000000}]}]
EOF
cat >"$work/bad.json" <<'EOF'
[{"id":"bad-1","postings":[{"dim":2,"amount":500},{"dim":20,"amount":-400}]}]
EOF

out=$("$ratio" post "$work/ok.json" --book "$book") || fail "a balanced entry was refused"
grep -q "posted   1" <<<"$out" || fail "unexpected post output: $out"
pass "a balanced entry posts"

if out=$("$ratio" post "$work/bad.json" --book "$book" 2>&1); then
  fail "an entry netting to 100 was ACCEPTED: $out"
fi
grep -q "does not conserve value" <<<"$out" || fail "refused, but not for conservation: $out"
pass "an unbalanced entry is refused, and says why"

tb=$("$ratio" balance --book "$book")
grep -Eq "^Difference +0\.00 +0\.00" <<<"$tb" || fail "trial balance does not tie:\n$tb"
grep -q "TRIAL BALANCE — 1 entrie" <<<"$tb" || fail "the refused entry reached the journal:\n$tb"
pass "the trial balance ties over exactly the one accepted entry"

today="$(date -u +%F)"
struck=$("$ratio" strike --as-of "$today" --book "$book" 2>/dev/null)
id=$(sed -n 's/^struck \([^ ]*\) in .*/\1/p' <<<"$struck")
[ -n "$id" ] || fail "strike printed no id:\n$struck"
grep -q "NAV        10000.00" <<<"$struck" || fail "unexpected NAV:\n$struck"
pass "strike --as-of $today lands as $id with NAV 10000.00"

if "$ratio" strike --as-of 1999-01-01 --book "$book" >/dev/null 2>&1; then
  fail "a strike --as-of another day was accepted"
fi
pass "a strike --as-of another day is refused"

grep -q "10000.00" <<<"$("$ratio" navs --book "$book")" || fail "navs does not list the strike"
pass "navs lists the strike"
"$ratio" replay "$id" --book "$book" >/dev/null 2>&1 || fail "replay of $id failed"
pass "replay re-derives $id"

echo "server (gRPC + REST on one port)"
port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
"$ratio" server --book "$book" --addr "127.0.0.1:$port" >"$work/server.log" 2>&1 &
server_pid=$!
base="http://127.0.0.1:$port/v1/books/fund-1"
for _ in $(seq 1 50); do
  curl -s -o /dev/null "http://127.0.0.1:$port/" && break
  sleep 0.2
done

code=$(curl -s -o "$work/r1" -w '%{http_code}' -X POST "$base/transactions" \
  -H 'content-type: application/json' \
  -d '{"postings":[{"dim":2,"amount":-125000},{"dim":1,"amount":125000}]}')
[ "$code" = 200 ] || fail "balanced REST post returned $code: $(cat "$work/r1")"
pass "REST: a balanced transaction posts (200)"

code=$(curl -s -o "$work/r2" -w '%{http_code}' -X POST "$base/transactions" \
  -H 'content-type: application/json' \
  -d '{"postings":[{"dim":2,"amount":-100},{"dim":1,"amount":90}]}')
[ "$code" = 400 ] && grep -q FAILED_PRECONDITION "$work/r2" \
  || fail "unbalanced REST post returned $code: $(cat "$work/r2")"
pass "REST: an unbalanced transaction is FAILED_PRECONDITION (400)"

curl -s "$base/trialBalance" | grep -q '"difference":"0"' \
  || fail "REST trial balance does not tie: $(curl -s "$base/trialBalance")"
pass "REST: the trial balance ties"

echo "smoke passed"
