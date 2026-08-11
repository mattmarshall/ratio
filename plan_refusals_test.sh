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
set -euo pipefail

PLAN="$1"

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

echo "  ok  ${#BUILT[@]} built features, none still on the plan's refusal list"
