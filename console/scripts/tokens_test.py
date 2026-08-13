#!/usr/bin/env python3
"""One design system, not two.

`console/src/app/globals.css` opens by saying so: "The site's tokens, unchanged.
site/style.css is the source; this page is a consumer of that system rather than
a second one." Nothing checked it, and a claim in a comment is the shape of
thing this repository has twice found to be false.

⚠ THE SHARED SUBSET ONLY. The console legitimately defines tokens the marketing
site has no use for — `--good`, `--warn`, `--crit`, `--block`, `--shadow` — and
the site defines `--accent-hover`, which no console control uses. What must not
drift is what both declare: a `--accent` that means one green on a landing page
and another in an operations console is two brands.

Run: tokens_test.py <site/style.css> <console/src/app/globals.css>
"""

import re
import sys
from pathlib import Path


def tokens(text: str) -> dict[str, str]:
    """Custom property -> the value in the FIRST (light) declaration."""
    out: dict[str, str] = {}
    for m in re.finditer(r"(--[\w-]+)\s*:\s*([^;}]+)", text):
        out.setdefault(m.group(1), m.group(2).strip().lower())
    return out


def main() -> None:
    site = tokens(Path(sys.argv[1]).read_text())
    console = tokens(Path(sys.argv[2]).read_text())
    if not site or not console:
        sys.exit("::error::no custom properties found — this would pass vacuously")

    shared = sorted(set(site) & set(console))
    if not shared:
        sys.exit("::error::the two stylesheets share no tokens — one of the paths is wrong")

    problems = [
        f"{name}: the site says {site[name]}, the console says {console[name]}"
        for name in shared
        if site[name] != console[name]
    ]
    if problems:
        for p in problems:
            print(f"::error::{p}")
        sys.exit(
            f"\n{len(problems)} token(s) drifted — the console is a CONSUMER of "
            f"site/style.css, not a second design system"
        )
    print(f"  ok  {len(shared)} shared token(s), all agreeing with site/style.css")


if __name__ == "__main__":
    main()
