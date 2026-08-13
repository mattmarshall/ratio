#!/usr/bin/env python3
"""Assert the console reaches every RPC, and that every screen it claims exists.

Three hand-written things have to agree with `console.proto`, and each drifts
silently on its own:

  * `crates/ratio-console/src/transcode.rs` — the routes the server SERVES.
    Checked from the other side by `//crates/ratio-console:transcode_test`.
  * `console/src/wire/client.ts` — the routes the console CALLS.
  * `console/src/routes.ts` — the screens, and which calls each one makes.

⛔ THE DIRECTION THAT MATTERS IS "AN RPC NOBODY READS". `//web:rendered_test`
existed because a field was "declared, transcoded, served, typechecked and
mirrored while NO COMPONENT READS IT — that has already happened once." An
unread RPC is that same defect one level up: the contract grows a method, the
server serves it, both mirrors agree about it, and no screen ever asks. Nothing
else in this repository would say a word.

⚠ AND A ROUTE THAT NAMES A MISSING FILE IS A 404 FOUND BY A CUSTOMER. Next
routes by directory, so `routes.ts` can name a screen that was never written and
nothing at build time objects.

⛔ EVERY PATH IS DERIVED FROM A FILE, NEVER FROM THE WORKING DIRECTORY. Under
Bazel a test runs from the runfiles root, not from its package, so a relative
`console/src/app` resolves to nothing and every route reads as missing. The app
directory is taken from `routes.ts`'s own location instead, which is right from
any cwd — and the scripts must be runnable both ways, because CI runs them under
Bazel and `console.yml` runs them directly.

Run: route_manifest_test.py <console.proto> <client.ts> <routes.ts>
"""

import re
import sys
from pathlib import Path

# ⚠ At most this many upstream calls in one page. Two 15-second timeouts stack
# behind each — the Lambda's and the gateway's — and `watch.rs` closes the
# connection after every response, so there is no keep-alive to amortize them.
# What decides whether a page renders is the NUMBER of calls, not the distance.
MAX_READS_PER_FILE = 3


def strip_comments(text: str) -> str:
    """Drop `//` comments so a template quoted in prose is not read as a rule."""
    return re.sub(r"//[^\n]*", "", text)


def proto_rules(text: str) -> set[tuple[str, str]]:
    """Every `google.api.http` rule in the contract, as (METHOD, template)."""
    out: set[tuple[str, str]] = set()
    for block in re.finditer(
        r"option\s*\(google\.api\.http\)\s*=\s*\{(.*?)\};",
        strip_comments(text),
        re.S,
    ):
        for verb in re.finditer(
            r"\b(get|post|put|patch|delete):\s*\"([^\"]+)\"", block.group(1)
        ):
            out.add((verb.group(1).upper(), verb.group(2)))
    return out


def client_rules(text: str) -> set[tuple[str, str]]:
    """Every rule the client claims to call.

    Read out of the `// GET /v1/...` comment above each function, because that
    comment is the thing a reader compares against the proto. The template is
    also assembled below it in a real fetch — an impl whose comment and URL
    disagree is a different bug, and not one this can see.
    """
    return {
        (m.group(1), m.group(2))
        for m in re.finditer(
            r"^//\s*(GET|POST|PUT|PATCH|DELETE)\s+(/v1/\S+)\s*$", text, re.M
        )
    }


def client_exports(text: str) -> set[str]:
    return set(re.findall(r"^export const (\w+) = ", text, re.M))


def manifest(text: str) -> list[tuple[str, str, list[str]]]:
    """(path, file, reads) for every entry in ROUTES."""
    out: list[tuple[str, str, list[str]]] = []
    body = text[text.index("export const ROUTES") :]
    for m in re.finditer(
        r"path:\s*\"([^\"]+)\",\s*file:\s*\"([^\"]+)\",\s*reads:\s*\[([^\]]*)\]",
        strip_comments(body),
        re.S,
    ):
        reads = re.findall(r"\"(\w+)\"", m.group(3))
        out.append((m.group(1), m.group(2), reads))
    return out


def main() -> None:
    proto_text = Path(sys.argv[1]).read_text()
    client_text = Path(sys.argv[2]).read_text()
    routes = Path(sys.argv[3])
    routes_text = routes.read_text()
    # `src/routes.ts` sits beside `src/app/`. See the ⛔ above.
    app_dir = routes.parent / "app"

    problems: list[str] = []

    # 1. The client calls exactly the contract's routes.
    want, have = proto_rules(proto_text), client_rules(client_text)
    if not want:
        sys.exit("::error::no google.api.http rules found — this would pass vacuously")
    if not have:
        sys.exit("::error::no routes found in the client — this would pass vacuously")
    for method, template in sorted(want - have):
        problems.append(f"{method} {template}: in the contract, not called by the console")
    for method, template in sorted(have - want):
        problems.append(f"{method} {template}: called by the console, not in the contract")

    # 2. Every screen exists, reads only real calls, and stays inside the budget.
    routes = manifest(routes_text)
    if not routes:
        sys.exit("::error::no routes found in routes.ts — this would pass vacuously")
    exports = client_exports(client_text)
    read_by_somebody: set[str] = set()
    for path, file, reads in routes:
        if not (app_dir / file).is_file():
            problems.append(f"{path}: routes.ts names {file}, which does not exist")
        for r in reads:
            if r not in exports:
                problems.append(f"{path}: reads {r!r}, which the client does not export")
            read_by_somebody.add(r)
        if len(reads) > MAX_READS_PER_FILE:
            problems.append(
                f"{path}: makes {len(reads)} upstream calls, over the budget of "
                f"{MAX_READS_PER_FILE} — split it behind a <Suspense> child segment"
            )

    # 3. ⛔ The direction that matters: no RPC goes unread.
    for fn in sorted(exports - read_by_somebody):
        problems.append(
            f"{fn}: the console can call it and no screen does — "
            f"either give it a route in routes.ts, or take it out of the client"
        )

    if problems:
        for p in problems:
            print(f"::error::{p}")
        sys.exit(f"\n{len(problems)} problem(s) between the contract and the console")

    print(
        f"  ok  {len(want)} route(s) in the contract, all called; "
        f"{len(routes)} screen(s), all present; {len(exports)} call(s), all read"
    )


if __name__ == "__main__":
    main()
