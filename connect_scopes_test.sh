#!/usr/bin/env bash
#
# The Connect catalog is a contract. This checks the three documents that
# name it cannot drift apart in silence.
#
# ⛔ IT DOES NOT SAY LIVE OAUTH EXISTS. A green run means PLAN.md,
# HANDOFF.md and docs/connect-scopes.md still agree on the frozen strings
# and the leftovers. The in-process authorizer accepts catalog scopes;
# #150 stays open for the allowlist / reserved RPCs / reference skeleton.
# leftover #22 is the shared demo dial, unused Cognito resources, and
# live provider OAuth.
#
# ⚠ THE SAME LIMITATION AS //:plan_refusals_test. This is a cross-check
# between DECLARED lists. It does not scan the tree for an authorizer.
# Adding a scope to the catalog without adding it here (or the reverse)
# is the moment somebody notices.
set -euo pipefail

CATALOG="$1"
PLAN="$2"
HANDOFF="$3"

fail() { echo "  x $*" >&2; exit 1; }

flat() { tr '\n' ' ' | tr -s ' '; }

CAT=$(flat <"$CATALOG")
WHOLE_PLAN=$(flat <"$PLAN")
WHOLE_HANDOFF=$(flat <"$HANDOFF")

[ -n "${CAT// /}" ] || fail "$CATALOG is empty — this check would pass for any list"
[ -n "${WHOLE_PLAN// /}" ] || fail "$PLAN is empty"
[ -n "${WHOLE_HANDOFF// /}" ] || fail "$HANDOFF is empty"

# Frozen grantable scopes. One missing from the catalog is a silent rename.
FROZEN=(
  "books:read"
  "books:write"
  "books:ingest"
  "journals:read"
  "journals:post"
  "statements:read"
  "views:read"
  "positions:read"
  "lots:read"
  "lots:elect"
  "nav:read"
  "nav:strike"
  "partners:read"
  "partners:write"
  "capital:read"
  "commits:read"
  "calls:post"
  "fees:read"
  "fees:accrue"
  "budget:read"
  "billing:read"
  "breaks:read"
  "breaks:explain"
  "closes:read"
  "config:read"
  "audit:export"
  "deliveries:write"
  "facts:admit"
  "webhooks:journal"
)

# Named so they stop being tempting. The catalog must refuse them; PLAN
# must name the first two and portal impersonation.
HARD_NON=(
  "rules:approve"
  "config:promote"
)

# Body/comment near-misses. Granting these would be two names for one door.
ALIASES=(
  "journal:read"
  "journal:append"
  "projects:budget:read"
  "projects:billing:read"
)

bad=0
for scope in "${FROZEN[@]}"; do
  grep -qF -- "\`$scope\`" "$CATALOG" \
    || { echo "  x catalog does not freeze \`$scope\`" >&2; bad=1; }
done

for scope in "${HARD_NON[@]}"; do
  grep -qF -- "\`$scope\`" "$CATALOG" \
    || { echo "  x catalog does not name hard non-scope \`$scope\`" >&2; bad=1; }
  grep -qF -- "$scope" <<<"$WHOLE_PLAN" \
    || { echo "  x PLAN.md does not name hard non-scope $scope" >&2; bad=1; }
done

for alias in "${ALIASES[@]}"; do
  grep -qF -- "\`$alias\`" "$CATALOG" \
    || { echo "  x catalog dropped alias \`$alias\` — a near-miss would pass" >&2; bad=1; }
done

# The three documents must point at each other. A catalog nobody can find
# is the same class of defect as a plan that was not amended.
for phrase in "docs/connect-scopes.md" "#150"; do
  grep -qF -- "$phrase" <<<"$WHOLE_PLAN" \
    || { echo "  x PLAN.md does not mention $phrase" >&2; bad=1; }
  grep -qF -- "$phrase" <<<"$WHOLE_HANDOFF" \
    || { echo "  x HANDOFF.md does not mention $phrase" >&2; bad=1; }
done

grep -qF -- "portal impersonation" <<<"$WHOLE_PLAN" \
  || { echo "  x PLAN.md dropped portal impersonation from the hard non-scopes" >&2; bad=1; }

# Honesty: leftovers stay leftovers. The authorizer accepting catalog
# scopes is Built; #150 stays open for the allowlist and reserved RPCs.
for phrase in "Connect tokens accepted with catalog scopes" "#151" "leftover #22" "does not close #150"; do
  grep -qF -- "$phrase" "$CATALOG" \
    || { echo "  x catalog is missing leftover honesty: $phrase" >&2; bad=1; }
done

grep -qF -- "does not close #150" <<<"$WHOLE_PLAN" \
  || { echo "  x PLAN.md must say it does not close #150" >&2; bad=1; }

# #177: equalization / drip / side-pocket stay Connect, not kernel
# primitives. A catalog or plan that drops the decision is the same
# lag as a scope rename nobody noticed.
for phrase in "equalization, drip, and side-pocket stay Connect" "#177"; do
  grep -qiF -- "$phrase" <<<"$CAT" \
    || { echo "  x catalog dropped $phrase" >&2; bad=1; }
  grep -qiF -- "$phrase" <<<"$WHOLE_PLAN" \
    || { echo "  x PLAN.md dropped $phrase" >&2; bad=1; }
  grep -qiF -- "$phrase" <<<"$WHOLE_HANDOFF" \
    || { echo "  x HANDOFF.md dropped $phrase" >&2; bad=1; }
done

# ⛔ lots:elect is SpecID names, not a Method / Order. The same sentence
# this repository uses for MinTax / SpecID / average cost.
grep -qF -- 'lot_method = "specific_id"' "$CATALOG" \
  || { echo "  x catalog must refuse lot_method = \"specific_id\" as lots:elect" >&2; bad=1; }

[ "$bad" -eq 0 ] || fail "the Connect catalog and the plan no longer agree"

echo "  ok  ${#FROZEN[@]} frozen scopes, ${#HARD_NON[@]} hard non-scopes, leftovers still named"
