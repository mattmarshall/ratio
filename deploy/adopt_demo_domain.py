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

⛔ Skip IMPORT only when the live get-template body already declares both
logical ids (`owns`). `describe-stack-resources --logical-resource-id
DemoDomain` exits 0 with an empty list when the stack does not own the
resource; that skip is how #246 CREATE_FAILED AlreadyExists.
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


def find_top_level(text: str, name: str) -> str | None:
    """Return the `  Name:` block at Parameters/Resources indent, or None."""
    lines = text.splitlines(keepends=True)
    start = None
    for i, line in enumerate(lines):
        if line.startswith(f"  {name}:"):
            start = i
            break
    if start is None:
        return None
    end = start + 1
    while end < len(lines):
        if re.match(r"  [A-Za-z0-9]+:", lines[end]):
            break
        end += 1
    return "".join(lines[start:end])


def extract_top_level(text: str, name: str) -> str:
    """Return the `  Name:` block at Parameters/Resources indent."""
    block = find_top_level(text, name)
    if block is None:
        raise SystemExit(f"app template has no {name!r} block")
    return block


def live_declares(live: str, name: str, type_name: str) -> bool:
    """True iff the live template Resources already declare `name` as `type_name`."""
    if live.lstrip().startswith("{"):
        doc = json.loads(live)
        res = (doc.get("Resources") or {}).get(name)
        if not isinstance(res, dict):
            return False
        return res.get("Type") == type_name
    block = find_top_level(live, name)
    if block is None:
        return False
    return type_name in block


def live_owns_imported_domain(live: str) -> bool:
    """True only when the live template already declares both import targets.

    ⛔ `describe-stack-resources --logical-resource-id DemoDomain` is the
    wrong gate. The plural API returns empty `StackResources` and exit 0
    when the logical id is absent (#246). The subsequent
    `cloudformation deploy` then CREATE_FAILED AlreadyExists because the
    physical hostname is ops-attached outside the stack. Ownership is the
    live `get-template` body containing DemoDomain and DemoApiMapping.
    A half-imported template is an error — import cannot create the
    missing half, and CREATE would remint `d-xxxxx`.
    """
    has_domain = live_declares(live, LOGICAL_DOMAIN, "AWS::ApiGatewayV2::DomainName")
    has_mapping = live_declares(live, LOGICAL_MAPPING, "AWS::ApiGatewayV2::ApiMapping")
    if has_domain and has_mapping:
        return True
    if has_domain or has_mapping:
        raise SystemExit(
            "live template has only one of DemoDomain / DemoApiMapping — "
            "import cannot create the missing half; do not CreateDomainName"
        )
    return False


# Top-level sections that may follow Parameters in a CloudFormation
# template. Used to append a parameter at the *end of Parameters*, not
# at Resources.
#
# ⛔ SPLICING AT `Resources:` PUTS THE PARAMETER UNDER Conditions.
# app.yaml (and the live stack at d1bc047) has Conditions: between
# Parameters and Resources. Run 34006554883 then died with
# `Parameters: [CertificateArn] do not exist in the template` because
# inject_yaml had landed CertificateArn as a Condition (#250).
# `"  CertificateArn:" in live` is also the wrong existence check —
# DemoDomain's `CertificateArn: !Ref` property contains that
# substring at a deeper indent.
_AFTER_PARAMETERS = (
    "Metadata",
    "Rules",
    "Mappings",
    "Conditions",
    "Transform",
    "Resources",
    "Outputs",
)


def parameter_names(template: str) -> list[str]:
    """Return Parameters: keys. Conditions / Resources do not count.

    ⭐ THIS IS THE CHECK CREATECHANGESET ACTUALLY RUNS. A CertificateArn
    that landed under Conditions is not a Parameter, and passing
    `--parameters ParameterKey=CertificateArn` is then a ValidationError.
    """
    if template.lstrip().startswith("{"):
        doc = json.loads(template)
        params = doc.get("Parameters") or {}
        return list(params) if isinstance(params, dict) else []
    names: list[str] = []
    in_params = False
    for line in template.splitlines():
        if line.startswith("Parameters:"):
            in_params = True
            continue
        if not in_params:
            continue
        if line and line[0] not in " \t" and not line.lstrip().startswith("#"):
            break
        m = re.match(r"  ([A-Za-z0-9]+):", line)
        if m:
            names.append(m.group(1))
    return names


