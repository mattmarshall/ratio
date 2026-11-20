#!/usr/bin/env bash
#
# The console's six source-text checks, exactly as console.yml runs them.
#
# Usage:  .agents/skills/console/scripts/contract-checks.sh
#
# They need only Python 3, not Node, so they run in any container even when
# `pnpm check` cannot. They are NOT the whole console gate: tsc, the render
# suite, `next build` and the phone check also run in console.yml (see the
# console skill). Keep this list in step with that workflow's "The contract
# reaches every screen" step.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)"

fail=0
run() {
  local name="$1"; shift
  if out=$(python3 "$@" 2>&1); then
    echo "  ok    $name"
  else
    echo "  FAIL  $name"
    sed 's/^/        /' <<<"$out"
    fail=1
  fi
}

run "route manifest: the console calls exactly the contract's routes" \
  console/scripts/route_manifest_test.py \
  proto/ratio/console/v1/console.proto console/src/wire/client.ts console/src/routes.ts
run "fixtures match the proto" \
  console/scripts/fixtures_test.py proto/ratio/console/v1/console.proto console/fixtures/funds.json
run "screens still read the fields that have shipped missing" \
  console/scripts/fields_test.py console/src/routes.ts
run "design tokens agree with site/style.css" \
  console/scripts/tokens_test.py site/style.css console/src/app/globals.css
run "nothing that belongs in the environment is committed" \
  console/scripts/no_secrets_test.py console/src/routes.ts
run "every BookKind is named in PLAN.md" \
  console/scripts/book_kinds_in_plan_test.py console/src/wire/types.ts PLAN.md

exit "$fail"
