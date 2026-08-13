#!/usr/bin/env bash
#
# Regenerate console/fixtures/ from a RUNNING ratio.
#
# ⛔ CAPTURED, NOT WRITTEN. A fixture somebody typed is a claim about what the
# server sends; a fixture the server sent is what it sends. This repository has
# the same distinction between `//proto:mirrors_test`'s two mirrors, and it says
# which is the dangerous one: "types.ts can promise a field the server never
# sends, and TypeScript will be perfectly happy about it." A hand-edited fixture
# is that failure with a test suite green on top of it.
#
# ⚠ `//console:fixtures_test` checks the SHAPE of whatever is here against
# console.proto on every build, so a hand-edit cannot invent a field. It cannot
# check the VALUES — only a real book knows what a real book says — which is why
# this script exists and why it should be what refreshes them.
#
#   bazel build //crates/ratio
#   ./deploy/seed-demo-funds.sh bazel-bin/crates/ratio/ratio /tmp/demo-funds
#   RATIO_FUNDS=/tmp/demo-funds bazel run //crates/ratio -- watch --book /tmp/demo-funds &
#   console/scripts/capture_fixtures.sh
#
# ⚠ A local `ratio watch` sets no RATIO_AUTH, so it answers as Subject::Local
# and needs no token. Do not point this at the deployed API with a real session:
# these files are committed, and a real book's figures are a customer's.
set -euo pipefail

API="${RATIO_API_ORIGIN:-http://127.0.0.1:7373}"
FUND="${RATIO_FIXTURE_FUND:-harbourline-global-value}"
OUT="$(cd "$(dirname "$0")/../fixtures" && pwd)"

get() { # get <file> <path>
  local body
  body="$(curl -sf "${API}/v1/$2")" || {
    echo "::error::could not read /v1/$2 from $API" >&2
    exit 1
  }
  printf '%s\n' "$body" | python3 -m json.tool > "$OUT/$1"
  echo "  ok  $1"
}

# One representative of each list, and one of each singleton the screens open.
get funds.json              "funds"
get fund.json               "funds/${FUND}"
get breaks.json             "funds/${FUND}/breaks"
get accounts.json           "funds/${FUND}/accounts"
get positions.json          "funds/${FUND}/positions"
get navStrikes.json         "funds/${FUND}/navStrikes"
get configVersions.json     "funds/${FUND}/configVersions"
get rules.json              "funds/${FUND}/rules"
get templates.json          "funds/${FUND}/templates"
get deliveries.json         "funds/${FUND}/deliveries"
get pendingFacts.json       "funds/${FUND}/pendingFacts"
get corporateActions.json   "funds/${FUND}/corporateActions"
get changeLogEntries.json   "funds/${FUND}/changeLogEntries"

# The singletons need an id from the list above, so they are derived rather than
# named — a hard-coded break id goes stale the first time the seed changes.
id() { python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d[sys.argv[2]][0]["name"].split("/")[-1])' "$OUT/$1" "$2"; }

get break.json      "funds/${FUND}/breaks/$(id breaks.json breaks)"
get postings.json   "funds/${FUND}/accounts/$(id accounts.json accounts)/postings"
get lots.json       "funds/${FUND}/positions/$(id positions.json positions)/lots"
get replay.json     "funds/${FUND}/navStrikes/$(id navStrikes.json navStrikes):replay"
get diff.json       "funds/${FUND}/configVersions/$(id configVersions.json configVersions):diff"

echo "captured into $OUT — now run: python3 console/scripts/fixtures_test.py \\"
echo "  proto/ratio/console/v1/console.proto console/fixtures"
