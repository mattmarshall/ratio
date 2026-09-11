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
    '_bootstrap is a reserved book ID',
    'subject="$(deploy/control_member.sh "$DEMO_MEMBERS")"',
    'membership "$ACTION"',
    '--published "$BOOK_ID"',
    '--operation "$OPERATION_ID"',
]
missing = [claim for claim in required if claim not in workflow]
if missing:
    raise SystemExit(f"control workflow is missing required claims: {missing}")
static_credential_tokens = [
    "aws-access-key-id",
    "aws-secret-access-key",
    "aws_access_key_id",
    "aws_secret_access_key",
    "aws_session_token",
]
if any(token in workflow.lower() for token in static_credential_tokens):
    raise SystemExit("control workflow must use OIDC, not a long-lived AWS key")
print("ok  durable control workflow is bounded and OIDC-only")
