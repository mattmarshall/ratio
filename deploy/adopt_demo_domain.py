#!/usr/bin/env python3
"""Build an import-only template: live stack + DemoDomain / DemoApiMapping.

⛔ A REGULAR STACK UPDATE THAT CREATES AWS::ApiGatewayV2::DomainName FAILS
because ops already attached `api.ratio.marsh.build`. The physical name is
the hostname. Recreating it would mint a new `d-xxxxx` and break the
Cloudflare CNAME that already points at `d-dh396r9poe.execute-api.us-east-1.amazonaws.com`.

CloudFormation import change sets cannot include modifications of existing
resources. This script copies CertificateArn / DemoDomain / DemoApiMapping
from the new app template into the *currently deployed* template and leaves
Function env, ScaleTask env, and DemoUrl alone. deploy.yml imports that,
then `cloudformation deploy` of app.yaml flips DemoUrl and RATIO_PUBLIC_ORIGIN.

Logical ids must stay DemoDomain / DemoApiMapping — the same names app.yaml
uses — or the follow-up deploy would CREATE a second DomainName.
"""

from __future__ import annotations

import argparse
import json
import re
import sys


LIVE_DOMAIN = "api.ratio.marsh.build"
LIVE_CERT = (
    "arn:aws:acm:us-east-1:320473299741:certificate/"
    "4452c092-e591-46e2-8d40-2e29269a033b"
)
LOGICAL_DOMAIN = "DemoDomain"
LOGICAL_MAPPING = "DemoApiMapping"


def extract_top_level(text: str, name: str) -> str:
    """Return the `  Name:` block at Parameters/Resources indent."""
    lines = text.splitlines(keepends=True)
    start = None
    for i, line in enumerate(lines):
        if line.startswith(f"  {name}:"):
            start = i
            break
    if start is None:
        raise SystemExit(f"app template has no {name!r} block")
    end = start + 1
    while end < len(lines):
        if re.match(r"  [A-Za-z0-9]+:", lines[end]):
            break
        end += 1
    return "".join(lines[start:end])


def inject_yaml(live: str, app: str) -> str:
    if f"  {LOGICAL_DOMAIN}:" in live and "AWS::ApiGatewayV2::DomainName" in live:
        return live
    cert = extract_top_level(app, "CertificateArn")
    domain = extract_top_level(app, LOGICAL_DOMAIN)
    mapping = extract_top_level(app, LOGICAL_MAPPING)
    if "ConnectApi" in mapping:
        raise SystemExit(
            "DemoApiMapping cites ConnectApi — the custom domain is Demo HTTP API only"
        )
    if "  CertificateArn:" not in live:
        if "\nResources:\n" not in live:
            raise SystemExit("live template has no Resources: section")
        live = live.replace("\nResources:\n", "\n" + cert + "Resources:\n", 1)
    if f"  {LOGICAL_DOMAIN}:" not in live:
        if "\nOutputs:\n" not in live:
            raise SystemExit("live template has no Outputs: section")
        live = live.replace(
            "\nOutputs:\n",
            "\n" + domain + mapping + "Outputs:\n",
            1,
        )
    return live


def inject_json(live: str, app: str) -> str:
    doc = json.loads(live)
    params = doc.setdefault("Parameters", {})
    resources = doc.setdefault("Resources", {})
    if LOGICAL_DOMAIN in resources:
        return json.dumps(doc, indent=2) + "\n"
    if "CertificateArn" not in params:
        params["CertificateArn"] = {
            "Type": "String",
            "Default": LIVE_CERT,
            "Description": extract_top_level(app, "CertificateArn"),
        }
    resources[LOGICAL_DOMAIN] = {
        "Type": "AWS::ApiGatewayV2::DomainName",
        "DeletionPolicy": "Retain",
        "UpdateReplacePolicy": "Retain",
        "Properties": {
            "DomainName": LIVE_DOMAIN,
            "DomainNameConfigurations": [
                {
                    "CertificateArn": {"Ref": "CertificateArn"},
                    "EndpointType": "REGIONAL",
                    "SecurityPolicy": "TLS_1_2",
                }
            ],
        },
    }
    resources[LOGICAL_MAPPING] = {
        "Type": "AWS::ApiGatewayV2::ApiMapping",
        "DeletionPolicy": "Retain",
        "UpdateReplacePolicy": "Retain",
        "DependsOn": ["Stage", LOGICAL_DOMAIN],
        "Properties": {
            "DomainName": {"Ref": LOGICAL_DOMAIN},
            "ApiId": {"Ref": "Api"},
            "Stage": "$default",
        },
    }
    return json.dumps(doc, indent=2) + "\n"


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--live", required=True, help="currently deployed template")
    p.add_argument("--app", required=True, help="new deploy/app.yaml")
    p.add_argument("--out", required=True, help="import-only template to write")
    args = p.parse_args(argv)

    live = open(args.live, encoding="utf-8").read()
    app = open(args.app, encoding="utf-8").read()
    if LOGICAL_DOMAIN not in app or LOGICAL_MAPPING not in app:
        raise SystemExit("app.yaml is missing DemoDomain / DemoApiMapping")
    if "AWS::CloudFront::" in app:
        raise SystemExit("app.yaml invented CloudFront — #152 is HTTP API DomainName")

    stripped_app = "\n".join(
        line for line in app.splitlines() if not line.lstrip().startswith("#")
    )
    if "AWS::ApiGatewayV2::DomainName" not in stripped_app:
        raise SystemExit("app.yaml DomainName is comment-only")

    merged = inject_json(live, app) if live.lstrip().startswith("{") else inject_yaml(live, app)
    open(args.out, "w", encoding="utf-8").write(merged)
    return 0


if __name__ == "__main__":
    sys.exit(main())
