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
         "technical", "team"]

# The practitioner page sells outcomes. Every one of these belongs on
# platform.html or technical.html instead. See site/README.md.
BANNED_ON_INDEX = [
    "Lean", "theorem", "conservation", "conserve", "kernel", "gRPC", "Rust",
    "monoidal", "machine-checked", "formally verified", "content-addressed",
    "bit-exact", "append-only", "CUDA", "GPU", "associative",
]

# Ratio is source-available under strong copyleft with a commercial license, and
# is NOT open source. The distinction is commercially load-bearing, and prose
# drifts back toward the familiar phrase without anyone noticing in a diff — so
# it is asserted rather than left to review. See site/README.md.
BANNED_EVERYWHERE = [
    "open source", "open-source", "MIT licen", "open core", "self-host",
    "permissively licensed",   # only ever correct as "NOT permissively licensed"
]
# Phrases that legitimately contain a banned substring.
LICENSE_EXCEPTIONS = ["not permissively licensed"]

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

    for e in errors:
        print(e)
    print(f"\n{len(errors)} problem(s)" if errors else "\nall checks passed")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
