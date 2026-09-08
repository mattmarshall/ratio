#!/usr/bin/env python3
"""The app stack asks for things; the deploy role has to be able to give them.

⛔ THE FAILURE THIS IS FOR COSTS A FULL CI RUN TO DISCOVER. `app.yaml` is
deployed by a role in `bootstrap.yaml` that is scoped almost to nothing, and
`bootstrap.yaml` is applied BY HAND by a human with admin. So a resource added
to the app stack without the matching grant does not fail here, or in review —
it fails in `aws cloudformation deploy`, after a Bazel build, a full test suite,
a docker build and an ECR push, with `User is not authorized to perform`.

⚠ AND THE CAPABILITY FLAG IS WORSE, because it fails the same way for a reason
that reads as unrelated: a stack that creates a NAMED IAM role is refused unless
the deploy passes `--capabilities CAPABILITY_NAMED_IAM`, and the error names
neither the role nor the flag helpfully.

⛔ WHAT THIS CANNOT DO, STATED PLAINLY. It does not talk to AWS and it does not
simulate IAM. It is a cross-check between two declared documents — what the app
stack creates, and what the deploy role is allowed to create — at the level of
the SERVICE PREFIX and the named role ARNs. A policy that grants `ecs:CreateCluster`
but not `ecs:RegisterTaskDefinition` still passes here and still fails in AWS.
What it buys is that the two files cannot drift apart in silence, which is the
failure that actually happens: somebody adds a resource and forgets the grant.
"""

import json
import re
import sys
import pathlib


def check_run_blocks_indented(path: str, text: str) -> None:
    """A column-0 line inside `run: |` exits the literal block.

    GitHub then reports a workflow-file issue and schedules no jobs —
    that is how tip deploy died after #243 (`import json, sys` at
    column 0). This check would have gone red on that file.
    """
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        line = lines[i]
        stripped = line.lstrip(" \t")
        indent = len(line) - len(stripped)
        if re.match(r"run:\s*\|", stripped):
            i += 1
            while i < len(lines):
                nxt = lines[i]
                if nxt.strip() == "":
                    i += 1
                    continue
                nxt_stripped = nxt.lstrip(" \t")
                nxt_indent = len(nxt) - len(nxt_stripped)
                if nxt_indent <= indent:
                    # Sibling key (`env:`) or the next list item (`- name:`)
                    # ends the block cleanly. A payload that is not YAML
                    # (`import json, sys` at column 0) is the #243 break.
                    if re.match(r"[A-Za-z0-9_.-]+:", nxt_stripped) or nxt_stripped.startswith("- "):
                        break
                    fail(
                        f"{path}:{i + 1}: line inside run: | is not indented "
                        f"— YAML parse dies here: {nxt_stripped!r}"
                    )
                    break
                i += 1
            continue
        i += 1

# CloudFormation resource type -> the IAM service prefix that creates it.
SERVICE = {
    "AWS::EC2::": "ec2",
    "AWS::ECS::": "ecs",
    "AWS::S3::": "s3",
    "AWS::Logs::": "logs",
    "AWS::IAM::": "iam",
    "AWS::Lambda::": "lambda",
    "AWS::ApiGatewayV2::": "apigateway",
    "AWS::Cognito::": "cognito-idp",
}

def the_console_jwt_issuer_is_the_auth_api_host_session_tokens_mint(
    app_path: str,
    workflow_path: str,
    app_code: str,
    flow: str,
    production_issuer: str,
    stale_session_issuer: str,
) -> None:
    """API Gateway JWT iss must equal the Auth API custom-domain issuer.

    After authapi.ratio.marsh.build was verified (2026-09-06), OIDC
    discovery at api.workos.com/user_management/{client_id} publishes
    issuer https://authapi.ratio.marsh.build/user_management/{client_id}.
    Session access tokens mint that iss. Pointing the console
    authorizer at api.workos.com/user_management/{client_id} 401s
    every AuthKit bearer — the leftover status on /books after #253.
    """
    if f'Default: "{stale_session_issuer}"' in app_code:
        fail(
            f"{app_path} still defaults WorkOsIssuer to the pre-AuthAPI host "
            f"{stale_session_issuer} — session tokens mint {production_issuer}"
        )
    else:
        print("  ok  WorkOsIssuer is not the pre-AuthAPI api.workos.com host")
    if f"PRODUCTION_ISSUER='{stale_session_issuer}'" in flow:
        fail(
            f"{workflow_path} still falls back to the pre-AuthAPI host "
            f"{stale_session_issuer} — that 401s every AuthKit session"
        )
    else:
        print("  ok  the workflow fallback is not the pre-AuthAPI host")
    if f"WORKOS_ISSUER:-{stale_session_issuer}" in flow:
        fail(
            f"{workflow_path} smoke still expects the pre-AuthAPI host from "
            "WORKOS_ISSUER — a stale GitHub var would green a 401 authorizer"
        )
    else:
        print("  ok  smoke does not expect the pre-AuthAPI host from WORKOS_ISSUER")
    if "STALE_SESSION_ISSUER=" not in flow:
        fail(
            f"{workflow_path} does not name the pre-AuthAPI host as stale — "
            "a GitHub var still holding it would 401 every session"
        )
    else:
        print("  ok  the workflow treats the pre-AuthAPI host as stale")
    if production_issuer not in app_code:
        fail(
            f"{app_path} does not name the Auth API session issuer "
            f"{production_issuer}"
        )


failures = []


def fail(msg):
    failures.append(msg)
    print(f"  x {msg}", file=sys.stderr)


