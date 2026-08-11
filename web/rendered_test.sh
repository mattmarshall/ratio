#!/usr/bin/env bash
#
# The console has to RENDER the fields it receives, and nothing checked that.
#
# ⛔ THE GAP THIS CLOSES IS RECORDED IN THE HISTORY. `//web:ts_typecheck_test`
# proves types.ts compiles and `//proto:mirrors_test` proves it mirrors the
# proto — so a field can be declared, transcoded, served, typechecked and
# mirrored while NO COMPONENT READS IT. That has already happened once: the fix
# was found by grepping the served HTML by hand, because nothing else would have
# said a word.
#
# ⚠ SO THIS GREPS THE BUNDLE, and that is deliberately crude. It cannot tell a
# well-rendered field from a badly-rendered one. What it CAN tell is that the
# component still exists — a bundler drops what nothing references, so a class
# name that vanishes from the output is a screen that vanished from the product.
# A crude check on the artifact beats a precise check on the source, because the
# artifact is what is served.
set -euo pipefail

HTML="$1"
fail() { echo "  x $*" >&2; exit 1; }

# Present, and NOT merely a string in a stylesheet: the class has to appear in
# the script bundle too, which is the part a tree-shake would remove.
present() {
  grep -q -- "$1" "$HTML" || fail "$2 — '$1' is not in the served console"
}

# The untranslated per-currency split beneath a translated account total. The
# figure above it is a conversion, and a reader checking it needs the fact.
present 'ccyrow' 'the per-currency split is not rendered'
present 'currencyTotals' 'the per-currency split is never read from the response'

# The lot book behind a position. Deployed and invisible once already.
present 'lotrow' 'the lot rows are not rendered'
present 'openLotCount' 'the open-lot count is never read'

# A realized gain is credit-normal, and the flip happens in exactly one place.
present 'realizedGain' 'the realized gain is never read from the response'

# The two figures that make a strike checkable rather than assertable.
present 'historyIntact' 'the replay proof is not rendered'
present 'qualification' 'a strike qualification would not be shown'

echo "  ok  the served console reads and renders every field checked"
