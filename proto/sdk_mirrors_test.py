#!/usr/bin/env python3
"""Assert every hand-written mirror of the kernel API matches the contract.

`ledger.proto` and `chart.proto` have three mirrors, each hand-written for the
reason `console/src/wire/types.ts` is — a codegen toolchain per language for
seven messages costs more than the check that fails just as loudly:

  * `crates/ratio-api/src/json.rs` — the `JsonView` impls that WRITE and READ the
    REST JSON. The encoder is the dangerous one: it can promise a key the
    contract never declared, and every client would learn to rely on it.
  * `sdk/python/ratio/types.py` — the Python SDK's dataclasses and their JSON.
  * `sdk/typescript/src/types.ts` — the TypeScript SDK's wire interfaces.

For every message that crosses the wire as JSON, each mirror must carry exactly
the contract's fields under proto3's canonical (lowerCamelCase) names. A key a
mirror invents, or a field it forgets, fails the build.

Run: sdk_mirrors_test.py <ledger.proto> <chart.proto> <json.rs> <types.py> <types.ts>
"""

import re
import sys
from pathlib import Path

# Requests are built from the URL and the body by the transport, not mirrored as
# types; the body messages they carry (Transaction, Account) are checked as
# themselves.
SKIP = re.compile(r"Request$")

# Python names its pages by what they hold; the contract names them by the call.
PY_ALIASES = {"TransactionPage": "ListTransactionsResponse", "AccountPage": "ListAccountsResponse"}


def camel(snake: str) -> str:
    head, *rest = snake.split("_")
    return head + "".join(w[:1].upper() + w[1:] for w in rest)


def proto_messages(text: str) -> dict[str, set[str]]:
    """message name -> the set of canonical JSON field names."""
    out: dict[str, set[str]] = {}
    text = re.sub(r"//[^\n]*", "", text)
    for m in re.finditer(r"\bmessage\s+(\w+)\s*\{", text):
        i, depth = m.end(), 1
        while i < len(text) and depth:
            depth += (text[i] == "{") - (text[i] == "}")
            i += 1
        body = re.sub(r"\b(?:enum|message)\s+\w+\s*\{[^{}]*\}", "", text[m.end() : i - 1])
        fields = {
            camel(f.group(1))
            for f in re.finditer(r"^\s*(?:optional\s+|repeated\s+)?[\w.]+\s+(\w+)\s*=\s*\d+", body, re.M)
        }
        if fields:
            out[m.group(1)] = fields
    return out


def blocks(text: str, head: str) -> dict[str, str]:
    """name -> body, for every `head <name> {` block, walking braces."""
    out: dict[str, str] = {}
    for m in re.finditer(head, text):
        i, depth = m.end(), 1
        while i < len(text) and depth:
            depth += (text[i] == "{") - (text[i] == "}")
            i += 1
        out[m.group(1)] = text[m.end() : i - 1]
    return out


def rust_json(text: str) -> dict[str, set[str]]:
    """The keys each `impl JsonView for pb::X` writes (`put_*(&mut o, "k", …)`)
    or reads (`get_*(v, "k", …)`)."""
    return {
        name: set(re.findall(r'\b(?:put|get)_\w+\(\s*(?:&mut\s+o|v)\s*,\s*"(\w+)"', body))
        for name, body in blocks(text, r"impl JsonView for pb::(\w+)\s*\{").items()
    }


def python_json(text: str) -> dict[str, set[str]]:
    """The keys each dataclass writes (`_put(o, "k", …)`) or reads (`_x(d, "k")`, `d.get("k")`)."""
    out: dict[str, set[str]] = {}
    parts = re.split(r"^class (\w+)\b.*$", text, flags=re.M)
    for name, body in zip(parts[1::2], parts[2::2]):
        keys = set(re.findall(r'\b_put\(\s*o\s*,\s*"(\w+)"', body))
        keys |= set(re.findall(r'\b_(?:int64|opt_int64|str|opt_str|enum)\(\s*d\s*,\s*"(\w+)"', body))
        keys |= set(re.findall(r'\bd\.get\(\s*"(\w+)"', body))
        keys |= set(re.findall(r'\bo\[\s*"(\w+)"\s*\]', body))
        if keys:
            out[PY_ALIASES.get(name, name)] = keys
    return out


def ts_interfaces(text: str) -> dict[str, set[str]]:
    return {
        m.group(1): set(re.findall(r"^\s*(\w+)\??:", m.group(2), re.M))
        for m in re.finditer(r"export interface (\w+)\s*\{([^}]*)\}", text)
    }


def compare(label: str, contract: dict[str, set[str]], mirror: dict[str, set[str]]) -> list[str]:
    errors = []
    for name, fields in sorted(contract.items()):
        if SKIP.search(name):
            continue
        if name not in mirror:
            errors.append(f"{label}: {name} is not mirrored at all")
            continue
        for f in sorted(fields - mirror[name]):
            errors.append(f"{label}: {name}.{f} is in the contract and not in the mirror")
        for f in sorted(mirror[name] - fields):
            errors.append(f"{label}: {name}.{f} is in the mirror and not in the contract")
    for name in sorted(set(mirror) - set(contract)):
        errors.append(f"{label}: {name} is mirrored but is not a message in the contract")
    return errors


def main() -> None:
    ledger, chart, json_rs, types_py, types_ts = (Path(p).read_text() for p in sys.argv[1:6])
    contract = {**proto_messages(ledger), **proto_messages(chart)}
    wire = {n: f for n, f in contract.items() if not SKIP.search(n)}
    if len(wire) < 7:
        sys.exit(f"::error::only {len(wire)} wire message(s) found in the contract — this test would pass vacuously")

    errors = []
    errors += compare("json.rs", contract, rust_json(json_rs))
    errors += compare("types.py", contract, python_json(types_py))
    errors += compare("types.ts", contract, ts_interfaces(types_ts))

    print(f"contract declares {len(wire)} wire message(s): {', '.join(sorted(wire))}")
    if errors:
        for e in errors:
            print(f"::error::{e}")
        sys.exit(f"\n{len(errors)} mirror(s) out of step with the contract")
    print("every mirror matches the contract")


if __name__ == "__main__":
    main()
