#!/usr/bin/env python3
"""Assert the render fixtures are shaped like the contract.

⛔ A MOCK THAT HAS DRIFTED FROM THE ENCODER IS WORSE THAN NO MOCK. It makes a
suite green about a screen that would render `undefined` against the real
server, which is the exact failure `//proto:mirrors_test` exists to prevent
between `types.ts` and `transcode.rs`. These fixtures are a third mirror of the
same contract, so they get the same treatment: read every message and field out
of `console.proto`, convert to proto3 canonical JSON, and assert each fixture
carries exactly those names — recursively, through nested messages.

⚠ WHAT THIS DOES NOT CHECK IS THE VALUES. It cannot: only a real book knows what
a real book says. `console/scripts/capture_fixtures.sh` regenerates these from a
running `ratio watch` over `deploy/seed-demo-funds.sh`'s seven funds, and that is
the honest way to refresh them. This test is what keeps a hand-edit from
silently inventing a field in the meantime.

⛔ Anchored on a file, not a cwd-relative directory — see fields_test.py.

Run: fixtures_test.py <console.proto> <console/fixtures/funds.json>
"""

import json
import re
import sys
from pathlib import Path

# fixture file -> the message it is a body of.
FIXTURES = {
    "books.json": "ListBooksResponse",
    "book.json": "Book",
    "funds.json": "ListFundsResponse",
    "fund.json": "Fund",
    "views.json": "ListViewsResponse",
    "view.json": "View",
    "reconcile.json": "ReconcileViewsResponse",
    "breaks.json": "ListBreaksResponse",
    "break.json": "Break",
    "accounts.json": "ListAccountsResponse",
    "householdAccounts.json": "ListAccountsResponse",
    "postings.json": "ListPostingsResponse",
    "positions.json": "ListPositionsResponse",
    "lots.json": "ListLotsResponse",
    "navStrikes.json": "ListNavStrikesResponse",
    "replay.json": "ReplayNavStrikeResponse",
    "explain.json": "ExplainNavStrikeResponse",
    "configVersions.json": "ListConfigVersionsResponse",
    "diff.json": "DiffConfigVersionsResponse",
    "rules.json": "ListRulesResponse",
    "templates.json": "ListTemplatesResponse",
    "deliveries.json": "ListDeliveriesResponse",
    "pendingFacts.json": "ListPendingFactsResponse",
    "corporateActions.json": "ListCorporateActionsResponse",
    "changeLogEntries.json": "ListChangeLogEntriesResponse",
    "entry.json": "Entry",
    "entries.json": "ListEntriesResponse",
}

SCALARS = {
    "string", "bool", "int32", "int64", "uint32", "uint64", "sint32", "sint64",
    "fixed32", "fixed64", "sfixed32", "sfixed64", "float", "double", "bytes",
}


def camel(snake: str) -> str:
    head, *rest = snake.split("_")
    return head + "".join(w[:1].upper() + w[1:] for w in rest)


def proto_messages(text: str) -> dict[str, dict[str, str]]:
    """message -> {canonical JSON field name: declared type}."""
    out: dict[str, dict[str, str]] = {}
    text = re.sub(r"//[^\n]*", "", text)
    for m in re.finditer(r"\bmessage\s+(\w+)\s*\{", text):
        i, depth = m.end(), 1
        while i < len(text) and depth:
            depth += (text[i] == "{") - (text[i] == "}")
            i += 1
        body = text[m.end() : i - 1]
        nested = set(re.findall(r"\b(?:enum|message)\s+(\w+)\s*\{", body))
        body = re.sub(r"\b(?:enum|message)\s+\w+\s*\{[^{}]*\}", "", body)
        fields: dict[str, str] = {}
        for f in re.finditer(
            r"^\s*(?:repeated\s+)?([\w.]+)\s+(\w+)\s*=\s*\d+", body, re.M
        ):
            kind = f.group(1).split(".")[-1]
            # An enum declared inside this message is a string on the wire.
            fields[camel(f.group(2))] = "string" if kind in nested else kind
        if fields:
            out[m.group(1)] = fields
    return out


def check(
    what: str,
    value: object,
    message: str,
    messages: dict[str, dict[str, str]],
    problems: list[str],
) -> None:
    fields = messages.get(message)
    if fields is None:
        # google.protobuf.Timestamp / Duration and google.type.Date are not
        # declared here; their JSON is a scalar or a small well-known object.
        return
    if not isinstance(value, dict):
        problems.append(f"{what}: expected an object for {message}")
        return
    for f in sorted(set(fields) - set(value)):
        problems.append(f"{what}.{f}: in the proto, missing from the fixture")
    for f in sorted(set(value) - set(fields)):
        problems.append(f"{what}.{f}: in the fixture, not in the proto")
    for name, kind in fields.items():
        if kind in SCALARS or name not in value:
            continue
        child = value[name]
        if isinstance(child, list):
            for i, item in enumerate(child):
                check(f"{what}.{name}[{i}]", item, kind, messages, problems)
        elif child is not None:
            check(f"{what}.{name}", child, kind, messages, problems)


def main() -> None:
    messages = proto_messages(Path(sys.argv[1]).read_text())
    if not messages:
        sys.exit("::error::no messages found in the proto — this would pass vacuously")
    root = Path(sys.argv[2]).parent

    problems: list[str] = []
    seen = 0
    for filename, message in sorted(FIXTURES.items()):
        p = root / filename
        if not p.is_file():
            problems.append(f"{filename}: declared in FIXTURES and not on disk")
            continue
        if message not in messages:
            problems.append(f"{filename}: {message} is not a message in the proto")
            continue
        check(filename, json.loads(p.read_text()), message, messages, problems)
        seen += 1

    # A fixture nothing declares is a fixture nothing checks.
    for p in sorted(root.glob("*.json")):
        if p.name not in FIXTURES:
            problems.append(f"{p.name}: on disk and not declared in FIXTURES")

    if problems:
        for x in problems:
            print(f"::error::{x}")
        sys.exit(f"\n{len(problems)} fixture field(s) out of step with the contract")
    print(f"  ok  {seen} fixture(s), every field matching the contract")


if __name__ == "__main__":
    main()
