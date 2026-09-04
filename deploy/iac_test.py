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

import re
import sys
import pathlib

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
    # ⚠ THE PATH UNDER /user_management/{client_id} IS THE REAL ISSUER.
    # AuthKit session tokens mint that iss. A prefix match on
    # https://api.workos.com would fail the correct default, so these
    # patterns end at an optional trailing slash and then the quote.
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
        "https://api.workos.com/user_management/client_01M1JJZTFXFDZJ0XJM1NPNSEJB"
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
            f"{app_path} defaults WorkOsIssuer to the AuthKit custom domain — "
            "session tokens mint iss under api.workos.com/user_management/"
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
        r'Default:\s+"https://api\.workos\.com(/user_management/[^"]*)?"',
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

    # ⛔ THE DEPLOYED DEMO API MUST NOT HYDRATE FROM S3 ON COLD START.
    # Ops cleared RATIO_JOURNAL_BUCKET + RATIO_JOURNAL_PREFIX from live
    # Lambda ratio-demo (account 320473299741, us-east-1) after production
    # /books showed "the journal is still hydrating" (orTransient on API
    # 503). Timeout was already 60. /v1/books then returned 401, not 503.
    # The next CloudFormation deploy would restore those two env vars
    # from this template and the hang. Unset is /tmp journals — the
    # local `ratio watch` shape. Scale still uses ScaleBucket; this
    # check is the Function Environment only. A sentence describing
    # the vars in a comment must not satisfy or fail this — `app_code`
    # is comment-stripped. Same pattern as RATIO_DEMO_OPEN.
    if re.search(r"^\s+RATIO_JOURNAL_BUCKET:", app_code, re.M):
        fail(
            f"{app_path} still sets RATIO_JOURNAL_BUCKET on the function — "
            "a deploy would restore S3 journal hydrate and 503 /v1/books"
        )
    else:
        print("  ok  RATIO_JOURNAL_BUCKET is unset on the deployed function")
    if re.search(r"^\s+RATIO_JOURNAL_PREFIX:", app_code, re.M):
        fail(
            f"{app_path} still sets RATIO_JOURNAL_PREFIX on the function — "
            "a deploy would restore S3 journal hydrate and 503 /v1/books"
        )
    else:
        print("  ok  RATIO_JOURNAL_PREFIX is unset on the deployed function")

    # Scale still needs ScaleBucket (RATIO_SCALE_BUCKET, cluster, task).
    # Unsetting the journal pair must not drop the scale path.
    if not re.search(r"^\s+RATIO_SCALE_BUCKET:\s+!Ref\s+ScaleBucket\s*$", app_code, re.M):
        fail(
            f"{app_path} dropped RATIO_SCALE_BUCKET — stopping API journal "
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
    # cannot apply. The API function no longer names the bucket; the
    # policy stays so a later scale / durable-write path can. Prefix
    # is the scale-bucket journals/ prefix, not a Function env var.
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

    if failures:
        print(f"\n{len(failures)} problem(s): the app stack and the deploy role disagree "
              "about what may be created", file=sys.stderr)
        sys.exit(1)
    print(f"  ok  {len(types)} resource types, all creatable by the role that deploys them")


if __name__ == "__main__":
    main(*sys.argv[1:4])
