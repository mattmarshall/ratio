#!/usr/bin/env python3
"""Assert that a built site is complete, self-contained and on-register.

    python3 site/verify.py _site

Run by CI on every push and pull request, and worth running by hand before
publishing anywhere. Exits non-zero and prints GitHub Actions error
annotations on failure.

These are not style checks. Each one corresponds to a way this site has
actually broken, or would break silently:

  * an unsubstituted placeholder renders as literal underscores, or — worse —
    a duplicated one inlines a second copy of every font into a comment
  * a missing doctype puts the page in quirks mode at 980px on a phone
  * an external subresource is invisible locally and blocked outright by the
    Artifact CSP, so the page silently loses a font or an image
  * a page written for fund accountants that acquires the word "kernel" has
    stopped doing its job, and nobody will notice from a diff
"""

import html
import pathlib
import re
import sys

PAGES = ["index", "workflows", "platform", "compliance", "roadmap",
         "technical"]   # "team" withheld — see build.py

# The practitioner page sells outcomes. Every one of these belongs on
# platform.html or technical.html instead. See site/README.md.
BANNED_ON_INDEX = [
    "Lean", "theorem", "conservation", "conserve", "kernel", "gRPC", "Rust",
    "monoidal", "machine-checked", "formally verified", "content-addressed",
    "bit-exact", "append-only", "CUDA", "GPU", "associative",
]

# Ratio is AGPL-3.0 with a commercial license alongside it. What must not appear
# is any phrasing that implies the PERMISSIVE grant it used to carry — prose
# drifts back toward the familiar words without anyone noticing in a diff, so it
# is asserted rather than left to review. See site/README.md and LICENSING.md.
#
# ⛔ "open source" WAS ON THIS LIST AND HAS BEEN REMOVED. The license is now
# AGPL-3.0, which is OSI- and FSF-approved: banning the phrase would have made
# this check enforce a false statement about our own license. The distinction
# that is actually commercially load-bearing is reciprocity — you may read, run
# and modify it, and §13 means you may not take it private — not openness.
BANNED_EVERYWHERE = [
    "MIT licen", "open core",
    "permissively licensed",   # only ever correct as "NOT permissively licensed"
    "use it however you like",
]
# Phrases that legitimately contain a banned substring.
LICENSE_EXCEPTIONS = ["not permissively licensed"]

# ⛔ ISSUE #67. The roadmap's "not yet built" column listed persistence,
# multi-currency, tax lots and corporate actions after PLAN.md had marked
# them done. That is the same class of defect as the refusal list lagging
# the repo: two documents disagreed, and nothing said so.
#
# Each PLAN phrase must still appear in PLAN.md, or this check is vacuous —
# the same failure plan_refusals_test.sh names. Each page phrase must appear
# in the built column and must not appear in the spec column.
#
# ⚠ POSTGRES IS THE EXPLICIT EXCEPTION. PLAN's "four of these were built"
# table includes Postgres as "spec only"; Stage E is still open. Requiring
# the page to call Postgres built would enforce a lie. It belongs on the
# spec side, and must not be claimed as a running engine.
PLAN_MARKS_ENGINE_DONE = [
    "tax lots and cost basis",
    "multi-currency and FX",
    "corporate actions",
    "Built with no database at all",
    # ⚠ #133 / #138 landed the wash window and the Rust engine; PLAN
    # named tax lots and said nothing. Same class of defect as #67.
    "wash sales",
    # ⚠ #141 landed MinTax as a ranking at a price; PLAN named it in
    # the same-commit amendment. The public roadmap lagged. Same class
    # of defect as #67 / wash sales.
    "MinTax is a ranking at a price",
    # ⚠ #143 landed SpecID as a named selection; PLAN named it in
    # the same-commit amendment. The public roadmap lagged. Same class
    # of defect as #67 / wash sales / MinTax.
    "SpecID is a named selection",
    # ⚠ Average cost landed as a pool, not a Method; PLAN named it in
    # the same-commit amendment.
    "Average cost is a pool",
    # ⚠ WashRestatement landed as a citeable record; PLAN named it in
    # the same-commit amendment.
    "WashRestatement is a citeable record",
    # ⚠ The non-US holding-period variant landed as an election; PLAN
    # named it in the same-commit amendment.
    "the non-US holding-period variant is an election",
    # ⚠ The pooled holding-period category landed as a date, not a
    # Method; PLAN named it in the same-commit amendment.
    "the pooled holding-period category is a date",
]
ROADMAP_ENGINE_BUILT = [
    "append-only journal",
    "tax lots",
    "FX translation",
    "corporate actions",
    "persistence without a database",
    "ConfigStore",
    "fact plane",
    "wash sales",
    "MinTax is a ranking at a price",
    "SpecID is a named selection",
    "Average cost is a pool",
    "WashRestatement is a citeable record",
    "the non-US holding-period variant is an election",
    "the pooled holding-period category is a date",
]
ROADMAP_ENGINE_NOT_BUILT = [
    "Postgres",
]
# An open phase-one deliverable that still names one of these is the
# original defect in checklist form.
ROADMAP_PHASE_ONE_MUST_NOT_STAY_OPEN = [
    "tax lots",
    "multi-currency",
    "corporate actions",
    "persistence without a database",
    "Control plane",
    "Fact plane",
]

