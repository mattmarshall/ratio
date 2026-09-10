#!/usr/bin/env python3
import pathlib
import sys

workflow = pathlib.Path(sys.argv[1]).read_text()
required = [
    "workflow_dispatch:",
    "id-token: write",
    "ratio-demo-deploy",
    "RATIO_JOURNAL_BUCKET: ratio-demo-scale-320473299741",
    'confirmation must exactly match ACTION:BOOK_ID',
    'DEMO_MEMBERS must name exactly one AuthKit subject',
    'membership "$ACTION"',
    '--published "$BOOK_ID"',
    '--operation "$OPERATION_ID"',
]
missing = [claim for claim in required if claim not in workflow]
if missing:
    raise SystemExit(f"control workflow is missing required claims: {missing}")
if "aws-access-key-id" in workflow or "AWS_SECRET_ACCESS_KEY" in workflow:
    raise SystemExit("control workflow must use OIDC, not a long-lived AWS key")
print("ok  durable control workflow is bounded and OIDC-only")
