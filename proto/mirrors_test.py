#!/usr/bin/env python3
"""Assert every hand-written mirror of console.proto matches it.

Two mirrors exist, both deliberate and both dangerous:

  * `web/src/types.ts` — the client's view of the wire types. Generating it
    would mean a second codegen toolchain (buf, protoc-gen-es) for a handful of
    messages that change when the product does.
  * `crates/ratio-console/src/transcode.rs` — the `JsonView` impls that
    actually WRITE the JSON. Hand-written for the same reason the route table
    is: there is no Rust grpc-gateway worth the dependency.

Unchecked, either one drifts silently and in a way that is invisible until a
screen renders `undefined` where a figure should be. The encoder is the worse
of the two: types.ts can promise a field the server never sends, and TypeScript
will be perfectly happy about it.

So: read every message and field from console.proto, convert each name to
proto3 canonical JSON (lowerCamelCase), and assert both mirrors carry exactly
those names.

Run: mirrors_test.py <console.proto> <types.ts> <transcode.rs>
"""

import re
import sys
from pathlib import Path

# Request messages the transcoder builds from the URL never cross the wire as
# JSON, so neither mirror has a reason to declare them.
#
# ⚠ That stopped being true of ALL of them the moment a method declared
# `body: "*"`. Such a request IS a wire type — it is what the browser POSTs and
# what the Rust decodes — and skipping it left BOTH its mirrors unchecked, which
# is exactly the hole this file exists to close. `body_requests()` below finds
# them from the contract rather than from a list somebody has to remember to
# update.
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


def json_impls(text: str) -> dict[str, set[str]]:
    """message name -> the JSON keys its `JsonView` impl actually writes.

    The keys are read out of the format string rather than out of the struct
    fields it interpolates, because the format string is what reaches the
    browser. An impl that names a field and then interpolates the wrong one is
    a different bug, and not one this can see.
    """
    out: dict[str, set[str]] = {}
    for m in re.finditer(r"impl JsonView for pb::(\w+)\s*\{", text):
        i, depth = m.end(), 1
        while i < len(text) and depth:
            depth += (text[i] == "{") - (text[i] == "}")
            i += 1
        body = text[m.end() : i - 1]
        # `\"name\":` in the Rust source — the escaped quotes are literal here.
        out[m.group(1)] = set(re.findall(r'\\"(\w+)\\":', body))
    return out


def body_requests(text: str) -> dict[str, set[str]]:
    """request message -> the fields carried in the BODY, in canonical JSON.

    A field bound by the path template (`{parent=funds/*}`) is not in the body;
    that is what `body: "*"` means. Read off the contract so a new method with a
    body is covered the day it is declared.
    """
    out: dict[str, set[str]] = {}
    for m in re.finditer(
        r"rpc\s+\w+\((\w+)\)[^{]*\{(.*?)\n  \}", text, re.S
    ):
        req, block = m.group(1), m.group(2)
        if "body:" not in block:
            continue
        template = re.search(r'(?:post|put|patch):\s*"([^"]+)"', block)
        bound = set(re.findall(r"\{(\w+)=", template.group(1))) if template else set()
        out[req] = bound
    return out


def rust_decoders(text: str) -> dict[str, set[str]]:
    """request message -> the JSON keys its decoder reads.

    By convention the decoder for `FooRequest` is `fn foo_request(`. Nothing
    enforces that but this test, which fails loudly when it cannot find one.
    """
    out: dict[str, set[str]] = {}
    for m in re.finditer(r"fn (\w+_request)\(", text):
        i = text.index("{", m.end())
        j, depth = i + 1, 1
        while j < len(text) and depth:
            depth += (text[j] == "{") - (text[j] == "}")
            j += 1
        # Single-word string literals only; prose and format strings have spaces.
        out[m.group(1)] = set(re.findall(r'"(\w+)"', text[i:j]))
    return out


def snake(camel_name: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", camel_name).lower()


def compare(what: str, proto: dict[str, set[str]], mirror: dict[str, set[str]],
            only: set[str] | None = None) -> list[str]:
    problems = []
    for name, fields in sorted(proto.items()):
        if only is not None and name not in only:
            continue
        if name not in mirror:
            problems.append(f"{name}: in the proto, missing from {what}")
            continue
        for f in sorted(fields - mirror[name]):
            problems.append(f"{name}.{f}: in the proto, not in {what}")
        for f in sorted(mirror[name] - fields):
            problems.append(f"{name}.{f}: in {what}, not in the proto")
    return problems


def main() -> None:
    proto = proto_messages(Path(sys.argv[1]).read_text())
    ts = ts_interfaces(Path(sys.argv[2]).read_text())
    rs = json_impls(Path(sys.argv[3]).read_text())

    checked = {n: f for n, f in proto.items() if not SKIP.search(n)}
    if not checked:
        sys.exit("::error::no messages found in the proto — this would pass vacuously")
    if not rs:
        sys.exit("::error::no JsonView impls found — this would pass vacuously")

    # types.ts must declare every response message. The encoder is checked only
    # for what it claims to encode: not every message is returned at the top
    # level, and demanding an impl for each would be demanding dead code.
    problems = compare("types.ts", checked, ts)
    problems += compare("the JSON encoder", checked, rs, only=set(rs))

    # Request messages carried in a body are wire types too — both mirrors.
    bodies = body_requests(Path(sys.argv[1]).read_text())
    decoders = rust_decoders(Path(sys.argv[3]).read_text())
    for req, bound in sorted(bodies.items()):
        fields = proto.get(req, set()) - {camel(b) for b in bound}
        if not fields:
            problems.append(f"{req}: declares a body but has no fields outside the path")
            continue
        problems += compare("types.ts", {req: fields}, ts)
        fn = snake(req)
        if fn not in decoders:
            problems.append(f"{req}: carried in a body, and no `fn {fn}` decodes it")
            continue
        problems += compare("the request decoder", {req: fields}, {req: decoders[fn]})
        print(f"  ok  {req} ({len(fields)} body fields, both mirrors)")

    for name in sorted(set(rs) & set(checked)):
        if not compare("x", {name: checked[name]}, {name: rs[name]}):
            print(f"  ok  {name} ({len(checked[name])} fields, both mirrors)")

    if problems:
        for p in problems:
            print(f"::error::{p}")
        sys.exit(f"\n{len(problems)} mirror field(s) out of step with the contract")
    print(f"{len(checked)} message(s) in types.ts, {len(rs)} in the encoder — all match")


if __name__ == "__main__":
    main()
