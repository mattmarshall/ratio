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

    if failures:
        print(f"\n{len(failures)} problem(s): the app stack and the deploy role disagree "
              "about what may be created", file=sys.stderr)
        sys.exit(1)
    print(f"  ok  {len(types)} resource types, all creatable by the role that deploys them")


if __name__ == "__main__":
    main(*sys.argv[1:4])