def insert_parameter_yaml(live: str, param_block: str) -> str:
    """Append `param_block` to Parameters, before Conditions / Resources."""
    if "\nParameters:\n" not in live and not live.startswith("Parameters:\n"):
        raise SystemExit("live template has no Parameters: section")
    cut = None
    for name in _AFTER_PARAMETERS:
        idx = live.find(f"\n{name}:\n")
        if idx != -1 and (cut is None or idx < cut):
            cut = idx
    if cut is None:
        raise SystemExit(
            "live template has no section after Parameters — "
            "cannot place CertificateArn under Parameters"
        )
    block = param_block if param_block.endswith("\n") else param_block + "\n"
    return live[:cut] + "\n" + block + live[cut:]


def inject_yaml(live: str, app: str) -> str:
    if live_owns_imported_domain(live):
        return live
    cert = extract_top_level(app, "CertificateArn")
    domain = extract_top_level(app, LOGICAL_DOMAIN)
    mapping = extract_top_level(app, LOGICAL_MAPPING)
    if "ConnectApi" in mapping:
        raise SystemExit(
            "DemoApiMapping cites ConnectApi — the custom domain is Demo HTTP API only"
        )
    # find_top_level is indent-2 only — a DomainNameConfigurations
    # CertificateArn property does not count as the Parameter.
    if find_top_level(live, "CertificateArn") is None:
        live = insert_parameter_yaml(live, cert)
    if f"  {LOGICAL_DOMAIN}:" not in live:
        if "\nOutputs:\n" not in live:
            raise SystemExit("live template has no Outputs: section")
        live = live.replace(
            "\nOutputs:\n",
            "\n" + domain + mapping + "Outputs:\n",
            1,
        )
    if "CertificateArn" not in parameter_names(live):
        raise SystemExit(
            "import template Parameters has no CertificateArn — "
            "CreateChangeSet would reject ParameterKey=CertificateArn (#250)"
        )
    return live


def inject_json(live: str, app: str) -> str:
    if live_owns_imported_domain(live):
        doc = json.loads(live)
        return json.dumps(doc, indent=2) + "\n"
    doc = json.loads(live)
    params = doc.setdefault("Parameters", {})
    resources = doc.setdefault("Resources", {})
    if "CertificateArn" not in params:
        params["CertificateArn"] = {
            "Type": "String",
            "Default": LIVE_CERT,
            "Description": extract_top_level(app, "CertificateArn"),
        }
    if "CertificateArn" not in parameter_names(json.dumps(doc)):
        raise SystemExit(
            "import template Parameters has no CertificateArn — "
            "CreateChangeSet would reject ParameterKey=CertificateArn (#250)"
        )
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


def live_template_from_get_template(doc: dict) -> str:
    """Unwrap `cloudformation get-template --output json` to the live body.

    ⛔ THIS USED TO BE `python3 -c` IN deploy.yml. A Python line at column 0
    inside `run: |` exits the YAML literal block; GitHub then schedules no
    jobs. That is how tip deploy died after #243 (#244). Keep the body here.
    """
    body = doc["TemplateBody"]
    if isinstance(body, str):
        return body if body.endswith("\n") else body + "\n"
    return json.dumps(body, indent=2)


