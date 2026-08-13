# Deploying the demo

The three screens, the console's API and the MCP endpoint, running on AWS as a
Lambda behind an HTTP API. Live at the `DemoUrl` output of the `ratio-demo-app`
stack.

```
  /               302 → the console (RATIO_CONSOLE_URL); 404 with a sentence if unset
  /app            the same, for the old console's bookmarks and OAuth callback
  /balance        trial balance, with drill-down
  /breaks         break report
  /rules          rules and their checks
  /chat           set up the books — a model driving the MCP tools
  /v1/**          the console's API, JWT-authorized
  POST /mcp       the MCP tools — the same six as `ratio mcp`, same fence
  POST /chat.json one exchange with the model
```

## ⛔ The console is not served from here any more

It was compiled into the binary — one HTML document with the React bundle and
the stylesheet inlined, embedded as a `&str`. It is a **Next.js application in
`console/`, deployed to Vercel**, and this stack only points at it.

Two things about that are load-bearing here:

- **The browser never calls this API.** The console's server does, attaching the
  id token from an httpOnly cookie. So `CorsConfiguration` is not consulted for
  console traffic at all, and the **absence** of `authorization` from
  `AllowHeaders` is now a fence rather than an oversight — it is what makes a
  browser-direct call impossible. Do not add it.
- **The Cognito callback moved to the console's origin.** `CallbackURLs` is
  `${ConsoleOrigin}/api/auth/callback` plus `http://localhost:3000/...`, and
  `/authconfig.json` advertises `"redirectPath":"/api/auth/callback"`. Those two
  must agree or every sign-in is refused by the IdP.

Set `ConsoleOrigin` through the **`CONSOLE_ORIGIN` repository variable** (a
hostname is not a secret, so a variable rather than a secret — same reasoning as
`DEMO_MEMBERS`). Leave it unset and the demo still works: the three public
screens, the API and MCP all serve, and `/` says what it serves instead of
redirecting.

⛔ **Cognito accepts no wildcards in callback URLs.** A Vercel preview
deployment on its own generated hostname cannot sign in. That is deliberate —
previews render from `console/fixtures/`. If live preview data is ever needed,
the pattern is a bounce through a registered origin carrying the preview host in
the OAuth `state`, **with a server-side allowlist on the way back**; without one
that is an open redirect on a route that carries tokens.

## ⛔ The chat screen needs a one-time console action

Bedrock requires an **Anthropic use-case form** per account before any Claude
model can be invoked. Until it is submitted the chat screen says so plainly and
everything else keeps working — the tools, the books and the fence are all
local and need no model.

Submit it at **Bedrock → Model access** in account 320473299741. There is a
`bedrock:PutUseCaseForModelAccess` API, but its payload is a compliance
attestation about intended use filed on behalf of the company, so it is left to
a person.

⚠ **A working Bedrock call is not evidence that the form was submitted.** This
account answered a tool-use call correctly and refused the identical call forty
minutes later — new accounts appear to get a grace window. Test after the form,
not before.

## Where it runs

Account **320473299741** (`ratio`), in the `Platform` OU of the marsh
organization. Region `us-east-1`.

| stack | what | deployed by |
|---|---|---|
| `ratio-demo-bootstrap` | ECR repository, GitHub OIDC provider, deploy role, execution role, budget | a human, once |
| `ratio-demo-app` | the function, the HTTP API, the log group, the Cognito pool + JWT authorizer | CI, on every push |

