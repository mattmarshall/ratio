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
    ("Why this is acceptable", "an accepted explanation is never shown"),
    ("accepted by", "an explanation would appear with no name on it"),
    ("no longer stands", "a stale explanation would read as a current one"),
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
    # ── multi-view books ────────────────────────────────────────────────────
    # ⚠ Same discipline: these are the strings a reader looks for when asking
    # WHICH BOOK OF RECORD a figure came from. A console that stops saying is
    # the row already in HANDOFF.md's failure table — the console and the CLI
    # reporting different NAVs for one book, neither saying which.
    ("viewswitch", "the book-of-record switch is not rendered"),
    ("Book of record", "the view switch has no accessible name"),
    (
        "default",
        "an undeclared view would read as an elected basis — the lot-method trap again",
    ),
    ("journal order", "the recorded basis would print as a settlement convention"),
    ("settled T+", "a settlement view never says how many days it settles in"),
    ("Unplaceable", "entries a view cannot date would vanish from the screen"),
    (
        "Recognised through",
        "a book of record would not say the day it has recognised entries through",
    ),
    (
        "Neither view can place these",
        "the reconciliation would look fully explained when it is not",
    ),
    ("recognisedHere", "the in-flight entries are never read from the response"),
    ("unplaceable", "the entries neither view can place are never read"),
    ("Contributing", "the in-flight lists would not show that they add to the difference"),
    # ── the plan ────────────────────────────────────────────────────────────
    # ⚠ Every needle here is a phrase whose ABSENCE would leave a figure on the
    # screen with nothing qualifying it. A plan is a picture, and a picture is
    # exactly the kind of thing that keeps rendering after the sentence beside it
    # has been edited away.
    (
        "The strike as recorded",
        "the growing curve is gone — the plan would quote only the flat one, "
        "which is the overclaim `ratio bench` exists to make hard",
    ),
    (
        "The same figure off the maintained totals",
        "the flat curve is gone, and with it the whole scale argument",
    ),
    (
        "Folding every open tax lot",
        "the plan not taken stops being priced, so the chosen one has nothing to "
        "be cheaper THAN",
    ),
    (
        "Applying the open actions by rewriting the lots",
        "`Ratio.Closure.the_cliff` is gone — an outstanding action would look free "
        "under both plans",
    ),
    (
        "not measured",
        "an unmeasured step would render as a figure, and a reader cannot tell an "
        "estimate from a measurement",
    ),
    (
        "Nothing here has been measured",
        "the unanalyzed plan stops saying so, and its estimates read as timings",
    ),
    (
        "Open lots per security",
        "the fragmentation dial is gone — 500 x 40,000 and 10,000 x 2,000 are both "
        "twenty million lots and are not the same fund",
    ),
    (
        "planlist",
        "the plan exists only as a diagram, and a diagram nothing can read aloud "
        "is a figure that cannot be cited",
    ),
    (
        "prefers-reduced-motion",
        "the edge animation stops honouring a reader who asked for less motion",
    ),
    # ── the command palette ─────────────────────────────────────────────────
    # ⚠ Same discipline as the rest: each is a control or a phrase whose absence
    # would leave the palette looking like it works. A palette is especially
    # exposed to this — it renders nothing until somebody presses a key, so a
    # screen test that never opens it sees a perfectly healthy console.
    # ⚠ A CLASS NAME ALONE IS NOT ENOUGH HERE, AND WORKING OUT WHY IS WORTH
    # KEEPING. `cmdk` and `cmdrow` appear in BOTH the component and globals.css,
    # so deleting the component leaves the needle satisfied by the stylesheet —
    # the rule outlives the markup. Every control below therefore also carries a
    # needle that exists in its `.tsx` and nowhere else, which is the one that
    # actually goes red. (The same weakness applies to the older class needles
    # above; this is not a new problem, and these are the first ones written with
    # it in mind.)
    (
        "cmdk",
        "the ⌘K hint is gone from the header — a palette nobody is told about is "
        "a palette nobody opens, and nothing else on the screen mentions it",
    ),
    # ⚠ THE VALUE, NOT THE ATTRIBUTE NAME. `aria-keyshortcuts` alone also matches
    # the comment in `CommandHint.tsx` that explains why the attribute is there —
    # so deleting the attribute and keeping the prose passed. A needle satisfied
    # by the sentence describing the control is the purest form of the vacuous
    # green this file is against.
    (
        "Meta+K Control+K",
        "the hint stops announcing the chord, so the palette is discoverable by "
        "sight and by nothing else",
    ),
    ("cmdrow", "the palette renders no results"),
    (
        "Command palette",
        "the palette's dialog has no accessible name — kbar names neither it nor "
        "the input, so both are this console's job",
    ),
    (
        "Open by id",
        "the deep-link tier is gone, and a pasted break id matches nothing",
    ),
    # ⚠ THE FULL STOP IS LOAD-BEARING. The palette says "Nothing matches." and the
    # exceptions queue says "Nothing matches this filter." — so the phrase without
    # its period is satisfied by the queue, and the case would stay green with the
    # palette's empty state deleted. With it, this matches one file.
    (
        "Nothing matches.",
        "an empty palette reads as a broken one rather than as an answer",
    ),
    # ⛔ AND NO `startTransition` NEEDLE, THOUGH THE PALETTE NEEDS ONE AS MUCH AS
    # `FilterChips` DOES. Both that file and `PlanControls` already contain the
    # literal, so the case would pass with the palette's transition dropped — the
    # vacuous green this script exists to refuse. Nothing observable from the DOM
    # distinguishes a transitioned navigation from a bare one either, so this
    # property is held by the ⛔ in `usePaletteNavigator` and by review, and it is
    # better to say so than to add a check that reports on somebody else's file.
    # ⛔ AND NOT A `navStrikes` NEEDLE, THOUGH IT IS THE TEMPTING ONE. The wire
    # client already contains that literal, so the case would stay green with the
    # palette's collection→segment table deleted. `src/lib/deeplink.test.ts` holds
    # the rename instead, where an assertion can see it — and note that a needle
    # placed in THAT file would pass vacuously too, because the exclusion below is
    # `.test.tsx` only.
    # ── investment capital activity (#70) ───────────────────────────────────
    (
        "Capital activity is an Investment figure",
        "a personal or project book would inherit the fund capital screen",
    ),
    (
        "partners plus unallocated activity — not a return, not attribution",
        "the capital figure would stop saying it is not a return",
    ),
    (
        "who put money in and took money out, not IRR",
        "the capital figure would read as performance reporting",
    ),
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
