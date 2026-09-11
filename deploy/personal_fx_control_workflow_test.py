#!/usr/bin/env python3
import pathlib
import sys

workflow = pathlib.Path(sys.argv[1]).read_text()
required = [
    "workflow_dispatch:",
    "id-token: write",
    "cancel-in-progress: false",
    "ratio-demo-deploy",
    "RATIO_JOURNAL_BUCKET: ratio-demo-scale-320473299741",
    "confirmation must exactly match activate:personal-fx-walkthrough",
    'subject="$(deploy/control_member.sh "$DEMO_MEMBERS")"',
    'export RATIO_ACTOR="$subject"',
    "config activate-personal-fx",
    '--operation "$OPERATION_ID"',
]
missing = [claim for claim in required if claim not in workflow]
if missing:
    raise SystemExit(f"Personal FX workflow is missing required claims: {missing}")
for forbidden in [
    "book_id:",
    "currencies:",
    "RATIO_PERSONAL_CURRENCIES",
    "aws-access-key-id",
    "aws-secret-access-key",
]:
    if forbidden.lower() in workflow.lower():
        raise SystemExit(f"Personal FX workflow exposes or trusts forbidden input: {forbidden}")
print("ok  Personal FX control is fixed-purpose, attributed, serialized, and OIDC-only")
