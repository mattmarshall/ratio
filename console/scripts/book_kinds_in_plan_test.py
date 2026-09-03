#!/usr/bin/env python3
"""Every BookKind the console offers must be named in PLAN.md.

⛔ THE REFUSAL-LIST CHECK CANNOT SEE THIS. `plan_refusals_test.sh` does not
scan the tree, and a kind nobody adds to `BUILT` stays invisible. PERSONAL
shipped that way: first-class Books landed, the roadmap still read as a
fund-admin Cognito demo, and nothing went red.

So this is a cross-check between two DECLARED lists — the `BookKind` union
in `console/src/wire/types.ts`, and the roadmap — and its honest limitation
is the same as the refusal check: a kind not in the union is not checked.

⚠ THE SPELLING IS THE POINT. A near-miss (`Personal` for `PERSONAL`) checks
nothing, which is how OPERATING would have drifted the same way. The
amendment names the wire tokens on purpose.

Run: book_kinds_in_plan_test.py <console/src/wire/types.ts> <PLAN.md>
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def kinds_of(types: str) -> list[str]:
    match = re.search(r"export type BookKind\s*=\s*([^;]+);", types, re.S)
    if not match:
        print("  x no `export type BookKind` in the wire types — did the name change?", file=sys.stderr)
        sys.exit(1)
    found = re.findall(r'"([A-Z]+)"', match.group(1))
    if not found:
        print("  x BookKind union is empty — a check that sees no kinds cannot fail", file=sys.stderr)
        sys.exit(1)
    return found


def main() -> None:
    if len(sys.argv) != 3:
        print("usage: book_kinds_in_plan_test.py <console/src/wire/types.ts> <PLAN.md>", file=sys.stderr)
        sys.exit(2)
    types_path = Path(sys.argv[1])
    plan_path = Path(sys.argv[2])
    kinds = kinds_of(types_path.read_text())
    # Flatten: a kind written across a wrap would be a check defeated by
    # markdown, which is the vacuity the refusal test already named.
    plan = " ".join(plan_path.read_text().split())
    missing = [k for k in kinds if not re.search(rf"\b{k}\b", plan)]
    if missing:
        print(
            "  x PLAN.md never names BookKind "
            + ", ".join(missing)
            + " — a kind the console offers that the roadmap does not record, "
            "which is how PERSONAL shipped invisible",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"  ok  {len(kinds)} BookKind values, each named in PLAN.md")


if __name__ == "__main__":
    main()