def import_resources(
    mappings: dict, api_id: str, connect_id: str, domain: str
) -> list[dict]:
    """Build the IMPORT `ResourcesToImport` list for DemoDomain / DemoApiMapping.

    Same refuse as the inline snippet #243 shipped: a mapping on ConnectApi
    would put both issuers on one hostname. The empty-key mapping must already
    point at the Demo HTTP API — this does not CreateDomainName or remint
    `d-xxxxx`.
    """
    items = mappings.get("Items") or []
    connect = [i for i in items if i.get("ApiId") == connect_id]
    if connect:
        sys.exit("custom domain is mapped to ConnectApi — issuers would collide")
    demo = [i for i in items if i.get("ApiId") == api_id and not i.get("ApiMappingKey")]
    if not demo:
        sys.exit("no empty-key ApiMapping from %s to Demo API %s" % (domain, api_id))
    return [
        {
            "ResourceType": "AWS::ApiGatewayV2::DomainName",
            "LogicalResourceId": LOGICAL_DOMAIN,
            "ResourceIdentifier": {"DomainName": domain},
        },
        {
            "ResourceType": "AWS::ApiGatewayV2::ApiMapping",
            "LogicalResourceId": LOGICAL_MAPPING,
            "ResourceIdentifier": {
                "DomainName": domain,
                "ApiMappingId": demo[0]["ApiMappingId"],
            },
        },
    ]


def _cmd_owns(argv: list[str]) -> int:
    """Print `owned` or `absent`. Partial ownership is a hard error.

    deploy.yml cases on the printed word. Exit 0 for both owned and
    absent so `set -e` does not treat "need import" as a script failure.
    """
    p = argparse.ArgumentParser(description="does the live template already own DemoDomain?")
    p.add_argument("--live", required=True, help="currently deployed template")
    args = p.parse_args(argv)
    live = open(args.live, encoding="utf-8").read()
    if live_owns_imported_domain(live):
        print("owned")
    else:
        print("absent")
    return 0


def _cmd_extract_live(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description="unwrap get-template JSON to the live body")
    p.add_argument("--src", required=True, help="get-template --output json file")
    p.add_argument("--out", required=True, help="live template to write")
    args = p.parse_args(argv)
    doc = json.load(open(args.src, encoding="utf-8"))
    open(args.out, "w", encoding="utf-8").write(live_template_from_get_template(doc))
    return 0


def _cmd_import_resources(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description="write resources-to-import JSON")
    p.add_argument("--mappings", required=True, help="get-api-mappings --output json")
    p.add_argument("--api-id", required=True, help="Demo HTTP API id")
    p.add_argument("--connect-id", required=True, help="Connect HTTP API id")
    p.add_argument("--domain", required=True, help="ops-attached custom hostname")
    p.add_argument("--out", required=True, help="resources-to-import JSON to write")
    args = p.parse_args(argv)
    data = json.load(open(args.mappings, encoding="utf-8"))
    resources = import_resources(data, args.api_id, args.connect_id, args.domain)
    json.dump(resources, open(args.out, "w", encoding="utf-8"), indent=2)
    print("import DemoDomain + DemoApiMapping %s -> %s" % (args.domain, args.api_id))
    return 0


def _cmd_parameter_keys(argv: list[str]) -> int:
    """Print Parameters: keys, one per line. Used by deploy.yml so
    CreateChangeSet never receives a key the import template omitted.
    """
    p = argparse.ArgumentParser(description="list Parameters keys in a template")
    p.add_argument("--template", required=True, help="import-only template")
    args = p.parse_args(argv)
    for name in parameter_names(open(args.template, encoding="utf-8").read()):
        print(name)
    return 0


def _cmd_inject(argv: list[str]) -> int:
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


def main(argv: list[str] | None = None) -> int:
    argv = list(sys.argv[1:] if argv is None else argv)
    # No subcommand keeps `--live --app --out` working (the only call #243 shipped).
    if argv and not argv[0].startswith("-"):
        cmd, rest = argv[0], argv[1:]
    else:
        cmd, rest = "inject", argv
    if cmd == "owns":
        return _cmd_owns(rest)
    if cmd == "extract-live":
        return _cmd_extract_live(rest)
    if cmd == "import-resources":
        return _cmd_import_resources(rest)
    if cmd == "inject":
        return _cmd_inject(rest)
    if cmd == "parameter-keys":
        return _cmd_parameter_keys(rest)
    raise SystemExit(f"unknown command {cmd!r}")


if __name__ == "__main__":
    sys.exit(main())
