#!/usr/bin/env bash
#
# The five-minute demo, run end to end with no human in it.
#
# PLAN.md Stage 2 is "done when the five-minute script runs end to end without
# a rehearsal." That claim decays the moment nobody checks it, so this *is* the
# rehearsal, it runs in CI, and it asserts at every step rather than printing
# and hoping. The demo's whole point is a fence, and a fence nobody tests is a
# fence that quietly opens.
#
# The one thing not automated here is the model. Steps 2–4 in PLAN.md are a
# person talking to Claude, which drives the MCP tools; this script drives
# those same tools directly over the same stdio transport, so what is verified
# is the surface the model actually touches — including that the tool it must
# not have is not there.
#
# Usage:  demo/rehearse.sh path/to/ratio
set -euo pipefail

RATIO="${1:?usage: rehearse.sh path/to/ratio-binary}"
[ -x "$RATIO" ] || { echo "not executable: $RATIO" >&2; exit 1; }
RATIO="$(cd "$(dirname "$RATIO")" && pwd)/$(basename "$RATIO")"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$WORK"

pass() { printf '  ok  %s\n' "$1"; }
fail() { printf '  XX  %s\n' "$1" >&2; exit 1; }
# Grep whose failure is an assertion failure, not a pipeline abort.
has()  { grep -qF -- "$2" <<<"$1" || fail "$3"; }
lacks(){ grep -qF -- "$2" <<<"$1" && fail "$3"; return 0; }

echo "== 1. an empty book =="
"$RATIO" init --book book >/dev/null
printf '' > empty.toml
"$RATIO" config set empty.toml --book book >/dev/null
OUT="$("$RATIO" balance --book book)"
has "$OUT" "0 entrie(s)"     "a fresh book should hold no entries"
has "$OUT" "Difference"      "the balance should report a difference line"
pass "zeros on screen"

echo "== 2-3. the model proposes a rule (MCP, over stdio) =="
# What Claude sends after hearing the rule said out loud. The fee legs are a
# weight vector, not amounts — the kernel's conservation law is what makes the
# entry balance, at any amount.
cat > mcp-in.jsonl <<'JSONL'
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}
{"jsonrpc":"2.0","id":2,"method":"tools/list"}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"list_accounts","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"propose_rule","arguments":{"toml":"[[rule]]\nid = \"management_fee_accrual\"\nkind = \"accrual\"\ndescription = \"Management fee, 75bp a year on prior-day net assets, ACT/365\"\nrate_bp = 75\nday_count = \"act/365\"\n[[rule.posting]]\naccount = 10\nweight = 1\n[[rule.posting]]\naccount = 40\nweight = -1\n"}}}
{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"approve_rule","arguments":{"id":"anything"}}}
{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"set_active","arguments":{}}}
JSONL
MCP="$("$RATIO" mcp --book book < mcp-in.jsonl)"
has "$MCP" '"name":"propose_rule"'  "propose_rule should be offered"
has "$MCP" 'rule management_fee_accrual {' "the draft should come back rendered, not as TOML"
has "$MCP" 'is NOT active'          "a proposal must say plainly that it is inert"
pass "draft appears, readable, and inert"

echo "== 4. the fence =="
# The assertion this whole demo exists for. Not "approve_rule returns an
# error" — approve_rule is *not a tool*, so it is absent from tools/list too.
# An error would be a permission check somebody could later relax; absence is
# not relaxable.
LIST="$(sed -n 's/.*"id":2,//p' <<<"$MCP" | head -1)"
lacks "$LIST" 'approve_rule' "approve_rule must not appear in tools/list"
lacks "$LIST" 'set_active'   "set_active must not appear in tools/list"
has "$MCP" '"isError":true'  "calling approve_rule should be refused"
has "$MCP" 'ratio approve'   "the refusal should name the human path"
# And the refusal must not have been a write with a complaint attached.
STILL="$("$RATIO" rules show --book book 2>&1 || true)"
has "$STILL" "(no rules)" "nothing the model did may have made a rule active"
pass "the model could not approve, and nothing went active"

echo "== 5. a person approves =="
ID="$(ls book/proposals | head -1 | sed 's/\.toml$//')"
[ -n "$ID" ] || fail "propose_rule wrote no proposal"
APPROVE="$("$RATIO" approve "$ID" --book book)"
has "$APPROVE" "approved $ID" "approve should confirm by id"
has "$APPROVE" "config "      "approve should surface a config hash"
CONFIG="$(sed -n 's/.*config \([0-9a-f]*\).*/\1/p' <<<"$APPROVE" | head -1)"
[ -n "$CONFIG" ] || fail "no config hash in the approval output"
pass "config $CONFIG, and only a human could have done it"

echo "== 6. post, and watch it tie =="
# PLAN.md says ten thousand. A day's accrual per event, on a basis that moves,
# so the figures are not a repeated constant.
python3 - <<'PY'
import json
json.dump(
    [{"rule": "management_fee_accrual", "id": f"fee-{d:05d}",
      "amount": 100_000_000 + d * 7919, "days": 1, "memo": "daily accrual"}
     for d in range(1, 10_001)],
    open("events.json", "w"))
PY
APPLIED="$("$RATIO" apply events.json --book book)"
has "$APPLIED" "10000 event(s)" "all ten thousand events should apply"
OUT="$("$RATIO" balance --book book)"
has "$OUT" "10000 entrie(s)" "the book should hold ten thousand entries"
# The difference line reads "Difference  <debit>  <credit>"; both must be zero.
DIFF="$(grep '^Difference' <<<"$OUT" | awk '{print $2, $3}')"
[ "$DIFF" = "0.00 0.00" ] || fail "difference is '$DIFF', expected '0.00 0.00'"
pass "ten thousand entries, difference 0.00"

echo "== 7. the model explains a figure, and cites the hash =="
printf '{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"trial_balance","arguments":{}}}\n' > read.jsonl
READ="$("$RATIO" mcp --book book < read.jsonl)"
has "$READ" "$CONFIG" "the reported balance must cite the config that produced it"
pass "reporting, not deciding"

echo "== 8. and none of it panics when piped =="
# Rust turns SIGPIPE off at startup, so `ratio explain | head` panics unless
# main puts it back. This check needs output larger than the 64KB pipe buffer
# or the producer finishes before the reader closes anything and the check
# passes vacuously — which is why it lives here, after ten thousand entries,
# and not in a script with a six-entry book.
LINES="$("$RATIO" explain 10 --book book | wc -l)"
[ "$LINES" -gt 10000 ] || fail "explain printed $LINES lines, too few to close a pipe on"
set +o pipefail
"$RATIO" explain 10 --book book 2>err.txt | head -2 >/dev/null
set -o pipefail
grep -qF "panicked" err.txt && fail "panicked on a closed pipe: $(cat err.txt)"
pass "$LINES lines through a pipe closed at 2, no panic"

echo
echo "the model wrote the rule, and still could not have made the books wrong."
