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
# ⛔ THE STRIKE FIXTURES COME FROM A FUND THAT HAS ONE. `$FUND` is the blocked
# book on purpose — it carries the break, the pending fact and the explanation
# state the screens are about — and a blocked fund now REFUSES its own NAV, so
# it has no strikes to capture and `id navStrikes.json` below would index an
# empty list. Which is the demo working: a fund that says BLOCKED and shows a
# NAV is a screen nobody should believe.
STRUCK="${RATIO_FIXTURE_STRUCK_FUND:-northstar-multi-strategy}"
OUT="$(cd "$(dirname "$0")/../fixtures" && pwd)"

# ⛔ `explain.json` HAS NEVER BEEN CAPTURED, AND IT IS THE ONE FIXTURE IN THIS
# DIRECTORY THAT IS NOT A RECORDING. It was emitted by `ratio_nav::explain::
# plan_of` directly — the real builder, so every step, citation, note and
# modelled figure is what the server would compute — with the encoding copied
# from `transcode.rs` and the fund's SHAPE chosen to agree with the `view.json`
# committed beside it (twelve positions, 252,843 open lots, prefix 6). What has
# never been checked is whether a real book's dials are those.
#
# ⚠ AND THE FIRST REAL CAPTURE WILL MOVE IT TO `$STRUCK`, along with
# `navStrikes.json` and `replay.json` — the committed trio is still harbourline's
# from before that fund became the blocked one. They move together or they
# describe two funds, which is worse than being stale.
#
# ⚠ THAT IS EXACTLY THE THING THIS SCRIPT'S HEADER WARNS ABOUT, so it is written
# down rather than left to be discovered. Run the capture below and commit
# whatever comes back, even if nothing else changed: a fixture that outlives the
# gap it was written for is how a captured fixture stops being a capture, which
# HANDOFF.md already records happening to `reconcile.json`.
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
get navStrikes.json         "funds/${STRUCK}/navStrikes"
get configVersions.json     "funds/${FUND}/configVersions"
get rules.json              "funds/${FUND}/rules"
get templates.json          "funds/${FUND}/templates"
# ⚠ THAT CAPTURE IS THE FUND BOOK. CreateBook also seeds `bank-statement`
# (Personal) and `project-invoices` (Project). After a recapture, merge those
# two rows back from git — `console/src/lib/templates.test.ts` fails if a
# CreateBook id is missing, which is how a harbourline-only refresh stays
# honest. `prime_equity_trades` is on the fund book and on CreateBook
# Investment; a recapture should keep it.
get deliveries.json         "funds/${FUND}/deliveries"
get pendingFacts.json       "funds/${FUND}/pendingFacts"
get corporateActions.json   "funds/${FUND}/corporateActions"
get changeLogEntries.json   "funds/${FUND}/changeLogEntries"

# The singletons need an id from the list above, so they are derived rather than
# named — a hard-coded break id goes stale the first time the seed changes.
id() { python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d[sys.argv[2]][0]["name"].split("/")[-1])' "$OUT/$1" "$2"; }

get break.json      "funds/${FUND}/breaks/$(id breaks.json breaks)"
get postings.json   "funds/${FUND}/accounts/$(id accounts.json accounts)/postings"
# The journal — list then the one posting cites. Id from the captured
# posting, not a hard-coded one: a seed that renumbers entries would
# otherwise leave this file pointing at a row the book no longer has.
get entries.json    "funds/${FUND}/entries"
get entry.json      "funds/${FUND}/entries/$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["postings"][0]["entryId"])' "$OUT/postings.json")"
get lots.json       "funds/${FUND}/positions/$(id positions.json positions)/lots"
get replay.json     "funds/${STRUCK}/navStrikes/$(id navStrikes.json navStrikes):replay"
# ⛔ `$STRUCK`, FOR THE REASON `replay.json` USES IT — a plan explains a STRIKE,
# and a blocked fund has none to explain. Capturing this against `$FUND` would
# index an empty list exactly as the strike list does.
#
# ⚠ AND WITHOUT `?analyze=true`, deliberately. The render suite's default case is
# the unmeasured plan, and it asserts every actual is BLANK rather than zero —
# capturing an analyzed one would make that case pass against a fixture that
# could not fail it.
get explain.json    "funds/${STRUCK}/navStrikes/$(id navStrikes.json navStrikes):explain"
get diff.json       "funds/${FUND}/configVersions/$(id configVersions.json configVersions):diff"

echo "captured into $OUT — now run: python3 console/scripts/fixtures_test.py \\"
echo "  proto/ratio/console/v1/console.proto console/fixtures"
