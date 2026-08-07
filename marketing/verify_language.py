#!/usr/bin/env python3
"""Assert the LaTeX says what the site says about licensing and the stack.

`site/verify.py` has enforced this on the web pages since the positioning
changed. The LaTeX had no such check, and it drifted: the brief shipped
"open-source", "self-host" and "Rust/Python" for months after the decision to
be source-available-and-commercially-licensed, and after the last Python left
the stack. A prospect reading the brief and the site got two different offers.

Run over any tree of .tex:

    python3 marketing/verify_language.py marketing competitive paper

No dependencies, no network, and the same checks a developer and CI run — an
inline shell version of these drifted from what ran locally once already.
"""

import pathlib
import re
import sys

# Ratio is source-available under strong copyleft with a commercial license for
# operators. It is NOT open source and NOT self-hostable. The distinction is
# commercially load-bearing, and prose drifts back toward the familiar phrase
# without anyone noticing in a diff.
BANNED = [
    ("open source", "Ratio is source-available, not open source"),
    ("open-source", "Ratio is source-available, not open source"),
    ("open core", "say 'auditable core' or 'source-available core'"),
    ("self-host", "operating the platform is a commercial arrangement"),
    ("MIT licen", "the MIT license was dropped"),
    ("permissively licensed", "only ever correct as 'not permissively licensed'"),
    # The stack is Lean-authored and Rust-emitted. There is no Python in it.
    ("Rust/Python", "there is no Python in the stack"),
    ("Rust / Python", "there is no Python in the stack"),
    # `meridian` is a different project and does not belong in Ratio's diagrams.
    ("meridian", "meridian is a separate project, not part of Ratio"),
    # Internal fleet naming means nothing to a prospect.
    ("fastverk", "internal fleet naming does not belong in buyer-facing material"),
]

# Phrases that legitimately contain a banned substring.
EXCEPTIONS = [
    "not permissively licensed",
    "is not open source",
    "not open source",
]


def strip_comments(text: str) -> str:
    """Drop LaTeX comments, so a note explaining a ban is not itself a hit.

    A `%` escaped as `\\%` is content, not a comment.
    """
    out = []
    for line in text.splitlines():
        cut = None
        for i, ch in enumerate(line):
            if ch == "%" and (i == 0 or line[i - 1] != "\\"):
                cut = i
                break
        out.append(line if cut is None else line[:cut])
    return "\n".join(out)


def main() -> None:
    roots = sys.argv[1:] or ["marketing"]
    files = sorted(
        f for root in roots for f in pathlib.Path(root).rglob("*.tex")
    )
    if not files:
        sys.exit(f"no .tex found under {roots} — wrong directory?")

    errors = []
    for f in files:
        text = strip_comments(f.read_text(encoding="utf-8"))
        # Collapse newlines so a phrase broken across a line wrap still matches;
        # LaTeX rewraps constantly and a check that only sees single lines would
        # pass on exactly the edits most likely to reintroduce a banned phrase.
        #
        # Join at a trailing hyphen FIRST. `self-\nhostable` collapses to
        # "self- hostable" under a plain whitespace squeeze and then matches
        # nothing — which is how the first version of this check passed a
        # deliberately planted "self-hostable".
        joined = re.sub(r"-[ \t]*\n[ \t]*", "-", text)
        flat = re.sub(r"\s+", " ", joined)
        low = flat.lower()
        for phrase, why in BANNED:
            start = 0
            while (i := low.find(phrase.lower(), start)) != -1:
                start = i + 1
                window = low[max(0, i - 40) : i + len(phrase) + 40]
                if any(e in window for e in EXCEPTIONS):
                    continue
                context = flat[max(0, i - 50) : i + len(phrase) + 50].strip()
                errors.append(f"{f}: {phrase!r} — {why}\n      …{context}…")

    print(f"checked {len(files)} .tex file(s) under {', '.join(roots)}")
    if errors:
        for e in errors:
            print(f"::error::{e}")
        sys.exit(f"\n{len(errors)} banned phrase(s)")
    print("all checks passed")


if __name__ == "__main__":
    main()
