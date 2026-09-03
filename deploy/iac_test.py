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
    app_code = "\n".join(
        line for line in app.splitlines()
        if not line.lstrip().startswith("#")
    )
    if re.search(r'Issuer:\s+"https://api\.workos\.com', app_code):
        fail(
            f"{app_path} sets the JWT authorizer issuer to the bare "
            "https://api.workos.com host, which has no OIDC discovery — "
            "CloudFormation will UPDATE_FAILED at Authorizer"
        )
    else:
        print("  ok  Authorizer issuer is not the bare api.workos.com host")

    if "https://auth.ratio.marsh.build" not in app_code:
        fail(
            f"{app_path} does not name the production AuthKit issuer "
            "https://auth.ratio.marsh.build"
        )
    else:
        print("  ok  the app stack names the production AuthKit issuer")

    if re.search(r'WorkOsIssuer="https://api\.workos\.com', flow):
        fail(
            f"{workflow_path} passes the bare api.workos.com host as "
            "WorkOsIssuer — CloudFormation will refuse the authorizer"
        )
    else:
        print("  ok  the workflow does not pass the bare api.workos.com issuer")

    if 'WorkOsIssuer="${ISSUER}"' not in flow:
        fail(
            f"{workflow_path} does not pass WorkOsIssuer from the resolved "
            "issuer — the authorizer and /authconfig.json would then depend "
            "on a template default the smoke test cannot see arrive"
        )
    else:
        print("  ok  WorkOsIssuer is passed from the resolved issuer")

    # ⛔ THE JOURNAL GRANT MUST LIVE IN THE APP STACK, NOT ONLY IN BOOTSTRAP.
    # Issue #129: Sid TheJournal was added to bootstrap.yaml in #84, and
    # README said "re-run bootstrap once". Nobody did. After #126 bound
    # before hydrate, /healthz lived and /balance.json died on
    # s3:PutObject AccessDenied for journals/book/journal/0000…1.
    # A grant only in the hand-applied stack is a grant the next deploy
    # cannot apply. The app stack must allow the execution role to
    # PutObject under journals/ on the same bucket RATIO_JOURNAL_BUCKET names.
    #
    # ⚠ COMMENT-STRIPPED. A sentence describing the grant must not satisfy
    # this — same shape as the CAPABILITY_NAMED_IAM check above.
    if "RATIO_JOURNAL_BUCKET: !Ref ScaleBucket" not in app_code:
        fail(
            f"{app_path} does not set RATIO_JOURNAL_BUCKET to ScaleBucket — "
            "hydrate would write a bucket the IAM grant does not name"
        )
    else:
        print("  ok  RATIO_JOURNAL_BUCKET is the scale bucket")

    prefix_m = re.search(r"RATIO_JOURNAL_PREFIX:\s+(\S+)", app_code)
    if prefix_m is None:
        fail(f"{app_path} does not set RATIO_JOURNAL_PREFIX")
        journal_prefix = None
    else:
        journal_prefix = prefix_m.group(1)
        print(f"  ok  RATIO_JOURNAL_PREFIX is {journal_prefix}")

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
    if "v1/funds" not in flow or "401" not in flow:
        fail(
            f"{workflow_path} no longer asserts unauthenticated /v1/funds is 401 — "
            "the journal grant must not be bought by opening the tenant boundary"
        )
    else:
        print("  ok  smoke still asserts unauthenticated /v1/funds is 401")

    if failures:
        print(f"\n{len(failures)} problem(s): the app stack and the deploy role disagree "
              "about what may be created", file=sys.stderr)
        sys.exit(1)
    print(f"  ok  {len(types)} resource types, all creatable by the role that deploys them")


if __name__ == "__main__":
    main(*sys.argv[1:4])
