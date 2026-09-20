#!/usr/bin/env python3
"""Assert the kernel API's REST routes are exactly the contract's http rules.

The route table in `src/rest.rs` is hand-written, for the reason the console's
is: there is no Rust grpc-gateway worth a dependency, and the whole point of
serving REST from the same binary is that nothing sits in front of it. But
hand-written and UNCHECKED is a 404 a customer finds, so this reads the
descriptor set `//proto:ratio_proto` compiles to, pulls every `google.api.http`
rule off the Ledger and Chart services, and compares the set to what the table
declares. Drift in either direction fails.

Run: rest_routes_test.py <descriptor-set.bin> <rest.rs>
"""

import re
import sys
from pathlib import Path

# The rule's path strings are read out of the encoded descriptor rather than
# parsed with a protobuf runtime — every template starts "/v1/" and is
# length-delimited ASCII, so a scan recovers them intact. `:` is in the class
# for AIP-136 custom methods; HttpRule's `body` tag (0x3a, ASCII colon) and
# `additional_bindings` tag (0x5a, `Z`) can land right after a template, so
# both are stripped. See crates/ratio-console/transcode_test.py.
TEMPLATE = re.compile(rb"/v1/[A-Za-z0-9{}=*/_.:-]*")

# The console declares its own book routes in the same descriptor. Everything
# else keyed on `books/` is the kernel's.
CONSOLE = {
    "/v1/books",
    "/v1/{name=books/*}",
    "/v1/{parent=books/*/views/*}/accounts",
    "/v1/{name=books/*/views/*/accounts/*}",
}


def from_descriptor(path: Path) -> set[str]:
    blob = path.read_bytes()
    found = {m.decode("ascii").rstrip(":Z") for m in TEMPLATE.findall(blob)}
    return {t for t in found if "books/" in t and "funds" not in t and t not in CONSOLE}


def from_source(path: Path) -> set[str]:
    src = path.read_text()
    start = src.index("pub const ROUTES")
    table = src[start : src.index("];", start)]
    return set(re.findall(r'template:\s*"([^"]+)"', table))


def main() -> None:
    contract = from_descriptor(Path(sys.argv[1]))
    served = from_source(Path(sys.argv[2]))
    missing = sorted(contract - served)
    extra = sorted(served - contract)

    print(f"contract declares {len(contract)} kernel route(s)")
    for t in sorted(contract):
        print(f"  {'ok ' if t in served else 'MISSING'} {t}")
    # Seven rules over five templates: each Create shares its List's path, and
    # the byte scan sees templates, not methods.
    if len(contract) < 5:
        sys.exit("::error::fewer than the five Ledger + Chart route templates were found "
                 "in the descriptor — this test would pass vacuously")
    for t in missing:
        print(f"::error::contract declares {t} and rest.rs does not serve it")
    for t in extra:
        print(f"::error::rest.rs serves {t} and the contract does not declare it")
    if missing or extra:
        sys.exit(f"\n{len(missing) + len(extra)} route(s) out of step with the contract")
    print("routes match the contract")


if __name__ == "__main__":
    main()
