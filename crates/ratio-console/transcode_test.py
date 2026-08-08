#!/usr/bin/env python3
"""Assert the transcoder's routes are exactly the contract's http rules.

The route table in `src/transcode.rs` is hand-written. That is a deliberate
choice — there is no Rust grpc-gateway worth a dependency, and Envoy in front
of a Lambda would cost more than the Lambda. But hand-written and UNCHECKED is
a 404 a customer finds, so this reads the descriptor set `//proto:ratio_proto`
compiles to, pulls every `google.api.http` rule off the Console service, and
compares the set to what the table declares.

Drift in either direction fails: a route the contract does not declare, and a
method the contract declares that nothing serves.

Run: transcode_test.py <descriptor-set.bin> <transcode.rs>
"""

import re
import sys
from pathlib import Path

# field 34 on MethodOptions is google.api.http (HttpRule). Rather than depend on
# a protobuf runtime to parse the descriptor, the rule's path strings are read
# out of the encoded bytes — every template starts "/v1/" and is length-
# delimited ASCII, so they survive a scan intact. A structural parse would be
# better; this is enough to catch drift and adds no dependency to a test.
# `:` is in the class because AIP-136 custom methods are suffixed with one —
# `/v1/{name=funds/*/navStrikes/*}:replay`. Without it the scan silently
# truncated every custom method to its resource path, so a missing custom route
# looked identical to a present one. Found when NavStrike's :replay was the one
# route this test did NOT flag.
TEMPLATE = re.compile(rb"/v1/[A-Za-z0-9{}=*/_.:-]*")


def from_descriptor(path: Path) -> set[str]:
    blob = path.read_bytes()
    # ⛔ The scan is byte-based, and HttpRule's `body` field tag is 0x3a — an
    # ASCII colon. On any rule that declares a body it lands on the end of the
    # template, so `…:applyEvent` scanned as `…:applyEvent:`. A template never
    # ends in a bare colon; a custom method always names one after it.
    found = {m.decode("ascii").rstrip(":") for m in TEMPLATE.findall(blob)}
    # Other services in the same descriptor declare /v1 routes too; the console's
    # are the ones under funds/.
    return {t for t in found if "funds" in t}


def from_source(path: Path) -> set[str]:
    src = path.read_text()
    table = src[src.index("pub const ROUTES"):src.index("/// Serve one request")]
    return set(re.findall(r'template:\s*"([^"]+)"', table))


def main() -> None:
    contract = from_descriptor(Path(sys.argv[1]))
    served = from_source(Path(sys.argv[2]))

    missing = sorted(contract - served)   # declared, not served
    extra = sorted(served - contract)     # served, not declared

    print(f"contract declares {len(contract)} console route(s)")
    for t in sorted(contract):
        print(f"  {'ok ' if t in served else 'MISSING'} {t}")

    if not contract:
        sys.exit("::error::no console routes found in the descriptor — "
                 "this test would pass vacuously")
    if missing:
        for t in missing:
            print(f"::error::contract declares {t} and the transcoder does not serve it")
    if extra:
        for t in extra:
            print(f"::error::transcoder serves {t} and the contract does not declare it")
    if missing or extra:
        sys.exit(f"\n{len(missing) + len(extra)} route(s) out of step with the contract")
    print("routes match the contract")


if __name__ == "__main__":
    main()
