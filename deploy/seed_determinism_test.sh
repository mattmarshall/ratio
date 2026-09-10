#!/usr/bin/env bash
#
# An unchanged source tree must produce the same durable seed tomorrow.
#
# #327 compared two builds that happened inside one wall-clock second, so the
# test stayed green while Delivery.received, Fact.provenance.received and an
# explanation's accept_time were guaranteed to drift on the next deploy.
set -euo pipefail

RATIO="$(cd "$(dirname "$1")" && pwd)/$(basename "$1")"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="${TEST_TMPDIR:-/tmp}/seed-determinism"
rm -rf "$ROOT"
mkdir -p "$ROOT"

EXPLAIN_THE_BREAK=1 "$HERE/seed-demo-book.sh" "$RATIO" "$ROOT/a" >/dev/null
sleep 2
EXPLAIN_THE_BREAK=1 "$HERE/seed-demo-book.sh" "$RATIO" "$ROOT/b" >/dev/null

for plane in journal deliveries entities facts actions explanations closes; do
  a="$ROOT/a/${plane}.jsonl"
  b="$ROOT/b/${plane}.jsonl"
  if [ -e "$a" ] || [ -e "$b" ]; then
    cmp "$a" "$b" || {
      echo "  x unchanged baked plane $plane depends on the build clock" >&2
      exit 1
    }
  fi
done

# `ratio gen` anchors settlement tails to now in normal CLI use. The seed epoch
# is the deployment source of truth that makes the baked large fund stable.
RATIO_SEED_EPOCH=1788393600 "$RATIO" gen --book "$ROOT/gen-a" \
  --securities 20 --lots-per 40 --currencies 3 --seed 1 >/dev/null
sleep 2
RATIO_SEED_EPOCH=1788393600 "$RATIO" gen --book "$ROOT/gen-b" \
  --securities 20 --lots-per 40 --currencies 3 --seed 1 >/dev/null
cmp "$ROOT/gen-a/journal.jsonl" "$ROOT/gen-b/journal.jsonl" || {
  echo "  x unchanged generated fund depends on the build clock" >&2
  exit 1
}

echo "  ok  delivery, fact, explanation, and generated-journal seeds are byte-identical"