MIN_KB, MAX_KB = 60, 2048

errors: list[str] = []


def err(file: str, msg: str) -> None:
    errors.append(f"::error file={file}::{msg}")


def visible_text(doc: str) -> str:
    doc = re.sub(r"<style>.*?</style>", " ", doc, flags=re.S)
    doc = re.sub(r"<!--.*?-->", " ", doc, flags=re.S)
    return html.unescape(re.sub(r"<[^>]+>", " ", doc))


def main() -> int:
    out = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "_site")
    if not out.is_dir():
        print(f"::error::{out} is not a directory")
        return 1

    for slug in PAGES:
        f = out / f"{slug}.html"
        name = str(f)
        if not f.is_file():
            err(name, "page was not produced")
            continue
        doc = f.read_text()

        stray = sorted(set(re.findall(r"__[A-Z0-9_]+__", doc)))
        if stray:
            err(name, f"unsubstituted placeholder: {' '.join(stray)}")

        if not doc.startswith("<!doctype html>"):
            err(name, "no doctype — the page would render in quirks mode")
        if 'name="viewport"' not in doc:
            err(name, "no viewport meta — lays out at 980px on a phone")
        n_title = doc.count("<title>")
        if n_title != 1:
            err(name, f"expected exactly one <title>, found {n_title}")

        # Self-contained: nothing may be LOADED from another host, because the
        # Artifact CSP blocks it and the page would silently lose a font or an
        # image. A plain <a href> to another site is prose, not a subresource,
        # and is fine — so this checks what actually fetches rather than keeping
        # an allowlist of hosts we happen to link to.
        external = set(re.findall(r'\ssrc="(https?://[^"]*)"', doc))
        external |= {m.group(1) for m in
                     re.finditer(r'<link\b[^>]*\shref="(https?://[^"]*)"', doc, re.I)}
        external |= set(re.findall(r'url\(\s*["\']?(https?://[^)"\']*)', doc, re.I))
        external |= set(re.findall(r'@import\s+["\'](https?://[^"\']*)', doc, re.I))
        if external:
            err(name, f"external subresource: {', '.join(sorted(external))}")

        n_fonts = doc.count("data:font/woff2")
        if n_fonts != 3:
            err(name, f"expected 3 inlined faces, found {n_fonts}")

        # The stylesheet has to actually PARSE, which nothing above checks.
        # A dropped brace shipped once: an edit to the palette ate the closing
        # brace of `@media (prefers-color-scheme:dark)`, so the whole rest of
        # the stylesheet nested inside it and the site rendered completely
        # unstyled in light mode. Every check here passed — the bytes were
        # present, the fonts inlined, the size plausible — because none of them
        # looked at structure.
        for css in re.findall(r"<style>(.*?)</style>", doc, re.S):
            depth = 0
            for ch in css:
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth < 0:
                        break
            if depth != 0:
                err(name, f"CSS braces unbalanced by {depth:+d} — a rule or "
                          "at-rule is unclosed, which silently swallows "
                          "everything after it")

            # Top-level rule count. The real stylesheet has well over a hundred;
            # if a stray at-rule has captured the file this collapses to a
            # handful, which is the shape the dropped brace produced.
            top = depth = 0
            for ch in css:
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        top += 1
            if top < 60:
                err(name, f"only {top} top-level CSS rules — expected 100+; an "
                          "at-rule has probably swallowed the stylesheet")

        kb = len(doc.encode()) // 1024
        if not MIN_KB < kb < MAX_KB:
            err(name, f"implausible size {kb}KB (expected {MIN_KB}–{MAX_KB})")

        for link in sorted(set(re.findall(r'href="([a-z]+\.html)"', doc))):
            if not (out / link).is_file():
                err(name, f"broken internal link -> {link}")

        # Collapse whitespace first: these phrases wrap across source lines, so
        # matching against the raw text misses the exceptions and fires falsely.
        text = re.sub(r"\s+", " ", visible_text(doc))
        for phrase in LICENSE_EXCEPTIONS:
            text = re.sub(re.escape(phrase), " ", text, flags=re.I)
        found = sorted({p for p in BANNED_EVERYWHERE if re.search(re.escape(p), text, re.I)})
        if found:
            err(name, f"licensing language: {', '.join(found)} — Ratio is "
                      "source-available under copyleft, not open source; see site/README.md")

        # An unfilled placeholder must never reach a published page. The team
        # page ships with TODO markers on purpose — real people's roles are not
        # something to invent — and this is what stops it going out that way.
        todos = len(re.findall(r"TODO", text))
        if todos:
            err(name, f"{todos} unfilled TODO placeholder(s) — this page is not "
                      "ready to publish")

        print(f"  ok  {slug}.html  {kb}KB")

    index = out / "index.html"
    if index.is_file():
        text = visible_text(index.read_text())
        hits = [
            f"{w}×{n}" for w in BANNED_ON_INDEX
            if (n := len(re.findall(r"(?<![A-Za-z])" + re.escape(w), text, re.I)))
        ]
        if hits:
            err(
                "site/index.src.html",
                "technical vocabulary on the practitioner page: "
                + ", ".join(hits)
                + " — this page is written for fund accountants; move it to "
                  "platform.src.html or technical.src.html",
            )
        else:
            print("  ok  index.html carries no technical vocabulary")

    check_roadmap_against_plan(out)

    for e in errors:
        print(e)
    print(f"\n{len(errors)} problem(s)" if errors else "\nall checks passed")
    return 1 if errors else 0


