#!/usr/bin/env bash
# Fail if any Lean source contains an escape hatch that would make a green
# build meaningless.
#
# `set_option warningAsError true` (set in every source) already turns a
# `sorry` into a hard error — verified by probe: before it was set, a file
# proving `(1 : Int) = 2` by `sorry` passed `lean_test`. This catches the
# things that option does NOT:
#
#   axiom          — a declared axiom is not a warning; it is simply believed
#   native_decide  — trusts the compiler via the `Lean.ofReduceBool` axiom,
#                    which is a soundness escape hatch, not a proof
#   sorry / admit  — belt and braces, in case the option is ever removed
#
# Deliberately textual. The alternative is `#print axioms`, whose output is
# informational and does not fail a build — which is the exact class of
# mistake this file exists to prevent.
set -uo pipefail

status=0
found=0

for f in "$@"; do
  case "$f" in
    *audit_proofs.sh) continue ;;
  esac

  # Strip block comments and line comments before matching, so prose that
  # merely *discusses* `sorry` does not trip the gate.
  stripped=$(perl -0777 -pe 's{/-.*?-/}{}gs; s{--.*$}{}gm' "$f")

  while IFS= read -r pattern; do
    if printf '%s' "$stripped" | grep -qE "$pattern"; then
      echo "FAIL  $f  matches /$pattern/"
      status=1
      found=1
    fi
  done <<'PATTERNS'
(^|[^a-zA-Z_.])sorry([^a-zA-Z_]|$)
(^|[^a-zA-Z_.])admit([^a-zA-Z_]|$)
(^|[^a-zA-Z_.])native_decide([^a-zA-Z_]|$)
^[[:space:]]*axiom[[:space:]]
PATTERNS

  if ! grep -q 'set_option warningAsError true' "$f"; then
    echo "FAIL  $f  missing 'set_option warningAsError true'"
    status=1
    found=1
  fi
done

if [ "$found" -eq 0 ]; then
  echo "ok    $# Lean source(s): no sorry, admit, native_decide or axiom; all guarded"
fi
exit "$status"
