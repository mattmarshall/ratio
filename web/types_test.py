#!/usr/bin/env python3
"""Assert web/src/types.ts mirrors console.proto.

The TypeScript types are hand-written. That is a considered choice — generating
them would mean a second codegen toolchain (buf, protoc-gen-es) for six messages
that change when the product does — but hand-written and UNCHECKED is a client
that compiles happily against a field the server stopped sending.

So: read every message and field from console.proto, convert each field name to
proto3 canonical JSON (lowerCamelCase), and assert the corresponding TypeScript
interface declares exactly those names.

Run: types_test.py <console.proto> <types.ts>
"""

import re
import sys
from pathlib import Path

# Messages the client does not consume. Request messages never cross the wire
# as JSON here — the transcoder builds them from the URL — so the client has no
# reason to declare them.
SKIP = re.compile(r"(Request)$")


def camel(snake: str) -> str:
    head, *rest = snake.split("_")
    return head + "".join(w[:1].upper() + w[1:] for w in rest)


def proto_messages(text: str) -> dict[str, set[str]]:
    """message name -> set of canonical JSON field names."""
    out: dict[str, set[str]] = {}
    # Strip comments so a field name inside prose is not mistaken for a field.
    text = re.sub(r"//[^\n]*", "", text)
    for m in re.finditer(r"\bmessage\s+(\w+)\s*\{", text):
        name = m.group(1)
        # Walk braces to find this message's body, so nested enums/messages do
        # not truncate it at the first `}`.
        i, depth = m.end(), 1
        while i < len(text) and depth:
            depth += (text[i] == "{") - (text[i] == "}")
            i += 1
        body = text[m.end() : i - 1]
        # Drop nested blocks; their fields belong to them, not here.
        body = re.sub(r"\b(?:enum|message)\s+\w+\s*\{[^{}]*\}", "", body)
        fields = set()
        for f in re.finditer(
            r"^\s*(?:repeated\s+)?[\w.]+\s+(\w+)\s*=\s*\d+", body, re.M
        ):
            fields.add(camel(f.group(1)))
        if fields:
            out[name] = fields
    return out


def ts_interfaces(text: str) -> dict[str, set[str]]:
    out: dict[str, set[str]] = {}
    for m in re.finditer(r"export interface (\w+)\s*\{([^}]*)\}", text):
        fields = set(re.findall(r"^\s*(\w+)\??:", m.group(2), re.M))
        out[m.group(1)] = fields
    return out


def main() -> None:
    proto = proto_messages(Path(sys.argv[1]).read_text())
    ts = ts_interfaces(Path(sys.argv[2]).read_text())

    checked = {n: f for n, f in proto.items() if not SKIP.search(n)}
    if not checked:
        sys.exit("::error::no messages found in the proto — this would pass vacuously")

    problems = []
    for name, fields in sorted(checked.items()):
        if name not in ts:
            problems.append(f"{name}: declared in the proto, missing from types.ts")
            continue
        missing = fields - ts[name]
        extra = ts[name] - fields
        for f in sorted(missing):
            problems.append(f"{name}.{f}: in the proto, not in types.ts")
        for f in sorted(extra):
            problems.append(f"{name}.{f}: in types.ts, not in the proto")
        if not missing and not extra:
            print(f"  ok  {name} ({len(fields)} fields)")

    if problems:
        for p in problems:
            print(f"::error::{p}")
        sys.exit(f"\n{len(problems)} type(s) out of step with the contract")
    print(f"{len(checked)} message(s) match the contract")


if __name__ == "__main__":
    main()