⛔ **Anything about the ACCOUNT rather than the demo lives in
[`mattmarshall/cloud-org`](https://github.com/mattmarshall/cloud-org)**, not
here: Identity Center assignments, the budget, and the cost-category bucket.
That repository is payer-level IaC for the whole organization, and its account
inventory (`aws/org/accounts.bzl`) is the one place account ids are written
down. Sign-in access is generated from that list by construction, which is what
makes a missing assignment a diff rather than a silence.

This directory owns only what is specific to running the demo.

## Signing in

`aws sso login --sso-session marsh`, then the account is reachable as the
`ratio` and `ratio-ro` profiles. Access comes from the `platform-admins` group —
`AdministratorAccess` and `ReadOnlyAccess`, kept separate so reading the account
does not require a role that can delete it. Add or remove people by changing
group membership, not by changing a template.

## What it costs

Close to nothing, and that is a design constraint rather than a happy accident.

- **Lambda** — billed per request and per GB-second. Nothing at rest.
- **HTTP API** — $1.00 per million requests. No hourly charge. (An ALB would
  have been ~$16/month before a single request; a Function URL would have been
  free, but see below.)
- **ECR** — three images kept, untagged expired after a day.
- **CloudWatch Logs** — 7-day retention, set explicitly. Lambda's default is
  *never expire*, which is the quietest recurring cost in AWS.
- **Budget** — $5/month at the payer, alerting at 50% actual and 100% forecast.
  Declared in cloud-org, not here.

The account's own Lambda concurrency limit of 10 is the cap on runaway
invocations. `ReservedConcurrentExecutions` is deliberately unset: AWS refuses
any reservation that would leave fewer than 10 unreserved, so on this account
one cannot be created — and the account limit is the better bound anyway, since
no template edit can widen it.

## Why an HTTP API and not a Function URL

A public Function URL would be simpler and free. It does not work in this
account: it returns 403 for any principal outside the account, before the
function is ever invoked. That was established rather than assumed —

- a SigV4 request signed with account credentials returns `200`;
- a throwaway control function created beside it is refused identically;
- no SCP or RCP is involved, and there is no Lambda public-access-block API;
- CloudFront in front with an OAC-signed request is refused too.

So the block is above the account and no template turns it off. An HTTP API
reaches the same function and works. The only capability given up is response
streaming, which nothing here uses.

## The demo book

Generated by `seed-demo-book.sh` on every image build, never committed — a book
checked in as bytes rots silently the first time a format changes, and nobody
reviews a binary diff. It contains a four-rule configuration, a replayed
quarter, one break of `2000.00`, and one proposal nobody has approved (so the
rules screen has both of its columns).

`entrypoint.sh` copies it to `/tmp` at start, because a Lambda filesystem is
read-only elsewhere. That also means **every cold start resets the demo**,
which is what you want in front of a customer.

Two assertions guard it, because a missing book is invisible: the image build
fails if the book lacks its accounts, journal, reports or proposals, and the
deploy's smoke test fails if the live site does not serve six entries, a
pending proposal, and that `2000.00` break. Both exist because an empty book
shipped twice while every other signal stayed green.

## Authentication and tenancy

Every `/v1` route — the console's reads and its four writes — now requires a
verified token. `/app`, `/version`, `/healthz`, `/mcp`, and the public
`/authconfig.json` stay open, because the unauthenticated console has to be able
to load and start the sign-in.

The split of responsibility is the load-bearing decision:

- **Authentication** — "the token is real, unexpired, ours" — is the API
  Gateway JWT authorizer, backed by a Cognito user pool. The server does no
  crypto. The authorizer puts the verified claims on the request context, which
  the Lambda Web Adapter forwards as `x-amzn-request-context` — a header the
  gateway synthesizes, so a client cannot forge its own claims.
- **Authorization** — "this subject may open this fund" — is entirely in Rust,
  at `Console::book_path`, where the test suite can break it. A fund a caller
  may not see is refused with the *same* error as one that does not exist. With
  `RATIO_AUTH=required` the server is fail-closed: a `/v1` request with no
  verified claims is refused even if the authorizer were removed, so a
  misconfigured gateway produces refusal, not open access.

Membership is data, not an IdP group: `funds/MEMBERSHIP.tsv`, lines of
`<subject>\t<fund-id>`, matched against a caller's verified `sub` **or** email.
`entrypoint.sh` writes it on each start from `RATIO_DEMO_MEMBER` and the funds
that actually exist.

### The demo is open (`RATIO_DEMO_OPEN`)

A public demo's audience is not known ahead of time, so it cannot be an
allow-list of emails. `RATIO_DEMO_OPEN=1` (set in `app.yaml`) makes the server
grant **any authenticated caller every fund**, while a write is still signed with
their verified id — so anyone may sign in (Google auto-provisions on first
sign-in) and then everyone sees the demo.

⚠ This is **not** a dropped boundary. Sign-in is still required (the 401 above is
unchanged), and the change is in a separate `Console::open` path, so the tenant
path (`Console::scoped`, matching `MEMBERSHIP.tsv`) and its isolation test are
untouched. Unset `RATIO_DEMO_OPEN` and the demo scopes each caller to the funds
`MEMBERSHIP.tsv` grants them, with no other change — that is the model a real
tenant deployment runs, and the sections below (the invited user, `DEMO_MEMBERS`)
describe it.

### Creating the invited demo user

The pool is **invite-only** (`AllowAdminCreateUserOnly`) — a public sign-up form
on an internet-facing pool is an abuse surface with no upside for a demo with a
known audience. Create the one demo user by hand, with the email that
`RATIO_DEMO_MEMBER` names (default `demo@ratio.fastverk.dev`):

```sh
POOL="$(aws cloudformation describe-stacks --stack-name ratio-demo-app \
  --query 'Stacks[0].Outputs[?OutputKey==`UserPoolId`].OutputValue' --output text)"

aws cognito-idp admin-create-user \
  --user-pool-id "$POOL" \
  --username demo@ratio.fastverk.dev \
  --user-attributes Name=email,Value=demo@ratio.fastverk.dev Name=email_verified,Value=true \
  --desired-delivery-mediums EMAIL
```

Cognito emails a temporary password; the first sign-in forces a reset. To skip
the email (e.g. a shared demo credential), follow with
`admin-set-user-password --permanent`.

⛔ **The email must equal `RATIO_DEMO_MEMBER`.** The tenant boundary matches on
it, so a mismatch signs the user in successfully and then shows an *empty* fund
rail — a green sign-in that looks like a broken demo. If you invite a user under
a different address, override the `DemoMember` parameter to match, or the grant
names a subject who never signs in.

⛔ **The console sends the *id* token as the API bearer, not the access token.**
The gateway authorizer accepts either, but only the id token carries the `email`
claim the tenant boundary matches on — a Cognito access token has `sub` and
`client_id` and no email, which would leave every signed-in person on an empty
rail regardless of MEMBERSHIP. It is `Caller.idToken` in
`console/src/wire/client.ts`, and the token is now held server-side, so a reader
who has both to hand will reach for the access token by reflex.

⚠ **`RATIO_DEMO_OPEN=1` HIDES THAT MISTAKE COMPLETELY.** An open demo grants any
authenticated caller every fund, so sending the wrong token still shows a full
rail. It would surface the day tenancy is turned on for a real customer, which
is the worst possible day to find it.

### Signing in with Google

Google is a native Cognito social provider, wired in `app.yaml` and gated on
`GoogleClientId` being non-empty — so the pool ships email/password-only until
you supply the credentials, then lights up "Continue with Google" on the next
deploy. Four steps:

1. **Create a Google OAuth 2.0 "Web application" client** (Google Cloud console →
   APIs & Services → Credentials). Set:
   - Authorized redirect URI:
     `https://ratio-demo-320473299741.auth.us-east-1.amazoncognito.com/oauth2/idpresponse`
   - Authorized JavaScript origin:
     `https://ratio-demo-320473299741.auth.us-east-1.amazoncognito.com`
2. **Store the credentials as repository secrets** (GitHub → repo Settings →
   Secrets and variables → Actions): `GOOGLE_OAUTH_CLIENT_ID` and
   `GOOGLE_OAUTH_CLIENT_SECRET`. `deploy.yml` reads them as env vars and passes
   them to CloudFormation; they are never committed.
3. **Grant the Google account membership.** Google signs a user in as their real
   email, and the tenant boundary matches on it — so that email must be a demo
   member or the sign-in lands on an empty rail. Set the repository **variable**
   (Settings → Secrets and variables → Actions → Variables) `DEMO_MEMBERS` to a
   comma-separated list, e.g. `demo@ratio.fastverk.dev,you@gmail.com`. `deploy.yml`
   passes it as the `DemoMember` parameter and `entrypoint.sh` grants each member
   every seeded fund. A variable, not a secret — an email is not one — so no
   address is committed. Unset keeps just the email/password demo user.
4. **Grant the deploy role the identity-provider permissions** (once) and
   redeploy. The deploy role needs `cognito-idp:*IdentityProvider*` to create the
   Google provider — the same shape as the pool grant. Either re-run
   `bootstrap.yaml`, or extend the inline policy in CloudShell:

   ```sh
   aws iam put-role-policy --role-name ratio-demo-deploy \
     --policy-name manage-the-demo-user-pool \
     --policy-document "$(aws iam get-role-policy --role-name ratio-demo-deploy \
        --policy-name manage-the-demo-user-pool --query PolicyDocument --output json \
        | python3 -c 'import json,sys; d=json.load(sys.stdin); d["Statement"][0]["Action"] += [
          "cognito-idp:CreateIdentityProvider","cognito-idp:UpdateIdentityProvider",
          "cognito-idp:DeleteIdentityProvider","cognito-idp:DescribeIdentityProvider",
          "cognito-idp:ListIdentityProviders"]; print(json.dumps(d))')"
   ```

   Then trigger a deploy (push, or run the `deploy` workflow) so CloudFormation
   creates the provider.

⚠ **Federation auto-provisions.** A first Google sign-in creates a pool user even
though the pool is invite-only — federation ignores that setting. The tenant
boundary still gates funds, so a Google account not in `DEMO_MEMBERS` signs in
and sees an *empty* rail, never another fund's data.

### The smoke test after auth

CI's smoke test asserts the boundary is *live* — `/v1/funds` without a token
returns `401` — but it cannot assert `/v1` **content** (a held position, the
three fund states, the NAV replay) without a token, and CI holds no user
credential. Those checks moved out of the public smoke test. To exercise the
authenticated path end to end, sign in through the Hosted UI on the live URL and
confirm the fund rail shows the five seeded funds and the principal chip shows
the signed-in email; or script an `initiate-auth` against the pool with a
smoke user's permanent password and replay one `/v1/funds` call with the
returned access token. The public `/balance.json` and `/breaks.json` checks
still prove a real book shipped, so blank-book protection is intact.

## One-time setup

Already done for this account. To recreate it elsewhere:

```sh
aws cloudformation deploy \
  --template-file deploy/bootstrap.yaml \
  --stack-name ratio-demo-bootstrap \
  --capabilities CAPABILITY_NAMED_IAM \
  --parameter-overrides GitHubRepo=mattmarshall/ratio
```

Then update `ACCOUNT_ID` in `.github/workflows/deploy.yml` and push.

⚠ **Re-run bootstrap after adding auth.** The deploy role gained
`cognito-idp:*` on the demo pool (create/update/describe/delete pool, client,
and domain) so CI can manage the authorizer's user pool. An account that
provisioned bootstrap before this round must re-run the command above once, or
the next app deploy fails with an access-denied creating the pool.

## How CI gets in

GitHub's OIDC provider, no long-lived key. The trust policy is scoped to
`repo:mattmarshall/ratio:ref:refs/heads/main` — a pull request, including one
from a fork, cannot assume the role. The deploy role can push to one ECR
repository, manage one function and one API, and pass one execution role.

The function's own role can do nothing but write its logs. It holds no secrets
and reaches no AWS API.

## Gotchas worth keeping

- **The runner and the base image are a matched pair.** `ubuntu-24.04` builds
  the binary; `ubuntu:24.04` runs it. Bazel links against the runner's glibc,
  and a smaller base on an older one (debian:bookworm-slim is 2.36 against the
  runner's 2.39) fails at start with a symbol-version error that reads like a
  corrupt image.
- **Deploys are by digest, not tag**, so the running code and the commit cannot
  drift apart.
- **`PayloadFormatVersion` must be `2.0`.** The Lambda Web Adapter expects it;
  on 1.0 it sees a different event shape and every request 500s.
- **`.dockerignore` is a blacklist on purpose.** The whitelist form (`*` then
  `!name`) shipped an empty demo book twice while everything looked green.
