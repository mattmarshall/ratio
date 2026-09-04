#!/usr/bin/env bash
#
# PLAN.md refuses to build things. This checks it is still telling the truth.
#
# ⛔ IT WAS NOT. The refusal list had eleven entries on 2026-08-07, and four of
# them — tax lots, multi-currency, corporate actions, Postgres — were built on
# 08-09 and 08-10 with the list untouched. For two days the plan and the
# repository disagreed about what the product is, and nothing said so. Every
# individual commit was defensible; that is what made it invisible.
#
# ⚠ THE TRIGGER IS THE POINT. CI skips `**/*.md`, so this does NOT run when
# someone edits the plan — it runs when someone builds CODE. That is the right
# way round: the moment to notice a contradiction is the commit that creates it,
# not a later reading of the document.
#
# ⛔ WHAT THIS CANNOT DO, STATED PLAINLY. It does NOT scan the tree. The first
# version tried to, with `[ -e crates/… ]`, and was VACUOUS: a Bazel test sees
# only its runfiles, `glob` does not cross package boundaries, so every path
# check was false and the file reported "none contradicted" under every possible
# edit. It could not have failed.
#
# So this is a cross-check between two DECLARED lists — the plan's refusals, and
# `BUILT` below — and its honest limitation is that a feature nobody adds to
# `BUILT` stays invisible. What it buys is that the two lists cannot silently
# drift apart once both are written down, which is the failure that actually
# happened. Adding a line to `BUILT` is the cost of building something the plan
# refused, and it is meant to be a moment where somebody notices.
#
# The one tree fact it is now handed is the `BookKind` union: Bazel passes
# `console/src/wire/types.ts` and `console/scripts/book_kinds_in_plan_test.py`
# so a kind the console offers that PLAN.md does not name goes red here,
# which is how PERSONAL shipped. That script still does not scan the tree.
set -euo pipefail

PLAN="$1"
# Optional: the BookKind union and the console-side check that holds PLAN.md
# to it. Bazel passes both; a bare `./plan_refusals_test.sh PLAN.md` still
# does the BUILT half. See console/scripts/book_kinds_in_plan_test.py.
TYPES="${2-}"
KINDS_PY="${3-}"

fail() { echo "  x $*" >&2; exit 1; }