def main(app_path, bootstrap_path, workflow_path):
    app = pathlib.Path(app_path).read_text()
    boot = pathlib.Path(bootstrap_path).read_text()

    # ⛔ COMMENTS STRIPPED, AND THIS CHECK PASSED FOR THE WRONG REASON UNTIL IT
    # WAS. The workflow explains the capability flag in a comment directly above
    # the flag, so a substring search over the whole file finds
    # `CAPABILITY_NAMED_IAM` in the PROSE — and deleting the actual argument left
    # this test green. A check satisfied by the sentence describing the thing,
    # rather than by the thing, is the shape of check that is worse than none.
    flow = "\n".join(
        line for line in pathlib.Path(workflow_path).read_text().splitlines()
        if not line.lstrip().startswith("#")
    )

    # What the app stack creates, by service.
    types = set(re.findall(r"^\s+Type:\s+(AWS::[A-Za-z0-9:]+)", app, re.M))
    if not types:
        fail(f"found no resource types in {app_path} — did the template change shape?")
        return

    services = set()
    for t in types:
        for prefix, svc in SERVICE.items():
            if t.startswith(prefix):
                services.add(svc)
                break
        else:
            fail(f"{t} is a service this check does not know — add it to SERVICE")

    # The deploy role's actions. Everything under DeployRole, before ExecutionRole.
    deploy_block = boot.split("ExecutionRole:")[0]
    granted = set(re.findall(r"^\s+-\s+([a-z0-9-]+):[A-Za-z*]+", deploy_block, re.M))

    for svc in sorted(services):
        # ⚠ `iam` is granted as PassRole/CreateRole rather than as a bare
        # prefix, and is checked by name below instead.
        if svc == "iam":
            continue
        if svc not in granted:
            fail(
                f"{app_path} creates {svc} resources and the deploy role in "
                f"{bootstrap_path} has no {svc}: action — the deploy will fail with "
                f"AccessDenied after the image is already pushed"
            )
        else:
            print(f"  ok  {svc}: created by the app stack, granted to the deploy role")

    # ⛔ NAMED IAM ROLES NEED THE CAPABILITY, AND THE GRANT, BY ARN.
    named = re.findall(r"^\s+RoleName:\s+([A-Za-z0-9-]+)", app, re.M)
    if named:
        if "CAPABILITY_NAMED_IAM" not in flow:
            fail(
                f"{app_path} creates named IAM roles ({', '.join(named)}) but "
                f"{workflow_path} does not pass --capabilities CAPABILITY_NAMED_IAM — "
                "CloudFormation refuses the stack"
            )
        else:
            print(f"  ok  {len(named)} named IAM role(s), and the deploy passes the capability")

        for role in named:
            # The deploy role must be able to create it, by exact ARN.
            if f"role/{role}" not in deploy_block:
                fail(
                    f"{app_path} creates the role {role!r} and the deploy role cannot: "
                    f"no arn naming role/{role} in {bootstrap_path}"
                )
            else:
                print(f"  ok  {role}: named in the app stack and grantable by CI")

    # ⛔ PERMISSIONS THE TEMPLATE NEEDS THAT NO RESOURCE TYPE NAMES.
    #
    # This check exists because the first deploy of the scale runner failed on
    # exactly this and nothing here saw it coming:
    #
    #   ScaleSubnet CREATE_FAILED  AccessDenied. User doesn't have permission
    #   to call ec2:DescribeAvailabilityZones
    #
    # `!GetAZs ""` needs that call. It is an INTRINSIC FUNCTION, not a resource,
    # so the service-prefix check above passed happily — `ec2` was granted, and
    # the one verb the template actually required was not. The cost of finding
    # out was a Bazel build, the whole test suite, a docker build, an ECR push
    # and a stack rollback.
    #
    # ⚠ Each entry is a thing the TEMPLATE does, mapped to the permission the
    # DEPLOYER needs for it. Add a row whenever a template starts using an
    # intrinsic that reaches an API.
    NEEDS = [
        ("!GetAZs", "ec2:DescribeAvailabilityZones", "resolving !GetAZs"),
    ]
    for marker, action, why in NEEDS:
        if marker not in app:
            continue
        svc, verb = action.split(":")
        # A wildcard grant covers it: `ec2:Describe*` is how this one is fixed,
        # and enumerating read-only actions is what failed in the first place.
        covered = any(
            g == action or (g.endswith("*") and verb.startswith(g.split(":")[1].rstrip("*")))
            for g in re.findall(rf"^\s+-\s+({re.escape(svc)}:[A-Za-z*]+)", deploy_block, re.M)
        )
        if not covered:
            fail(
                f"{app_path} uses {marker} ({why}) which calls {action}, and the deploy "
                f"role grants no such action — the stack will roll back at the resource "
                f"that uses it, not at deploy time"
            )
        else:
            print(f"  ok  {marker} needs {action}, and the deploy role has it")

    # ⛔ THE PERMISSION NO POLICY CAN GRANT: A SERVICE-LINKED ROLE THAT DOES NOT
    # EXIST. ECS manages awsvpc task networking through AWSServiceRoleForECS,
    # auto-created on first cluster creation only if the creator may
    # iam:CreateServiceLinkedRole — which the deploy role deliberately may not.
    # The failure that taught this: the IAM simulator said RunTask was allowed
    # and PassRole was allowed, and the first real button press failed anyway,
    # because the role ECS ITSELF assumes had never been created in the account.
    # Nothing in either template's grants could have said so; only the presence
    # of the SLR resource can.
    if "AWS::ECS::TaskDefinition" in app:
        if "AWS::IAM::ServiceLinkedRole" in boot and "ecs.amazonaws.com" in boot:
            print("  ok  ECS tasks exist and bootstrap creates the ECS service-linked role")
        else:
            fail(
                f"{app_path} runs ECS tasks but {bootstrap_path} does not create the ECS "
                "service-linked role (AWS::IAM::ServiceLinkedRole, ecs.amazonaws.com) — in "
                "an account that has never used ECS, every RunTask fails even though the IAM "
                "simulator says it is allowed"
            )

    # ⭐ THE ONE THAT WOULD BE SILENT. A role the app stack creates for ECS must
    # be passable TO ecs, or RunTask is refused at the moment a visitor presses
    # the button — long after every deploy has gone green.
    if any("ecs-tasks.amazonaws.com" in b for b in [app]):
        if "ecs-tasks.amazonaws.com" not in boot:
            fail(
                "the app stack creates roles ECS assumes, but nothing in "
                f"{bootstrap_path} passes a role to ecs-tasks.amazonaws.com — "
                "RunTask would be refused at the button, not at deploy"
            )
        else:
            print("  ok  the task roles can be passed to ECS")

    # ⛔ A MISSING WORKOS CLIENT ID MUST NOT BECOME A PRODUCTION IDENTIFIER.
    # `${WORKOS_CLIENT_ID:-}` in a `[ -z ]` test is not a fallback — that is
    # how bash reads an unset variable under `set -u`. The thing this refuses
    # is `:-client_…`, a missing value turning into a live identifier. `flow`
    # is already comment-stripped, so a sentence describing the old fallback
    # cannot satisfy or fail this.
    if ":-client_" in flow:
        fail(
            f"{workflow_path} falls back to a hard-coded WorkOS client id when "
            "the variable is unset — a missing configuration value becoming a "
            "production identifier"
        )
    else:
        print("  ok  WORKOS_CLIENT_ID has no silent default in the workflow")

    if 'WorkOsClientId="${WORKOS_CLIENT_ID}"' not in flow:
        fail(
            f"{workflow_path} does not pass WorkOsClientId from WORKOS_CLIENT_ID "
            "with no fallback"
        )
    else:
        print("  ok  WorkOsClientId is passed from the variable, nothing else")

    # ⭐ THE FAILURE THAT SAID NOTHING. `cloudformation deploy` exits 255 and
    # tells the reader to run describe-stack-events on a box they do not have.
    # A check satisfied by the command appearing in a comment is the same
    # shape as the CAPABILITY_NAMED_IAM check above, which is why this uses
    # `flow` rather than the raw file.
    if "describe-stack-events" not in flow:
        fail(
            f"{workflow_path} does not dump CloudFormation stack events on "
            "deploy failure — the next red run will again say nothing"
        )
    else:
        print("  ok  a failed deploy dumps stack events")

    if "ratio.marsh.build" not in flow:
        fail(
            f"{workflow_path} no longer names the production console origin "
            "https://ratio.marsh.build"
        )
    else:
        print("  ok  the workflow names the production console origin")

    if 'Default: "client_01M1JJZTFXFDZJ0XJM1NPNSEJB"' in app:
        fail(
            f"{app_path} defaults WorkOsClientId to a production client id — "
            "a missing parameter must not become a live audience"
        )
    else:
        print("  ok  WorkOsClientId has no template default")

    # ⛔ THE BARE api.workos.com HOST IS NOT AN OIDC ISSUER.
    # CloudFormation AWS::ApiGatewayV2::Authorizer fetches
    # {issuer}/.well-known/openid-configuration and refuses a 404
    # (run 33784570568, #122). A comment that names the rejected host
    # must not satisfy or fail this, which is why both documents are
    # comment-stripped. `flow` is already stripped above.
    #
    # ⚠ THE PATH UNDER /user_management/{client_id} IS THE REAL SESSION
    # ISSUER, ON THE AUTH API HOST. Session tokens mint
    # https://authapi.ratio.marsh.build/user_management/{client_id}.
    # A prefix match on https://api.workos.com would fail the correct
    # default, so these patterns end at an optional trailing slash and
    # then the quote. The pre-AuthAPI
    # api.workos.com/user_management/{client_id} host is a different
    # refuse (stale, 401s every session) checked by name below.
    # ⚠ `com/"?` IS THE WRONG OPTIONAL. That requires the slash and then
    # an optional quote — it misses Default: "https://api.workos.com"
    # and only catches the trailing-slash form. `com/?"` is the other
    # way around.
    app_code = "\n".join(
        line for line in app.splitlines()
        if not line.lstrip().startswith("#")
    )
    if re.search(
        r'(?:Issuer|Default):\s+"https://api\.workos\.com/?"\s*$',
        app_code,
        re.M,
    ):
        fail(
            f"{app_path} sets the JWT authorizer issuer to the bare "
            "https://api.workos.com host, which has no OIDC discovery — "
            "CloudFormation will UPDATE_FAILED at Authorizer"
        )
    else:
        print("  ok  Authorizer issuer is not the bare api.workos.com host")

    PRODUCTION_ISSUER = (
        "https://authapi.ratio.marsh.build/user_management/client_01M1JJZTFXFDZJ0XJM1NPNSEJB"
    )
    STALE_SESSION_ISSUER = (
        "https://api.workos.com/user_management/client_01M1JJZTFXFDZJ0XJM1NPNSEJB"
    )
    the_console_jwt_issuer_is_the_auth_api_host_session_tokens_mint(
        app_path,
        workflow_path,
        app_code,
        flow,
        PRODUCTION_ISSUER,
        STALE_SESSION_ISSUER,
    )
    if f'Default: "{PRODUCTION_ISSUER}"' not in app_code:
        fail(
            f"{app_path} does not default WorkOsIssuer to the AuthKit "
            f"session-token issuer {PRODUCTION_ISSUER}"
        )
    else:
        print("  ok  the app stack defaults WorkOsIssuer to the session-token issuer")

    # ⚠ WorkOsConnectIssuer defaults to that hostname on purpose —
    # Connect tokens mint it. This check is the session-token
    # parameter only. A prefix match on the whole file would fail
    # the honest Connect default.
    workos_issuer_param = None
    for m in re.finditer(
        r"^  WorkOsIssuer:\n((?:    .*\n)+)",
        app_code,
        re.M,
    ):
        workos_issuer_param = m.group(1)
        break
    if workos_issuer_param is None:
        fail(f"{app_path} has no WorkOsIssuer parameter")
    elif re.search(
        r'Default:\s+"https://auth\.ratio\.marsh\.build/?"\s*$',
        workos_issuer_param,
        re.M,
    ):
        fail(
            f"{app_path} defaults WorkOsIssuer to the hosted AuthKit / Connect "
            "domain — session tokens mint iss under "
            "authapi.ratio.marsh.build/user_management/"
        )
    else:
        print("  ok  WorkOsIssuer default is not the hosted AuthKit hostname")

    if re.search(r'WorkOsIssuer="https://api\.workos\.com/?"', flow):
        fail(
            f"{workflow_path} passes the bare api.workos.com host as "
            "WorkOsIssuer — CloudFormation will refuse the authorizer"
        )
    else:
        print("  ok  the workflow does not pass the bare api.workos.com issuer")

    if PRODUCTION_ISSUER not in flow:
        fail(
            f"{workflow_path} does not fall back to the AuthKit "
            f"session-token issuer {PRODUCTION_ISSUER}"
        )
    else:
        print("  ok  the workflow falls back to the session-token issuer")

    if 'WorkOsIssuer="${ISSUER}"' not in flow:
        fail(
            f"{workflow_path} does not pass WorkOsIssuer from the resolved "
            "issuer — the authorizer and /authconfig.json would then depend "
            "on a template default the smoke test cannot see arrive"
        )
    else:
        print("  ok  WorkOsIssuer is passed from the resolved issuer")

    # ⛔ CONNECT TOKENS ARE A DIFFERENT ISSUER. AWS::ApiGatewayV2::Authorizer
    # JwtConfiguration.Issuer is a single string. AuthKit session tokens
    # mint iss under /user_management/{client_id}. WorkOS Connect access
    # tokens mint iss as the AuthKit custom domain
    # (https://auth.ratio.marsh.build), which serves OIDC discovery and
    # /oauth2/jwks (verified 2026-09-04). One authorizer pointed at the
    # session issuer 401s every Connect token at the edge — the leftover
    # #224 named. A second HTTP API with a second JWT authorizer is the
    # honest split: same Lambda, same /v1 path, Connect issuer.
    #
    # ⚠ COMMENT-STRIPPED. A sentence describing the Connect authorizer
    # must not satisfy this — same shape as CAPABILITY_NAMED_IAM.
    PRODUCTION_CONNECT_ISSUER = "https://auth.ratio.marsh.build"
    if f'Default: "{PRODUCTION_CONNECT_ISSUER}"' not in app_code:
        fail(
            f"{app_path} does not default WorkOsConnectIssuer to the "
            f"Connect-token issuer {PRODUCTION_CONNECT_ISSUER}"
        )
    else:
        print("  ok  the app stack defaults WorkOsConnectIssuer to the Connect-token issuer")

    if re.search(
        rf'Default:\s+"{re.escape(PRODUCTION_ISSUER)}"',
        app_code,
    ) and app_code.count(f'Default: "{PRODUCTION_ISSUER}"') > 1:
        fail(
            f"{app_path} defaults more than one issuer parameter to the "
            "AuthKit session-token issuer — Connect tokens mint a different iss"
        )

    # ⛔ AUTH KIT-ONLY IS THE REGRESSION. One JWT authorizer, or a
    # Connect authorizer that still cites WorkOsIssuer, is the leftover
    # this PR closed. Two Issuer: !Ref WorkOsIssuer lines means both
    # APIs prove session tokens and Connect still 401s at the edge.
    issuer_refs = re.findall(r"Issuer:\s+!Ref\s+(\w+)", app_code)
    if "WorkOsConnectIssuer" not in issuer_refs:
        fail(
            f"{app_path} has no JWT authorizer Issuer: !Ref WorkOsConnectIssuer "
            "— Connect tokens are refused at the edge (AuthKit-issuer-only)"
        )
    else:
        print("  ok  a JWT authorizer proves WorkOsConnectIssuer")
    if issuer_refs.count("WorkOsIssuer") < 1:
        fail(
            f"{app_path} dropped Issuer: !Ref WorkOsIssuer — AuthKit session "
            "tokens would 401 at the console API"
        )
    else:
        print("  ok  a JWT authorizer still proves WorkOsIssuer (session tokens)")
    if issuer_refs.count("WorkOsIssuer") > 1 and "WorkOsConnectIssuer" not in issuer_refs:
        fail(
            f"{app_path} points every JWT authorizer at WorkOsIssuer — "
            "AuthKit-issuer-only; Connect tokens never reach /v1"
        )
    if issuer_refs.count("WorkOsConnectIssuer") < 1:
        fail(
            f"{app_path} JWT authorizers are AuthKit-issuer-only "
            f"(Issuer refs: {issuer_refs})"
        )

    if "workos-connect-jwt" not in app_code:
        fail(
            f"{app_path} does not name a workos-connect-jwt authorizer — "
            "the Connect grant-path split is missing"
        )
    else:
        print("  ok  workos-connect-jwt authorizer is declared")

    if "ratio-demo-connect" not in app_code:
        fail(
            f"{app_path} does not declare the Connect HTTP API "
            "(ratio-demo-connect) — a second route on the session API "
            "cannot OR issuers, and a path prefix is not /v1"
        )
    else:
        print("  ok  the Connect HTTP API (ratio-demo-connect) is declared")

    if "ConnectApiUrl" not in app_code:
        fail(
            f"{app_path} has no ConnectApiUrl output — Connect apps would "
            "have no host whose JWT authorizer proves their iss"
        )
    else:
        print("  ok  ConnectApiUrl is a stack output")

    if "ConnectProtectedRoute" not in app_code:
        fail(
            f"{app_path} has no ConnectProtectedRoute — Connect /v1 would "
            "not require a JWT"
        )
    else:
        print("  ok  Connect /v1 is a JWT-protected route")

    # Audience on the Connect authorizer must stay WorkOsClientId.
    # Connect `aud` is the Ratio WorkOS project client, not azp.
    # A second audience, or a hard-coded Connect app id, would be a
    # silent wrong accept or a silent 401.
    connect_auth = None
    for m in re.finditer(
        r"^  ConnectAuthorizer:\n((?:    .*\n)+)",
        app_code,
        re.M,
    ):
        connect_auth = m.group(1)
        break
    if connect_auth is None:
        fail(f"{app_path} has no ConnectAuthorizer resource")
    else:
        if "WorkOsClientId" not in connect_auth:
            fail(
                f"{app_path} ConnectAuthorizer audience is not WorkOsClientId "
                "— Connect aud is the Ratio project client, not azp"
            )
        else:
            print("  ok  ConnectAuthorizer audience is WorkOsClientId")
        if "WorkOsConnectIssuer" not in connect_auth:
            fail(
                f"{app_path} ConnectAuthorizer issuer is not WorkOsConnectIssuer "
                "— AuthKit-issuer-only on the Connect API"
            )
        else:
            print("  ok  ConnectAuthorizer issuer is WorkOsConnectIssuer")
        if "WorkOsIssuer" in connect_auth:
            fail(
                f"{app_path} ConnectAuthorizer still cites WorkOsIssuer — "
                "Connect tokens mint a different iss and would 401"
            )

    connect_param = None
    for m in re.finditer(
        r"^  WorkOsConnectIssuer:\n((?:    .*\n)+)",
        app_code,
        re.M,
    ):
        connect_param = m.group(1)
        break
    if connect_param is None:
        fail(f"{app_path} has no WorkOsConnectIssuer parameter")
    elif re.search(
        r'Default:\s+"https://(api\.workos\.com|authapi\.ratio\.marsh\.build)(/user_management/[^"]*)?"',
        connect_param,
    ):
        fail(
            f"{app_path} defaults WorkOsConnectIssuer to a session-token "
            "or bare WorkOS host — Connect tokens mint "
            f"{PRODUCTION_CONNECT_ISSUER}"
        )
    else:
        print("  ok  WorkOsConnectIssuer is not a session-token or bare host")

    if 'WorkOsConnectIssuer="${CONNECT_ISSUER}"' not in flow:
        fail(
            f"{workflow_path} does not pass WorkOsConnectIssuer from the "
            "resolved Connect issuer — the Connect authorizer would then "
            "depend on a template default the smoke test cannot see arrive"
        )
    else:
        print("  ok  WorkOsConnectIssuer is passed from the resolved Connect issuer")

    if PRODUCTION_CONNECT_ISSUER not in flow:
        fail(
            f"{workflow_path} does not fall back to the Connect-token "
            f"issuer {PRODUCTION_CONNECT_ISSUER}"
        )
    else:
        print("  ok  the workflow falls back to the Connect-token issuer")

    if "ConnectApiUrl" not in flow:
        fail(
            f"{workflow_path} does not smoke the Connect API — a missing "
            "or AuthKit-only Connect host would stay silent"
        )
    else:
        print("  ok  smoke asserts the Connect API")

    # ⭐ THE DEPLOYED DEMO API MUST HYDRATE THE SMALL journals/ PREFIX.
    # Unset FileBook writes /tmp only — CreateBook then dies on cold
    # start (Household wiped on ratio.marsh.build after #230). Ops
    # restored live Lambda env; the next CloudFormation deploy must
    # keep RATIO_JOURNAL_BUCKET + RATIO_JOURNAL_PREFIX so persistence
    # survives. Hydrate 503 is transient (accept-during-hydrate /
    # orTransient). The 40GB scale fold stays on Fargate ScaleTask.
    # A sentence describing the vars in a comment must not satisfy
    # this — `app_code` is comment-stripped. Same pattern as
    # RATIO_DEMO_OPEN (that one stays unset).
    if not re.search(
        r"^\s+RATIO_JOURNAL_BUCKET:\s+!Ref\s+ScaleBucket\s*$", app_code, re.M
    ):
        fail(
            f"{app_path} dropped RATIO_JOURNAL_BUCKET: !Ref ScaleBucket — "
            "a deploy would wipe CreateBook on the next cold start"
        )
    else:
        print("  ok  RATIO_JOURNAL_BUCKET names ScaleBucket on the function")
    if not re.search(r"^\s+RATIO_JOURNAL_PREFIX:\s+journals/\s*$", app_code, re.M):
        fail(
            f"{app_path} dropped RATIO_JOURNAL_PREFIX: journals/ — "
            "CreateBook durability is that prefix, not the 40GB scale fold"
        )
    else:
        print("  ok  RATIO_JOURNAL_PREFIX is journals/ on the function")

    # Scale still needs ScaleBucket (RATIO_SCALE_BUCKET, cluster, task).
    # Restoring the journal pair must not drop the scale path.
    if not re.search(r"^\s+RATIO_SCALE_BUCKET:\s+!Ref\s+ScaleBucket\s*$", app_code, re.M):
        fail(
            f"{app_path} dropped RATIO_SCALE_BUCKET — restoring API journal "
            "hydrate must not break the scale runner's bucket"
        )
    else:
        print("  ok  RATIO_SCALE_BUCKET still names ScaleBucket for scale")

    # ⛔ THE JOURNAL GRANT MUST LIVE IN THE APP STACK, NOT ONLY IN BOOTSTRAP.
    # Issue #129: Sid TheJournal was added to bootstrap.yaml in #84, and
    # README said "re-run bootstrap once". Nobody did. After #126 bound
    # before hydrate, /healthz lived and /balance.json died on
    # s3:PutObject AccessDenied for journals/book/journal/0000…1.
    # A grant only in the hand-applied stack is a grant the next deploy
    # cannot apply. The Function Environment names ScaleBucket
    # journals/; this policy is the write grant that hydrate and
    # append both use. Prefix is the same journals/ prefix.
    #
    # ⚠ COMMENT-STRIPPED. A sentence describing the grant must not satisfy
    # this — same shape as the CAPABILITY_NAMED_IAM check above.
    journal_prefix = "journals/"

    journal_policy = None
    for m in re.finditer(
        r"^  [A-Za-z0-9]+:\n    Type: AWS::S3::BucketPolicy\n"
        r"((?:.*\n)*?)(?=^  [A-Za-z]|\Z)",
        app_code,
        re.M,
    ):
        block = m.group(0)
        if journal_prefix and f"{journal_prefix}*" in block:
            journal_policy = block
            break
    if journal_policy is None:
        fail(
            f"{app_path} has no bucket policy covering {journal_prefix or 'journals/'}* "
            "— the identity grant in bootstrap.yaml is applied by hand and was "
            "the #129 miss; CI cannot PutRolePolicy on ratio-demo-execution"
        )
    else:
        print("  ok  the app stack has a bucket policy on the journal prefix")
        if "s3:PutObject" not in journal_policy:
            fail(
                f"{app_path} journal bucket policy does not grant s3:PutObject — "
                "hydrate's If-None-Match claim is an ordinary PutObject"
            )
        else:
            print("  ok  journal policy grants s3:PutObject")
        if "s3:GetObject" not in journal_policy:
            fail(
                f"{app_path} journal bucket policy does not grant s3:GetObject — "
                "/balance.json reads the objects hydrate just claimed"
            )
        else:
            print("  ok  journal policy grants s3:GetObject")
        if "s3:ListBucket" not in journal_policy:
            fail(
                f"{app_path} journal bucket policy does not grant s3:ListBucket — "
                "SeqLog.height is a LIST before the fold"
            )
        else:
            print("  ok  journal policy grants s3:ListBucket")
        if "s3:DeleteObject" in journal_policy:
            fail(
                f"{app_path} journal bucket policy grants s3:DeleteObject — "
                "a delete on an append-only log is a truncation wearing an IAM grant"
            )
        else:
            print("  ok  journal policy does not grant DeleteObject")
        if "ExecutionRoleArn" not in journal_policy:
            fail(
                f"{app_path} journal bucket policy is not scoped to ExecutionRoleArn — "
                "a principal other than the function is not the writer"
            )
        else:
            print("  ok  journal policy is scoped to the function's execution role")
        if "ScaleBucket" not in journal_policy:
            fail(
                f"{app_path} journal bucket policy is not on ScaleBucket — "
                "the env and the grant would name different buckets"
            )
        else:
            print("  ok  journal policy is on ScaleBucket")

    # ⛔ SMOKE STILL ASKS FOR A TYING BOOK, AND STILL REFUSES AN OPEN /v1.
    # A "fix" that dropped the difference:0.00 assertion, or that opened
    # /v1/funds to make the deploy green, would pass every other check here.
    if '"difference":"0.00"' not in flow or "balance.json" not in flow:
        fail(
            f"{workflow_path} no longer asserts a tying trial balance on "
            "/balance.json — that is the #129 smoke failure, not a check to drop"
        )
    else:
        print("  ok  smoke still asserts difference:0.00 on /balance.json")
    # ⛔ AFTER #136 THE BOOK JSON 503s WHILE /version IS ALREADY THE NEW SHA.
    # Run 33800551926: CloudFormation succeeded, Lambda served 454684d,
    # /authconfig.json and /scale-runs.json were 200, /balance.json was
    # still hydrating. `curl -sf` reported "could not fetch". A check that
    # only looks for difference:0.00 still passes if want() fails closed
    # on the first 503 — the string is in the file. The retry has to be
    # in the fetch path, and it must not also retry a 500. `flow` is
    # comment-stripped, so a sentence describing the retry does not count.
    if "hydrating" not in flow:
        fail(
            f"{workflow_path} does not look for the journal-hydrate body "
            "before asserting /balance.json — that is run 33800551926, "
            "not a check to drop"
        )
    else:
        print("  ok  smoke looks for the hydrate 503 body")
    if '"503"' not in flow:
        fail(
            f"{workflow_path} does not special-case HTTP 503 — a retry of "
            "every curl failure would also wait out a lasting 500"
        )
    else:
        print("  ok  smoke special-cases HTTP 503 rather than every failure")
    if "v1/funds" not in flow or "401" not in flow:
        fail(
            f"{workflow_path} no longer asserts unauthenticated /v1/funds is 401 — "
            "the journal grant must not be bought by opening the tenant boundary"
        )
    else:
        print("  ok  smoke still asserts unauthenticated /v1/funds is 401")

    # ⛔ THE DEPLOYED DEMO MUST NOT GRANT EVERY AUTHKIT SESSION EVERY FUND.
    # `RATIO_DEMO_OPEN` (any non-empty value) is the open-rail dial. It is
    # opt-in for local `ratio watch` / CI. A sentence describing the dial
    # in a comment must not satisfy or fail this, which is why `app_code`
    # is comment-stripped. `RATIO_DEMO_MEMBER` staying set is the
    # membership seed the walk-through uses once the dial is off.
    if re.search(r"^\s+RATIO_DEMO_OPEN:", app_code, re.M):
        fail(
            f"{app_path} still sets RATIO_DEMO_OPEN on the function — "
            "the deployed demo would grant any AuthKit session every fund"
        )
    else:
        print("  ok  RATIO_DEMO_OPEN is unset on the deployed function")
    if not re.search(r"^\s+RATIO_DEMO_MEMBER:", app_code, re.M):
        fail(
            f"{app_path} dropped RATIO_DEMO_MEMBER — unsetting the open "
            "dial without a membership seed leaves every AuthKit session "
            "authorized-empty for the seeded funds"
        )
    else:
        print("  ok  RATIO_DEMO_MEMBER still seeds membership on the demo")

    # ⛔ COGNITO IS NOT THE IdP. Unused UserPool / Client / Domain /
    # IdentityProvider resources, their outputs, and the Cognito-era
    # DEMO_MEMBERS default must not come back. AuthKit / Connect JWT
    # authorizers stay; this check is the leftover #22 teardown.
    # `app_code` / `flow` are comment-stripped, so a sentence describing
    # the unused pool cannot satisfy or fail this.
    cognito_types = [t for t in types if t.startswith("AWS::Cognito::")]
    if cognito_types:
        fail(
            f"{app_path} still creates Cognito resources ({', '.join(sorted(cognito_types))}) "
            "— AuthKit is the sole IdP; the unused pool must be deleted, not left in the template"
        )
    else:
        print("  ok  the app stack creates no Cognito resources")

    leftover_hits = []
    for leftover in (
        "UserPoolId:",
        "UserPoolClientId:",
        "HostedUiDomain:",
        "GoogleClientId:",
        "GoogleClientSecret:",
        "AWS::Cognito::UserPool",
        "AWS::Cognito::UserPoolClient",
        "AWS::Cognito::UserPoolDomain",
        "AWS::Cognito::UserPoolIdentityProvider",
    ):
        if leftover in app_code:
            leftover_hits.append(leftover)
            fail(
                f"{app_path} still names {leftover.rstrip(':')} — Cognito-era "
                "parameter / output / type must not survive the teardown"
            )
    if not leftover_hits:
        print("  ok  Cognito-era parameters, outputs, and types are gone")

    if re.search(r"Default:\s+demo@ratio\.fastverk\.dev", app_code):
        fail(
            f"{app_path} still defaults DemoMember to the Cognito-era "
            "address demo@ratio.fastverk.dev — that never appears on an "
            "AuthKit token; empty is the honest default"
        )
    else:
        print("  ok  DemoMember default is not the Cognito-era address")

    if "GoogleClientId=" in flow or "GoogleClientSecret=" in flow:
        fail(
            f"{workflow_path} still passes GoogleClientId / GoogleClientSecret "
            "— those parameters existed only for the unused Cognito Google IdP"
        )
    else:
        print("  ok  the workflow does not pass Cognito Google parameters")

    if "demo@ratio.fastverk.dev" in flow:
        fail(
            f"{workflow_path} still falls back to the Cognito-era "
            "demo@ratio.fastverk.dev address for DemoMember"
        )
    else:
        print("  ok  the workflow does not fall back to the Cognito-era address")

    if 'DemoMember="${DEMO_MEMBERS:-}"' not in flow:
        fail(
            f"{workflow_path} does not pass DemoMember from DEMO_MEMBERS "
            "with an empty fallback — a missing variable must not become "
            "a Cognito-era address"
        )
    else:
        print("  ok  DemoMember is passed from DEMO_MEMBERS with an empty fallback")

    # ⛔ THE STACK UPDATE MUST BE ABLE TO DELETE THE UNUSED POOL.
    # Create* is gone (the template no longer creates a pool). Delete*
    # stays on the deploy role so CloudFormation can tear the live
    # unused resources down. A comment that names DeleteUserPool must
    # not satisfy this — `deploy_block` is the role, not prose.
    if "cognito-idp:DeleteUserPool" not in deploy_block:
        fail(
            f"{bootstrap_path} dropped cognito-idp:DeleteUserPool — the "
            "stack update cannot delete the unused live pool"
        )
    else:
        print("  ok  the deploy role can still delete the unused UserPool")
    if "cognito-idp:CreateUserPool" in deploy_block:
        fail(
            f"{bootstrap_path} still grants cognito-idp:CreateUserPool — "
            "the unused pool is being torn down, not recreated"
        )
    else:
        print("  ok  the deploy role cannot recreate a Cognito UserPool")

    # Function timeout stays 60 — list/detail folds need the headroom;
    # 15s killed hydrate mid-seed. Ops already set 60. A comment that
    # names 60 must not satisfy this (`app_code` is comment-stripped).
    if not re.search(r"^      Timeout:\s+60\s*$", app_code, re.M):
        fail(
            f"{app_path} Function Timeout is no longer 60 — authenticated "
            "/v1 list still has to finish inside the HTTP API's 30s cap"
        )
    else:
        print("  ok  Function Timeout is 60")

    # ⭐ #152 — Demo HTTP API custom domain, not Connect, not CloudFront.
    # Ops already attached api.ratio.marsh.build to Demo API 1h4q8av2gb.
    # The template must declare DomainName + ApiMapping so the next
    # deploy adopts that mapping instead of wiping it or minting a
    # second hostname. A sentence in a comment must not satisfy this
    # (`app_code` / `flow` are comment-stripped).
    LIVE_DEMO_DOMAIN = "api.ratio.marsh.build"
    LIVE_CERT = (
        "arn:aws:acm:us-east-1:320473299741:certificate/"
        "4452c092-e591-46e2-8d40-2e29269a033b"
    )
    domain_types = [t for t in types if t == "AWS::ApiGatewayV2::DomainName"]
    mapping_types = [t for t in types if t == "AWS::ApiGatewayV2::ApiMapping"]
    if len(domain_types) != 1:
        fail(
            f"{app_path} must declare exactly one AWS::ApiGatewayV2::DomainName "
            f"(Demo API only); found {len(domain_types)}"
        )
    else:
        print("  ok  one API Gateway DomainName (Demo API)")
    if len(mapping_types) != 1:
        fail(
            f"{app_path} must declare exactly one AWS::ApiGatewayV2::ApiMapping "
            f"(Demo API only); found {len(mapping_types)}"
        )
    else:
        print("  ok  one API Gateway ApiMapping (Demo API)")

    if any(t.startswith("AWS::CloudFront::") for t in types):
        fail(
            f"{app_path} invented CloudFront — #152 is HTTP API DomainName, "
            "not an edge for its own sake"
        )
    else:
        print("  ok  no CloudFront")

    if any(t.startswith("AWS::CertificateManager::") for t in types):
        fail(
            f"{app_path} creates an ACM certificate — that would fight the "
            "already-issued DNS-validated cert; reference CertificateArn"
        )
    else:
        print("  ok  no ACM certificate resource")

    if LIVE_DEMO_DOMAIN not in app_code:
        fail(f"{app_path} does not name the live Demo host {LIVE_DEMO_DOMAIN}")
    else:
        print("  ok  DomainName is the live Demo host")

    if f'Default: "{LIVE_CERT}"' not in app_code:
        fail(
            f"{app_path} does not default CertificateArn to the issued cert "
            f"{LIVE_CERT} — a missing value must not mint a second cert"
        )
    else:
        print("  ok  CertificateArn defaults to the issued ACM cert")

    mapping_block = None
    for m in re.finditer(
        r"^  DemoApiMapping:\n((?:    .*\n)+)",
        app_code,
        re.M,
    ):
        mapping_block = m.group(1)
        break
    if mapping_block is None:
        fail(f"{app_path} has no DemoApiMapping resource")
    else:
        if not re.search(r"ApiId:\s+!Ref\s+Api\s*$", mapping_block, re.M):
            fail(
                f"{app_path} DemoApiMapping does not bind ApiId: !Ref Api — "
                "the custom domain must stay on the Demo HTTP API"
            )
        else:
            print("  ok  DemoApiMapping binds the Demo HTTP API")
        if "ConnectApi" in mapping_block:
            fail(
                f"{app_path} DemoApiMapping cites ConnectApi — Connect issuer "
                "must stay on its own execute-api host"
            )

    if 'Value: !Sub "https://${DemoDomain}/"' not in app_code:
        fail(
            f"{app_path} DemoUrl is not https://${{DemoDomain}}/ — smoke "
            "concatenates paths onto the trailing slash"
        )
    else:
        print("  ok  DemoUrl is the custom host with a trailing slash")

    if (
        'Value: !Sub "https://${ConnectApi}.execute-api.${AWS::Region}.amazonaws.com/"'
        not in app_code
    ):
        fail(
            f"{app_path} remapped ConnectApiUrl — Connect tokens stay on "
            "the execute-api host whose JWT authorizer proves their iss"
        )
    else:
        print("  ok  ConnectApiUrl is still the Connect execute-api host")

    if not re.search(
        r'^\s+RATIO_PUBLIC_ORIGIN:\s+!Sub\s+"https://\$\{DemoDomain\}"\s*$',
        app_code,
        re.M,
    ):
        fail(
            f"{app_path} Function RATIO_PUBLIC_ORIGIN is not "
            "https://${DemoDomain} (no trailing slash)"
        )
    else:
        print("  ok  Function RATIO_PUBLIC_ORIGIN is the custom host, no slash")

    if not re.search(
        r"Name:\s+RATIO_PUBLIC_ORIGIN\n\s+Value:\s+!Sub\s+\"https://\$\{DemoDomain\}\"\s*$",
        app_code,
        re.M,
    ):
        fail(
            f"{app_path} ScaleTask RATIO_PUBLIC_ORIGIN is not "
            "https://${DemoDomain} (no trailing slash)"
        )
    else:
        print("  ok  ScaleTask RATIO_PUBLIC_ORIGIN is the custom host, no slash")

    if re.search(r"RATIO_PUBLIC_ORIGIN:.*execute-api", app_code):
        fail(
            f"{app_path} still points RATIO_PUBLIC_ORIGIN at execute-api — "
            "permalinks would disagree with DemoUrl"
        )

    domain_block = None
    for m in re.finditer(
        r"^  DemoDomain:\n((?:    .*\n)+)",
        app_code,
        re.M,
    ):
        domain_block = m.group(0)
        break
    if domain_block is None:
        fail(f"{app_path} has no DemoDomain resource")
    elif "DeletionPolicy: Retain" not in domain_block:
        fail(
            f"{app_path} DemoDomain has no DeletionPolicy: Retain — a stack "
            "delete would wipe the live hostname"
        )
    else:
        print("  ok  DemoDomain is Retain, so a stack delete keeps the hostname")

    if "/domainnames" not in deploy_block:
        fail(
            f"{bootstrap_path} has no /domainnames grant — DemoDomain import "
            "fails with AccessDenied after the image is already pushed"
        )
    else:
        print("  ok  the deploy role can manage /domainnames*")

    if "cloudformation:GetTemplate" not in deploy_block:
        fail(
            f"{bootstrap_path} dropped cloudformation:GetTemplate — the first "
            "deploy cannot adopt the ops-attached DomainName"
        )
    else:
        print("  ok  the deploy role can GetTemplate for the import")

    if "acm:DescribeCertificate" not in deploy_block:
        fail(
            f"{bootstrap_path} cannot acm:DescribeCertificate on the issued "
            "Demo cert — CloudFormation will refuse DemoDomain"
        )
    else:
        print("  ok  the deploy role can read the issued ACM cert")

    if LIVE_DEMO_DOMAIN not in flow:
        fail(
            f"{workflow_path} does not name {LIVE_DEMO_DOMAIN} — smoke would "
            "keep curling execute-api after DemoUrl flips"
        )
    else:
        print("  ok  smoke names the custom Demo host")

    if "--change-set-type IMPORT" not in flow:
        fail(
            f"{workflow_path} does not import the ops-attached DomainName — "
            "the next stack update would CreateDomainName and fail"
        )
    else:
        print("  ok  deploy imports the ops-attached DomainName")

    if "resources-to-import" not in flow:
        fail(
            f"{workflow_path} has no resources-to-import for DemoDomain / "
            "DemoApiMapping"
        )
    else:
        print("  ok  import names DemoDomain / DemoApiMapping")

    # ⛔ #246 — describe-stack-resources --logical-resource-id DemoDomain
    # exits 0 with an empty list when the stack does not own the resource.
    # The skip must read the live get-template body. A comment that names
    # the wrong gate must not satisfy this (`flow` is comment-stripped).
    if re.search(r"logical-resource-id\s+DemoDomain", flow):
        fail(
            f"{workflow_path} still gates import on describe-stack-resources "
            "DemoDomain — the plural API is a no-op success when the logical "
            "id is absent, then deploy CREATE_FAILED AlreadyExists (#246)"
        )
    else:
        print("  ok  import is not gated on describe-stack-resources DemoDomain")

    if "adopt_demo_domain.py owns" not in flow:
        fail(
            f"{workflow_path} does not ask adopt_demo_domain.py owns — "
            "the skip must read the live get-template body for DemoDomain / "
            "DemoApiMapping (#246)"
        )
    else:
        print("  ok  import skip reads live get-template via adopt owns")

    if "live template already contains DemoDomain" not in flow:
        fail(
            f"{workflow_path} lost the get-template skip message — a "
            "describe-stack-resources no-op must not look like ownership"
        )
    else:
        print("  ok  skip message names the live template, not the stack resource API")

    if "get-domain-name" not in flow:
        fail(
            f"{workflow_path} does not confirm the physical DomainName "
            "exists before IMPORT — a missing hostname must not fall "
            "through to CreateDomainName (#246)"
        )
    else:
        print("  ok  import confirms the physical DomainName exists first")

    # ⛔ #248 — create-change-set accepts --template-body / --template-url,
    # not --template-file (that flag belongs to `cloudformation deploy`).
    # Run 34004094619: Unknown options → ChangeSetNotFound. Scope the
    # check to the create-change-set invocation so deploy --template-file
    # stays legal. `flow` is comment-stripped.
    cs_match = re.search(
        r"aws cloudformation create-change-set\b(.*?)\n\s*aws cloudformation ",
        flow,
        re.S,
    )
    if cs_match is None:
        fail(
            f"{workflow_path} has no create-change-set invocation — the "
            "ops-attached DomainName cannot be imported (#248)"
        )
    else:
        create_cs = cs_match.group(0)
        if re.search(r"--template-file\b", create_cs):
            fail(
                f"{workflow_path} create-change-set still uses --template-file "
                "— CreateChangeSet accepts --template-body / --template-url "
                "(run 34004094619, #248)"
            )
        elif not re.search(r"--template-body\b", create_cs) and not re.search(
            r"--template-url\b", create_cs
        ):
            fail(
                f"{workflow_path} create-change-set has neither --template-body "
                "nor --template-url — IMPORT would create an empty change set "
                "(#248)"
            )
        else:
            print("  ok  create-change-set passes the template as --template-body")

    # ⛔ #250 — CreateChangeSet ValidationError must fail the Deploy
    # step immediately. The waiter ChangeSetNotFound-polls until
    # timeout-minutes: 45 when the function is invoked as `func || dump`
    # (bash disables set -e in that context). `flow` is comment-stripped
    # so a sentence describing the hang cannot satisfy this.
    if "not waiting for a change set that was never created" not in flow:
        fail(
            f"{workflow_path} CreateChangeSet failure still falls through "
            "to wait change-set-create-complete (#250)"
        )
    else:
        print("  ok  CreateChangeSet failure does not wait for a missing change set")

    if "adopt_demo_domain.py parameter-keys" not in flow:
        fail(
            f"{workflow_path} does not filter IMPORT parameters through "
            "parameter-keys — CreateChangeSet would again pass CertificateArn "
            "when the import template omitted it (#250)"
        )
    else:
        print("  ok  IMPORT parameters are filtered to keys the import template declares")

    check_run_blocks_indented(workflow_path, pathlib.Path(workflow_path).read_text())
    if not any(f.startswith(f"{workflow_path}:") and "not indented" in f for f in failures):
        print("  ok  every run: | line in deploy.yml stays indented")

    try:
        import yaml

        yaml.safe_load(pathlib.Path(workflow_path).read_text())
        print("  ok  deploy.yml parses as YAML")
    except ImportError:
        # Hermetic Bazel Python may not have PyYAML. The indent walker
        # above is the check that would have gone red on #243.
        pass
    except Exception as exc:
        fail(f"{workflow_path} does not parse as YAML: {exc}")

    if re.search(r"python3 -c\s+'\s*$", flow, re.M):
        fail(
            f"{workflow_path} still embeds a multiline python3 -c body — "
            "put it in adopt_demo_domain.py so a column-0 line cannot "
            "exit the run: | block (#244)"
        )
    else:
        print("  ok  deploy.yml has no multiline python3 -c body")

    # ⛔ THE ADOPT SCRIPT MUST USE THE SAME LOGICAL IDS AS THE APP STACK.
    # Importing as DemoApiCustomDomain and deploying as DemoDomain is a
    # CREATE of a duplicate hostname.
    adopt_path = pathlib.Path(app_path).with_name("adopt_demo_domain.py")
    if not adopt_path.is_file():
        fail(
            f"{adopt_path} is missing from runfiles — add it to "
            "//deploy:iac_test data so the logical-id check can see it"
        )
    else:
        import importlib.util

        spec = importlib.util.spec_from_file_location("adopt_demo_domain", adopt_path)
        adopt = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(adopt)
        import io
        import tempfile
        from contextlib import redirect_stdout
        if adopt.LOGICAL_DOMAIN != "DemoDomain" or adopt.LOGICAL_MAPPING != "DemoApiMapping":
            fail(
                "adopt_demo_domain.py logical ids drifted from DemoDomain / "
                "DemoApiMapping — the follow-up deploy would CREATE a duplicate"
            )
        else:
            print("  ok  adopt script uses DemoDomain / DemoApiMapping")

        def drop_block(text, name):
            return text.replace(adopt.extract_top_level(text, name), "", 1)

        fake_live = drop_block(drop_block(drop_block(app, "DemoDomain"), "DemoApiMapping"), "CertificateArn")
        fake_live = fake_live.replace(
            '!Sub "https://${DemoDomain}"',
            '!Sub "https://${Api}.execute-api.${AWS::Region}.amazonaws.com"',
        )
        fake_live = fake_live.replace(
            '!Sub "https://${DemoDomain}/"',
            '!Sub "https://${Api}.execute-api.${AWS::Region}.amazonaws.com/"',
        )
        merged = adopt.inject_yaml(fake_live, app)
        if merged.count("AWS::ApiGatewayV2::DomainName") != 1:
            fail("adopt inject_yaml did not leave exactly one DomainName")
        elif (
            'Value: !Sub "https://${Api}.execute-api.${AWS::Region}.amazonaws.com/"'
            not in merged
        ):
            fail(
                "adopt inject_yaml flipped DemoUrl — an import change set "
                "cannot modify outputs"
            )
        elif "ConnectApi" in adopt.extract_top_level(merged, "DemoApiMapping"):
            fail("adopt inject_yaml mapped the custom domain to ConnectApi")
        else:
            print("  ok  adopt injects DomainName without flipping DemoUrl or mapping Connect")

        # ⛔ #250 — splicing CertificateArn at Resources: puts it under
        # Conditions. CreateChangeSet then rejects ParameterKey=CertificateArn.
        # The check above stayed green on that placement: it never asked
        # whether CertificateArn was a Parameter.
        cond = merged.find("\nConditions:\n")
        cert_param = merged.find("\n  CertificateArn:\n")
        if "CertificateArn" not in adopt.parameter_names(fake_live):
            print("  ok  stripped live template has no CertificateArn Parameter (the #250 fixture)")
        else:
            fail("stripped live template still declares CertificateArn — the #250 fixture drifted")
        if "CertificateArn" not in adopt.parameter_names(merged):
            fail(
                "inject_yaml did not declare CertificateArn under Parameters — "
                "CreateChangeSet rejects ParameterKey=CertificateArn (#250)"
            )
        elif cond < 0:
            fail("stripped live template lost Conditions — the #250 fixture drifted")
        elif cert_param < 0:
            fail("inject_yaml wrote no indent-2 CertificateArn block")
        elif cert_param > cond:
            fail(
                "inject_yaml placed CertificateArn after Conditions — "
                "it is not a Parameter; CreateChangeSet dies (#250)"
            )
        else:
            print("  ok  inject_yaml declares CertificateArn under Parameters, not Conditions")

        live_json = json.dumps(
            {
                "Parameters": {"ImageUri": {"Type": "String"}},
                "Conditions": {"X": {"Fn::Equals": ["1", "0"]}},
                "Resources": {"Api": {"Type": "AWS::ApiGatewayV2::Api"}},
                "Outputs": {"DemoUrl": {"Value": "https://example/"}},
            }
        )
        merged_json = json.loads(adopt.inject_json(live_json, app))
        if "CertificateArn" not in (merged_json.get("Parameters") or {}):
            fail("inject_json dropped CertificateArn from Parameters")
        elif "CertificateArn" not in adopt.parameter_names(json.dumps(merged_json)):
            fail("parameter_names missed JSON CertificateArn")
        else:
            print("  ok  inject_json declares CertificateArn under Parameters")

        cond_only = (
            "Parameters:\n  ImageUri:\n    Type: String\n"
            "Conditions:\n  CertificateArn:\n    Type: String\n"
            "Resources:\n  X:\n    Type: AWS::Logs::LogGroup\n"
        )
        if "CertificateArn" in adopt.parameter_names(cond_only):
            fail("parameter_names treated a Conditions key as a Parameter (#250)")
        elif adopt.parameter_names(cond_only) != ["ImageUri"]:
            fail(f"parameter_names drifted: {adopt.parameter_names(cond_only)}")
        else:
            print("  ok  parameter_names ignores CertificateArn under Conditions")

        with tempfile.TemporaryDirectory() as td:
            merged_path = pathlib.Path(td) / "merged.yaml"
            merged_path.write_text(merged)
            buf = io.StringIO()
            with redirect_stdout(buf):
                rc = adopt.main(["parameter-keys", "--template", str(merged_path)])
            keys = buf.getvalue().split()
            if rc != 0 or "CertificateArn" not in keys or "ImageUri" not in keys:
                fail(
                    f"parameter-keys CLI missed import Parameters: rc={rc} "
                    f"out={buf.getvalue()!r}"
                )
            else:
                print("  ok  parameter-keys CLI lists CertificateArn on the import template")

        adopt_src = adopt_path.read_text()
        if "custom domain is mapped to ConnectApi" not in adopt_src:
            fail(
                "adopt_demo_domain.py no longer refuses a custom-domain "
                "mapping on ConnectApi — that would collide issuers"
            )
        else:
            print("  ok  import refuses a ConnectApi mapping on the custom domain")

        if adopt.live_template_from_get_template({"TemplateBody": "Foo:\n"}) != "Foo:\n":
            fail("extract-live changed a TemplateBody that already ends in newline")
        elif adopt.live_template_from_get_template({"TemplateBody": "Foo:"}) != "Foo:\n":
            fail("extract-live dropped the trailing-newline append")
        elif adopt.live_template_from_get_template(
            {"TemplateBody": {"Resources": {"X": 1}}}
        ) != json.dumps({"Resources": {"X": 1}}, indent=2):
            fail("extract-live JSON TemplateBody is not dumps(indent=2)")
        else:
            print("  ok  extract-live unwraps get-template JSON")

        try:
            adopt.import_resources(
                {"Items": [{"ApiId": "connect", "ApiMappingId": "m1"}]},
                api_id="demo",
                connect_id="connect",
                domain="api.ratio.marsh.build",
            )
            fail("import-resources accepted a ConnectApi mapping")
        except SystemExit as exc:
            if "custom domain is mapped to ConnectApi" not in str(exc):
                fail(f"import-resources Connect refuse drifted: {exc}")
            else:
                print("  ok  import-resources refuses a ConnectApi mapping")

        try:
            adopt.import_resources(
                {
                    "Items": [
                        {
                            "ApiId": "demo",
                            "ApiMappingKey": "v1",
                            "ApiMappingId": "m1",
                        }
                    ]
                },
                api_id="demo",
                connect_id="connect",
                domain="api.ratio.marsh.build",
            )
            fail("import-resources accepted a keyed mapping as the empty-key Demo mapping")
        except SystemExit:
            print("  ok  import-resources requires an empty-key Demo mapping")

        got = adopt.import_resources(
            {
                "Items": [
                    {"ApiId": "demo", "ApiMappingId": "map-1"},
                    {"ApiId": "other", "ApiMappingId": "map-2"},
                ]
            },
            api_id="demo",
            connect_id="connect",
            domain="api.ratio.marsh.build",
        )
        if (
            len(got) != 2
            or got[0]["LogicalResourceId"] != "DemoDomain"
            or got[1]["LogicalResourceId"] != "DemoApiMapping"
            or got[1]["ResourceIdentifier"]["ApiMappingId"] != "map-1"
            or got[0]["ResourceIdentifier"]["DomainName"] != "api.ratio.marsh.build"
        ):
            fail("import-resources JSON drifted from DemoDomain / DemoApiMapping")
        else:
            print("  ok  import-resources names DemoDomain / empty-key DemoApiMapping")

        if adopt.live_owns_imported_domain(app) is not True:
            fail("app.yaml should already own DemoDomain / DemoApiMapping")
        elif adopt.live_owns_imported_domain(fake_live) is not False:
            fail("stripped live template must not look owned")
        elif adopt.live_owns_imported_domain('{"Resources": {}}\n') is not False:
            fail("empty JSON Resources must not look owned")
        elif adopt.live_owns_imported_domain(
            json.dumps(
                {
                    "Resources": {
                        "DemoDomain": {"Type": "AWS::ApiGatewayV2::DomainName"},
                        "DemoApiMapping": {"Type": "AWS::ApiGatewayV2::ApiMapping"},
                    }
                }
            )
        ) is not True:
            fail("JSON live template with both logical ids should be owned")
        else:
            print("  ok  owns is true only when get-template has DemoDomain and DemoApiMapping")

        try:
            adopt.live_owns_imported_domain(
                json.dumps(
                    {
                        "Resources": {
                            "DemoDomain": {"Type": "AWS::ApiGatewayV2::DomainName"},
                        }
                    }
                )
            )
            fail("owns accepted DemoDomain without DemoApiMapping")
        except SystemExit as exc:
            if "only one of DemoDomain / DemoApiMapping" not in str(exc):
                fail(f"owns partial-ownership refuse drifted: {exc}")
            else:
                print("  ok  owns refuses a half-imported live template")

        with tempfile.TemporaryDirectory() as td:
            absent_path = pathlib.Path(td) / "absent.yaml"
            owned_path = pathlib.Path(td) / "owned.yaml"
            absent_path.write_text(fake_live)
            owned_path.write_text(app)
            buf = io.StringIO()
            with redirect_stdout(buf):
                rc = adopt.main(["owns", "--live", str(absent_path)])
            if rc != 0 or buf.getvalue().strip() != "absent":
                fail(
                    f"owns CLI on a template without DemoDomain: rc={rc} "
                    f"out={buf.getvalue()!r}"
                )
            buf = io.StringIO()
            with redirect_stdout(buf):
                rc = adopt.main(["owns", "--live", str(owned_path)])
            if rc != 0 or buf.getvalue().strip() != "owned":
                fail(
                    f"owns CLI on app.yaml: rc={rc} out={buf.getvalue()!r}"
                )
            else:
                print("  ok  owns CLI prints absent / owned and exits 0")

        if adopt.inject_yaml(app, app) != app:
            fail(
                "inject_yaml rewrote a live template that already owns "
                "DemoDomain / DemoApiMapping — that would fight a later update"
            )
        else:
            print("  ok  inject_yaml is a no-op when the live template already owns both")

    if failures:
        print(f"\n{len(failures)} problem(s): the app stack and the deploy role disagree "
              "about what may be created", file=sys.stderr)
        sys.exit(1)
    print(f"  ok  {len(types)} resource types, all creatable by the role that deploys them")


if __name__ == "__main__":
    main(*sys.argv[1:4])
