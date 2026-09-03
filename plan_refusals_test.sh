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