# Things this repository HAS BUILT. Add a line here when you build one, and the
# check will tell you if the plan still refuses it.
#
# ⚠ THE PHRASE MUST MATCH THE PLAN LITERALLY, on the list or off it. A near-miss
# checks nothing, which is the failure this whole file is about — so a phrase
# that appears NOWHERE in the plan is itself an error.
BUILT=(
  "tax lots and cost basis"
  "multi-currency and FX"
  "corporate actions"
  "Postgres"
  # ⚠ ADDED WITH THE FEATURE, WHICH IS THE WHOLE PROTOCOL. Multi-view books are
  # not on the refusal list, so this check would have stayed green whatever was
  # built — it cross-checks two DECLARED lists and scans no tree, and says so
  # above. What the entry buys is that the day somebody adds "multi-view books"
  # to the refusal list, this goes red instead of the two documents quietly
  # disagreeing. The sentence it matches is in the 2026-08-13 amendment.
  "multi-view books"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED THEM, WHICH IS THE PROTOCOL.
  # Independent Books and WorkOS AuthKit are not on the refusal list — this
  # check would have stayed green whatever was built, same as multi-view
  # books. The sentences they match are in the 2026-09-03 book-centric
  # amendment. Four merged PRs landed them without this file moving; adding
  # the lines is the cost of noticing.
  "independent Books"
  "WorkOS AuthKit"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED THEM, WHICH IS THE PROTOCOL.
  # Wash sales are not on the refusal list — this check would have
  # stayed green whatever was built. The sentence it matches is in the
  # 2026-09-03 wash-sales amendment. #133 / #138 landed the engine
  # without this file moving; adding the line is the cost of noticing.
  "wash sales"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED THEM, WHICH IS THE PROTOCOL.
  # MinTax is not on the refusal list — this check would have stayed
  # green whatever was built. The sentence it matches is in the
  # 2026-09-03 MinTax amendment. #141 landed the ranking without this
  # file moving; adding the line is the cost of noticing.
  "MinTax is a ranking at a price"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED THEM, WHICH IS THE PROTOCOL.
  # SpecID is not on the refusal list — this check would have stayed
  # green whatever was built. The sentence it matches is in the
  # 2026-09-03 SpecID amendment. #143 landed the named selection
  # without this file moving; adding the line is the cost of noticing.
  "SpecID is a named selection"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED THEM, WHICH IS THE PROTOCOL.
  # Average cost is not on the refusal list — this check would have stayed
  # green whatever was built. The sentence it matches is in the
  # 2026-09-03 Average-cost amendment.
  "Average cost is a pool"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # WashRestatement is not on the refusal list — this check would have
  # stayed green whatever was built. The sentence it matches is in the
  # 2026-09-03 WashRestatement amendment.
  "WashRestatement is a citeable record"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The non-US holding-period variant is not on the refusal list — this
  # check would have stayed green whatever was built. The sentence it
  # matches is in the 2026-09-03 wash-holding amendment.
  "the non-US holding-period variant is an election"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The console wash-flag cite is not on the refusal list — this check
  # would have stayed green whatever was built. The sentence it matches
  # is in the 2026-09-03 console-cite amendment.
  "the console cites the wash election"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The MinTax / SpecID / average-cost console cites are not on the
  # refusal list — this check would have stayed green whatever was
  # built. The sentence it matches is in the 2026-09-04 console-cite
  # amendment.
  "the console cites the MinTax, SpecID, and average-cost elections"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The Personal tax-pack Connect app is not on the refusal list —
  # tax e-file stays refused; this is the product door the catalog
  # already named. The sentence it matches is in the 2026-09-04
  # tax-pack amendment.
  "a Personal tax-pack Connect app"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The Personal net-worth goals Connect app is not on the refusal
  # list — a cash forecast / FIRE number stays refused; this is the
  # product door the catalog already named. The sentence it matches
  # is in the 2026-09-04 goals amendment.
  "a Personal net-worth goals Connect app"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The Project vendor-portal Connect app is not on the refusal list —
  # a vendor user directory / AIA G702 product UI / EAC stay refused;
  # this is the product door the catalog already named. The sentence
  # it matches is in the 2026-09-04 vendor-portal amendment.
  "a Project vendor-portal Connect app"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The pooled holding-period category is not on the refusal list —
  # this check would have stayed green whatever was built. The
  # sentence it matches is in the 2026-09-04 PoolPeriod amendment.
  "the pooled holding-period category is a date"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The Project AIA pay-app Connect app is not on the refusal list —
  # AIA G702 product UI stays refused; this is the product door the
  # catalog already named. The sentence it matches is in the 2026-09-04
  # pay-app amendment.
  "a Project AIA pay-app Connect app"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The partner allocation cut is not on the refusal list — this check
  # would have stayed green whatever was built. The sentence it matches
  # is in the 2026-09-04 partner-cut amendment. #191 landed the cut
  # without this file moving; adding the line is the cost of noticing.
  "the partner allocation cut is named weights"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The Personal bank-feed Connect app is not on the refusal list —
  # bank OAuth product UI stays refused; this is the product door the
  # catalog already named. The sentence it matches is in the 2026-09-04
  # bank-feed amendment. #192 landed the scaffold without this file
  # moving; adding the line is the cost of noticing.
  "a Personal bank-feed Connect app"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Unit-movement subscriptions are not on the refusal list — this
  # check would have stayed green whatever was built. The sentence it
  # matches is in the 2026-09-04 unit-movement amendment. #196 landed
  # them without this file moving; adding the line is the cost of
  # noticing.
  "subscriptions and redemptions are unit movements"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The write-route actor binding is not on the refusal list — this
  # check would have stayed green whatever was built. The sentence it
  # matches is in the 2026-09-04 write-route amendment. #198 landed
  # the `sub` without this file moving; adding the line is the cost
  # of noticing. Leftovers stay on #22.
  "write-route actor is the WorkOS \`sub\`"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Management-fee accrual on the journal is not on the refusal list —
  # invoice / LP packaging stays Connect. The sentence it matches is
  # in the 2026-09-04 fee-accrual amendment.
  "management-fee accrual posts receivable/expense"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # The Project EAC / forecast Connect app is not on the refusal list —
  # EAC fields on /budget and a silent forecast of 0 stay refused;
  # this is the product door the catalog already named. The sentence
  # it matches is in the 2026-09-04 EAC / forecast amendment.
  "a Project EAC / forecast Connect app"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Unitized seed / period issued / per-share are not on the refusal
  # list — this check would have stayed green whatever was built. The
  # sentence it matches is in the 2026-09-04 leftovers amendment.
  "unitized seed, period issued/redeemed, per-share"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Change-order / award ingest on /budget is not on the refusal list —
  # a second budget store and EAC fields on /budget stay refused. The
  # sentence it matches is in the 2026-09-04 #170 amendment.
  "change-order and award ingest on /budget"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED THE DECISION, WHICH IS
  # THE PROTOCOL. Equalization / drip / side-pocket were not built —
  # they stay Connect. The sentence it matches is in the 2026-09-04
  # #177 amendment. Adding the line is the cost of noticing if the
  # phrase later lands on the refusal list as if the door were shut.
  "equalization, drip, and side-pocket stay Connect"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Cash application on /billing is not on the refusal list —
  # payment processors and a fake collected 0.00 stay refused. The
  # sentence it matches is in the 2026-09-04 #173 amendment.
  "cash application on /billing"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Live custodian statement ingest is not on the refusal list —
  # broker OAuth and fund-volume recon stay refused. The sentence
  # it matches is in the 2026-09-04 #155 amendment (already on main).
  "live custodian statement ingest"
  # ⚠ ADDED WITH THE AMENDMENT THAT RECORDED IT, WHICH IS THE PROTOCOL.
  # Job-cost / AP statement ingest is not on the refusal list —
  # retainage invention, a vendor portal, and a second recon engine
  # stay refused. The sentence it matches is in the 2026-09-04 #171
  # amendment.
  "job-cost / AP statement ingest"
)