def column_text(doc: str, cls: str) -> str:
    m = re.search(rf'<div class="{cls}">(.*?)</div>', doc, flags=re.S)
    return re.sub(r"\s+", " ", visible_text(m.group(1))) if m else ""


def flatten(text: str) -> str:
    return re.sub(r"\s+", " ", text)


def check_roadmap_against_plan(out: pathlib.Path) -> None:
    """The status columns must not contradict PLAN.md on engine work."""
    roadmap = out / "roadmap.html"
    if not roadmap.is_file():
        return
    plan_path = pathlib.Path(__file__).resolve().parent.parent / "PLAN.md"
    src = "site/roadmap.src.html"
    if not plan_path.is_file():
        err(src, f"PLAN.md not found at {plan_path} — this check would pass "
                 "for any columns, which is how a check like this stops working")
        return

    plan = flatten(plan_path.read_text())
    missing_plan = [p for p in PLAN_MARKS_ENGINE_DONE if p not in plan]
    if missing_plan:
        err("PLAN.md", "engine-done phrase missing, so the roadmap check "
                       f"would assert nothing: {', '.join(missing_plan)}")
        return

    doc = roadmap.read_text()
    built = column_text(doc, "r-built")
    spec = column_text(doc, "r-spec")
    if not built or not spec:
        err(src, "roadmap is missing the built or spec status column")
        return

    for phrase in ROADMAP_ENGINE_BUILT:
        if not re.search(re.escape(phrase), built, re.I):
            err(src, f"built column does not mention \"{phrase}\", which "
                     "PLAN.md marks done")
        if re.search(re.escape(phrase), spec, re.I):
            err(src, f"spec column still lists \"{phrase}\" as not yet built, "
                     "and PLAN.md marks it done")

    for phrase in ROADMAP_ENGINE_NOT_BUILT:
        if re.search(re.escape(phrase), built, re.I):
            err(src, f"built column claims \"{phrase}\", which is still "
                     "Stage E / spec-only — see PLAN.md and issue #8")
        if not re.search(re.escape(phrase), spec, re.I):
            err(src, f"spec column does not mention \"{phrase}\", so the "
                     "exception that keeps Stage E honest has nowhere to sit")

    # Phase-one open bullets. class="o" is the hollow marker; a done item
    # that still carries it is the checklist form of the same lag.
    open_items = [
        flatten(html.unescape(re.sub(r"<[^>]+>", " ", item)))
        for item in re.findall(r'<li class="o">(.*?)</li>', doc, flags=re.S)
    ]
    for phrase in ROADMAP_PHASE_ONE_MUST_NOT_STAY_OPEN:
        hits = [item for item in open_items if re.search(re.escape(phrase), item, re.I)]
        if hits:
            err(src, f"phase-one checklist still marks \"{phrase}\" open: "
                     + "; ".join(hits))

    # Phase-four marketplace is a Connect catalog, not kernel RPC sprawl.
    # A public page that drops the pointer is the same lag as the built
    # column forgetting tax lots — two documents disagree, nothing says so.
    # This needle lives in verify.py (not only the HTML) so a markdown-only
    # follow-up cannot skip the site workflow while the pointer drifts.
    phase_four_needles = (
        "WorkOS Connect",
        "#150",
        "connect-scopes.md",
        # #165 landed a Personal bank-feed scaffold in this tree. A public
        # page that drops the pointer while PLAN records the app is the
        # same lag as the built column forgetting tax lots.
        "#165",
        "bank-feed",
        # #166 landed a Personal tax-pack scaffold. Same pointer rule.
        "#166",
        "tax-pack",
        # #168 landed a Personal net-worth goals scaffold. Same pointer rule.
        "#168",
        "connect/goals",
        # #184 landed a Project AIA pay-app scaffold. Same pointer rule.
        "#184",
        "aia-pay-app",
        # #172 landed a Project vendor / GC portal scaffold. Same pointer rule.
        "#172",
        "vendor-portal",
    )
    if not all(n in doc for n in phase_four_needles):
        missing = [n for n in phase_four_needles if n not in doc]
        err(src, "phase four does not point at the Connect catalog "
                 f"({', '.join(missing)}); PLAN.md amendments 2026-09-04 "
                 "and issues #150 / #165 / #166 / #168 / #172 / #184")
    elif not any(e.startswith(f"::error file={src}::") or
                 e.startswith("::error file=PLAN.md::") for e in errors):
        print("  ok  roadmap status columns agree with PLAN.md on engine work")


if __name__ == "__main__":
    sys.exit(main())
