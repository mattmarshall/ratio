#!/usr/bin/env python3
"""The crude half of what `//web:rendered_test` did, kept deliberately crude.

⛔ THE GAP THIS CLOSES IS RECORDED IN THE HISTORY. A field can be declared,
transcoded, served, typechecked and mirrored while NO COMPONENT READS IT. That
has happened here twice: the corporate-actions screen was "written, compiled,
and absent", and the per-currency split was found by grepping served HTML by
hand. `//proto:mirrors_test` proves the types match the contract and says
nothing about whether a screen shows the value.

⚠ AND THIS IS NOT THE WHOLE REPLACEMENT. `console/src/app/screens.test.tsx`
RENDERS the screens against fixtures and asserts the figures appear, which is
the half a grep cannot do. This is the other half, and the two catch different
things: a render test sees a component that renders nothing, and a source sweep
sees a component nothing renders — a screen deleted from the route tree, or a
phrase quietly dropped from a page.

The needles come from two places, and both are inherited rather than invented:

  * `web/rendered_test.sh`, which named the fields whose absence had already
    shipped once.
  * `//crates/ratio:ratio_test`'s `the_served_console_carries_the_lot_engine`,
    which asserted eleven literals against the console embedded in the binary.
    That test could not survive the console leaving the binary; its needles had
    to, so they are here.

⛔ THE PATH IS DERIVED FROM A FILE, NEVER FROM THE WORKING DIRECTORY. Under
Bazel a test runs from the runfiles root rather than its package, so a relative
`console/src` finds nothing — and "nothing" is how this test passes vacuously.
The anchor is `src/routes.ts`; the tree scanned is its parent.

Run: fields_test.py <console/src/routes.ts>
"""

import sys
from pathlib import Path

# (needle, what its absence would mean)
NEEDLES: list[tuple[str, str]] = [
    # ── from web/rendered_test.sh ────────────────────────────────────────────
    ("ccyrow", "the per-currency split is not rendered"),
    ("currencyTotals", "the per-currency split is never read from the response"),
    ("lotrow", "the lot rows are not rendered"),
    ("openLotCount", "the open-lot count is never read"),
    ("realizedGain", "the realized gain is never read from the response"),
    ("historyIntact", "the replay proof is not rendered"),
    ("qualification", "a strike qualification would not be shown"),
    ("blocksNav", "the bounds a break's severity was graded against are never read"),
    ("graded at", "a severity would be shown with nothing behind it"),
    ("signin-btn", "the sign-in prompt is not rendered"),
    ("Sign out", "the signed-in principal / sign-out control is not rendered"),
    # ── from //crates/ratio:ratio_test, whose console is gone ────────────────
    # ⚠ These are the strings a reader looks for. If the wording changes, change
    # it here too — that is the point, not an inconvenience.
    ("Lot method", "the lot method row is gone"),
    (
        "a term of the administration agreement",
        "the elected-method claim is gone — half the row shipped once already",
    ),
    (
        "this configuration declares no method",
        "the DEFAULTED-method claim is gone, so a defaulted method would read as elected",
    ),
    ("Realized gain", "the realized gain row is gone"),
    ("Short-term", "the short-term split is gone"),
    ("Long-term", "the long-term split is gone"),
    ("Unclassified", "the unclassified residue is gone — the three parts stop adding up"),
    ("Basis relieved", "the basis relieved is gone"),
    ("no trade date", "a lot that cannot be classified would show a guessed date"),
    ("Tax lots", "the lot book is gone"),
    ("the fold that", "the scale claim is gone — it lived only in a benchmark once"),
]


def main() -> None:
    root = Path(sys.argv[1]).parent
    sources = [
        p
        for p in root.rglob("*")
        if p.suffix in {".ts", ".tsx", ".css"} and not p.name.endswith(".test.tsx")
    ]
    if not sources:
        sys.exit("::error::no sources found — this would pass vacuously")
    blob = "\n".join(p.read_text() for p in sources)

    missing = [(n, why) for n, why in NEEDLES if n not in blob]
    for n, why in missing:
        print(f"::error::{why} — {n!r} is nowhere in the console")
    if missing:
        sys.exit(f"\n{len(missing)} field(s) or phrase(s) no longer in the console")
    print(f"  ok  the console reads and renders every one of {len(NEEDLES)} checked")


if __name__ == "__main__":
    main()