# ⛔ FLATTENED, BECAUSE MARKDOWN WRAPS. The list is prose, so "the client
# portal" is written across a line break and a literal grep for it finds
# nothing — which the vacuity guard caught on the first run of this file. A
# check defeated by a line wrap is a check that passes for the wrong reason.
flat() { tr '\n' ' ' | tr -s ' '; }

WHOLE=$(flat <"$PLAN")

# The refusal list: from the heading to the first sub-heading.
LIST=$(awk '/^## Explicitly not building/{f=1;next} /^###/{f=0} f' "$PLAN" | flat)
[ -n "${LIST// /}" ] || fail "no 'Explicitly not building' section in $PLAN — did the heading change?"

bad=0
for phrase in "${BUILT[@]}"; do
  grep -qF -- "$phrase" <<<"$WHOLE" \
    || fail "\"$phrase\" appears nowhere in $PLAN — this entry checks nothing, which is \
exactly how a check like this stops working"

  if grep -qF -- "$phrase" <<<"$LIST"; then
    echo "  x PLAN.md still refuses \"$phrase\", and it is built" >&2
    bad=1
  fi
done

[ "$bad" -eq 0 ] || fail "the plan and the repository disagree about what the product is — \
edit PLAN.md in the same commit as the feature, or do not land the feature"

if [ -n "$TYPES" ] && [ -n "$KINDS_PY" ]; then
  python3 "$KINDS_PY" "$TYPES" "$PLAN"
fi

echo "  ok  ${#BUILT[@]} built features, none still on the plan's refusal list"
