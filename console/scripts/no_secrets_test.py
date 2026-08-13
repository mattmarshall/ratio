#!/usr/bin/env python3
"""No credential, account id or endpoint gets committed with the console.

⛔ THIS MIGRATION INTRODUCES THIS REPOSITORY'S FIRST RUNTIME SECRET. Until now
the application path held none: the Google OAuth client secret is a CloudFormation
`NoEcho` parameter read from a GitHub secret, and the Lambda's own role reaches
no AWS API and holds nothing. A session-cookie key is different — it lives in
the process, it is read from the environment, and the shortest path to a working
local run is to paste one into a file.

⚠ AND A CLIENT-SIDE FILE IS A PUBLISHED FILE. Anything a component imports ships
to the browser. A key committed here is a key disclosed, and rotating it is a
deploy rather than an edit.

⚠ WHAT THIS CANNOT SEE is a secret in an environment variable set correctly,
which is the whole intended path. It is a floor, not a proof.

Run: no_secrets_test.py <console-src-dir>
"""

import re
import sys
from pathlib import Path

# (pattern, what it would be)
FORBIDDEN: list[tuple[re.Pattern[str], str]] = [
    # The demo account. Named in deploy/ on purpose; not something to scatter.
    (re.compile(r"\b320473299741\b"), "an AWS account id"),
    # A Cognito pool or client id, which pins the console to one deployment.
    (re.compile(r"\bus-east-1_[A-Za-z0-9]{9}\b"), "a Cognito user pool id"),
    (re.compile(r"\b[0-9a-z]{26}\b"), "a Cognito app client id"),
    # An AWS access key. Never right in a repository, let alone a client bundle.
    (re.compile(r"\b(?:AKIA|ASIA)[0-9A-Z]{16}\b"), "an AWS access key id"),
    # A base64 32-byte key, which is exactly what RATIO_SESSION_KEYS holds.
    (re.compile(r"[\"'][A-Za-z0-9+/]{43}=[\"']"), "a 32-byte base64 key"),
    # A deployment's own hostnames. These belong in environment variables, and a
    # console with one compiled in cannot be pointed at a second book.
    (re.compile(r"execute-api\.[a-z0-9-]+\.amazonaws\.com"), "an API Gateway host"),
    (re.compile(r"[a-z0-9-]+\.auth\.[a-z0-9-]+\.amazoncognito\.com"), "a Cognito domain"),
]

# The env var NAMES are meant to appear in the source; only their values are not.
ALLOWED = re.compile(r"RATIO_SESSION_KEYS|RATIO_API_ORIGIN|RATIO_CONSOLE_ORIGIN")


def main() -> None:
    root = Path(sys.argv[1])
    files = [p for p in root.rglob("*") if p.suffix in {".ts", ".tsx", ".css", ".json"}]
    if not files:
        sys.exit("::error::no sources found — this would pass vacuously")

    problems: list[str] = []
    for p in files:
        for n, line in enumerate(p.read_text().splitlines(), 1):
            if ALLOWED.search(line):
                continue
            for pattern, what in FORBIDDEN:
                if pattern.search(line):
                    problems.append(f"{p}:{n}: looks like {what}")

    if problems:
        for x in problems:
            print(f"::error::{x}")
        sys.exit(
            f"\n{len(problems)} line(s) carry something that belongs in the "
            f"environment, not in the repository"
        )
    print(f"  ok  {len(files)} file(s), nothing that belongs in the environment")


if __name__ == "__main__":
    main()
