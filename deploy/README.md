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
  WorkOS access token. So `CorsConfiguration` is not consulted for
  console traffic at all, and the **absence** of `authorization` from
  `AllowHeaders` is now a fence rather than an oversight — it is what makes a
  browser-direct call impossible. Do not add it.
- **AuthKit's redirect URI is on the console's origin.** The path is the one
  AuthKit-for-Next.js documents (`handleAuth()` at `/app/callback/route.ts`),
  not Cognito's `/api/auth/callback`. See [WorkOS AuthKit](#workos-authkit--project-agnostic)
  for the exact URIs to register. Cognito resources remain in the template
  unused so a stack update does not destroy the live pool.

Set `ConsoleOrigin` through the **`CONSOLE_ORIGIN` repository variable** (a
hostname is not a secret, so a variable rather than a secret — same reasoning as
`DEMO_MEMBERS`). Production is **`https://ratio.marsh.build`**. The retired
`https://ratio-ims.vercel.app` host still resolves; `deploy.yml` refuses it
rather than shipping it as CORS/redirect configuration. Leave the variable
unset and CI fails with that name — locally, an empty parameter still means
the three public screens, the API and MCP all serve, and `/` says what it
serves instead of redirecting.

⭐ **One parameter, three consumers, no second copy.** `ConsoleOrigin` becomes
the Cognito `CallbackURLs` entry, the `RATIO_CONSOLE_URL` that makes `/` and
`/app` redirect, **and** the `consoleOrigin` field in `/authconfig.json` that the
console reads its OAuth `redirect_uri` back out of. The console does not hold its
own copy, because it did once and the two disagreed — Vercel had
`https://ratio-console.vercel.app` while Cognito had `https://ratio-ims.vercel.app`,
and nothing compared them until somebody clicked Sign in. The deploy smoke test
now asserts the published field equals `CONSOLE_ORIGIN`, so a typo here goes red
in this workflow.

### What Vercel needs

Five variables, in **Production and Preview** (Settings → Environment Variables).
None of them is a WorkOS client id written in this repository.

| | |
|---|---|
| `RATIO_API_ORIGIN` | this stack's `DemoUrl`, **scheme and host, no path and no trailing slash** |
| `WORKOS_CLIENT_ID` | Ratio Production: `client_01M1JJZTFXFDZJ0XJM1NPNSEJB` |
| `WORKOS_API_KEY` | the same application's API key |
| `WORKOS_COOKIE_PASSWORD` | ≥32 characters; `openssl rand -base64 32` |
| `NEXT_PUBLIC_WORKOS_REDIRECT_URI` | this origin's AuthKit callback, e.g. `https://ratio.marsh.build/callback` |

⛔ **Generate the cookie password yourself and paste it nowhere else.** A value
that has been in a chat log or a plan file is a disclosed key.
`console/scripts/preflight.mjs` checks its length and never prints it.

⚠ `RATIO_CONSOLE_ORIGIN` is **not** in that table on purpose — it is a local
development override and setting it on a deployment only creates the drift
described above. The preflight warns when it disagrees with the published value.

⚠ **Vercel reads these at build time**, so editing one changes nothing until you
redeploy. A wrong `RATIO_API_ORIGIN` now fails that build rather than shipping a
console nobody can sign into.

⛔ **VERCEL AUTHENTICATION HAS TO BE OFF ON THE CONSOLE PROJECT, AND IT IS ON BY
DEFAULT.** The team default is `ssoProtection: all_except_custom_domains`, so
every `*.vercel.app` URL sits behind Vercel's own login. That breaks this twice
over: nobody outside the Vercel team can reach the console, and Vercel
intercepts the return from AuthKit before `/callback` ever sees the `?code=` —
which reads as a broken sign-in rather than as a protection setting.
Vercel dashboard → the project → Settings → Deployment Protection → Vercel
Authentication → Disabled. That is correct here: the console is *meant* to be a
public sign-in page, WorkOS is the real boundary, and `RATIO_AUTH=required`
makes this API fail closed regardless. A custom domain would be exempt, which is
the other way out once #25 lands.

⚠ **Preview hostnames are not registered.** AuthKit will accept a wildcard
redirect URI, but this console's documented URIs are the two origins below.
Previews render from `console/fixtures/`. `preflight.mjs` requires the
WorkOS variables on `VERCEL_ENV=production` only; a preview with none of
them set is a fixtures build, not a broken sign-in. A preview with some of
them set and not others still fails — that is a half-configured IdP.

## WorkOS AuthKit — Ratio project

Team **Marsh**, project **Ratio** (`project_01M1JJZSSEMWSHNNQX151D19GB`).
Do not use a client id from any other product.

AuthKit reads `WORKOS_CLIENT_ID`, `WORKOS_API_KEY`, `WORKOS_COOKIE_PASSWORD`
and `NEXT_PUBLIC_WORKOS_REDIRECT_URI`. The running code never hardcodes a
client id. Paths match the [authkit-nextjs README](https://github.com/workos/authkit-nextjs):
`handleAuth()` at `/app/callback/route.ts`, Sign-in URL at
`/app/sign-in/route.ts`. Already registered on this project.

| | Staging (sandbox, local) | Production (Vercel + API Gateway) |
|---|---|---|
| environment | `environment_01M1JJZSSNCSMVHWV7RXVPE2SY` | `environment_01M1JJZTBCFS9K7QEJ1ADFMEZW` |
| AuthKit app | `app_01M1JJZTB38S4WXDV3YTXT23Q8` | `app_01M1JJZTN0B6JK2GZR5AJP0YAA` |
| `WORKOS_CLIENT_ID` | `client_01M1JJZT4T0NN1WWT65NE6CV3W` | `client_01M1JJZTFXFDZJ0XJM1NPNSEJB` |
| Redirect URI | `http://localhost:3000/callback` (default), `https://ratio.marsh.build/callback` | `https://ratio.marsh.build/callback` |
| Sign-in URL | `http://localhost:3000/sign-in` | `https://ratio.marsh.build/sign-in` |
| Sign-out URI | `http://localhost:3000` (default), `https://ratio.marsh.build` | `https://ratio.marsh.build` |
| CORS | both origins above | `https://ratio.marsh.build` |

### What to set

**Local** (`console/.env.example` → `.env.local`), only if you want a live
IdP. Unset means `Subject::Local` against `ratio watch`.

- `WORKOS_CLIENT_ID=client_01M1JJZT4T0NN1WWT65NE6CV3W`
- `WORKOS_API_KEY` — staging secret from the Ratio environment; never committed
- `WORKOS_COOKIE_PASSWORD` — ≥32 characters; `openssl rand -base64 32`
- `NEXT_PUBLIC_WORKOS_REDIRECT_URI=http://localhost:3000/callback`

**Vercel** (Production and Preview), console project:

- `RATIO_API_ORIGIN` — the stack's `DemoUrl`
- `WORKOS_CLIENT_ID=client_01M1JJZTFXFDZJ0XJM1NPNSEJB` (Production) or the
  staging id above (Preview, if you want sandbox users)
- `WORKOS_API_KEY` — matching environment secret
- `WORKOS_COOKIE_PASSWORD` — ≥32 characters
- `NEXT_PUBLIC_WORKOS_REDIRECT_URI=https://ratio.marsh.build/callback`

**API Gateway** audience is `WorkOsClientId` on the stack. GitHub variable
`WORKOS_CLIENT_ID` is required — there is no template default and no
`${VAR:-client_…}` fallback in the workflow. A missing value fails the
Deploy step by name rather than silently becoming a production identifier.
It is not a secret. The Production id to put in the variable is
`client_01M1JJZTFXFDZJ0XJM1NPNSEJB`.

**API Gateway** issuer is `WorkOsIssuer`. Default and workflow fallback are
`https://auth.ratio.marsh.build` (the production AuthKit custom domain).
That host serves `/.well-known/openid-configuration`; the bare
`https://api.workos.com/` host does not, and CloudFormation refuses it.
Optional repository variable `WORKOS_ISSUER` overrides the default; the
workflow rejects the bare WorkOS API host by name.

`WORKOS_API_KEY` is a secret (`sk_…`). Do not commit it. `/login` and
`/api/auth/login` are the same initiate-login handler as `/sign-in`.
`/api/auth/callback` only redirects to `/signin` (the prompt page).

Local `next dev` and `ratio watch` leave every `WORKOS_*` variable unset.
That is `Subject::Local`, not a second IdP.

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

## The report email (SES) — one-time setup

The demo's follow-up email — a lead's run report, with the permalink to every
figure — sends from the identity in the `ScaleSender` stack parameter, currently
`demo@ratio.msoftware.co`. Until SES is set up, leave `SCALE_SENDER` unset:
**the demo is whole without it** — no email is attempted and the report link is
on the page the moment a run completes.

To turn it on, once, by hand:

1. **Verify the domain.** SES console (us-east-1) → Verified identities →
   Create identity → Domain → `ratio.msoftware.co`. Add the three DKIM CNAME
   records it prints to the domain's DNS. Wait for "Verified".
2. **Leave the sandbox.** SES starts sandboxed: it will only send TO verified
   addresses, which for a lead-capture demo is exactly backwards. SES console →
   Account dashboard → Request production access; the form asks what you send
   and how you handle bounces (transactional, one report per demo run, capped
   at 200 per run in code). Usually approved inside a day.
3. **Re-run bootstrap** (the function's replay-email grant is scoped to
   `*@ratio.msoftware.co` — change both if the domain ever changes):

       aws --profile ratio cloudformation deploy \
         --template-file deploy/bootstrap.yaml \
         --stack-name ratio-demo-bootstrap \
         --capabilities CAPABILITY_NAMED_IAM \
         --parameter-overrides GitHubRepo=mattmarshall/ratio

4. **Set the sender** as a repository variable: `SCALE_SENDER=demo@ratio.msoftware.co`.
   The next deploy passes it through; email activates with no code change.

⚠ **Order matters:** set `SCALE_SENDER` only after the identity verifies —
before that, every send fails quietly in the task log while the demo carries on.

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

That seeded book is a **fund**. Kind-selected chrome is a different
walk-through: CreateBook(Personal) lands on the sheet, with a period P&L and
household budget vs actual (`[personal] budget` on the configuration that
book pins, then `/books/{id}/views/{view}/budget` for a month or a year —
unset shows as unset, not a fake zero). CreateBook(Project) lands on the
same `/budget` URL as a cumulative project roll-up — original contract,
approved change orders, revised, awarded committed cost keyed to work
packages, and remaining to spend (revised − incurred − awarded) — and
`/billing` as billed vs earned, retainage, cost by phase, remaining to
bill (revised − billed), and collections vs billed (cash against AR).
A change-order walk-through can record `approve_co_site` / `deduct_co_site`
or ingest `change-orders`; a remaining-to-bill / collections walk-through
can cite those leftovers on `/billing` after `progress_bill` /
`collect_receivable`; a committed-cost walk-through can record
`award_commitment_site` / `release_commitment_site` or ingest
`purchase-orders`. `/budget` does not forecast — EAC and cost to complete
are refused rather than implied by remaining to spend. None of those
walk-throughs can show AIA G702 product UI, a client portal, e-signature,
or CRM. Unset original / unposted COs / unawarded remaining / unbilled
remaining / uncollected cash stay unset, not a fake zero. The live demo
does not seed those baselines or any commitment postings.
CreateBook(Investment)
lands on capital activity (funded partners plus commitment / undrawn —
unset until a commitment posts, not a callable zero), then the fund ABOR
warehouse. A capital-call walk-through can record `commit_lp` / `call_lp`
or ingest `capital-calls`; a capital-account walk-through can cite each
partner's beginning, contributions, distributions, and ending on the
same `/capital` page. Allocated income / expense / unrealized stay
unset — not an equal share of book NAV. Neither can show IRR, a
waterfall, a future call schedule, management-fee billing, an LP
portal, or K-1 packaging. The live demo does not seed those baselines or any
commitment postings.

`entrypoint.sh` copies the seeded chart and config to `/tmp` at start, because a
Lambda filesystem is read-only elsewhere. **The journal is not that copy.** When
`RATIO_JOURNAL_BUCKET` is set (the app stack sets it to the scale bucket, prefix
`journals/`), append is one object per entry with a conditional PUT — `tla/S3Journal.tla`,
issue #24. A posted entry survives the next cold start, and two containers
share one log rather than silently forking under one URL. Unset, `/tmp` is
still the system of record, which is the local `ratio watch` shape.

⭐ **The write grant lives on the bucket, because that is what CI can apply.**
`install_journal_store` / hydrate / append are a conditional `s3:PutObject`
(`If-None-Match:*`) plus `GetObject`/`ListBucket` under `journals/` on the
scale bucket. Sid `TheJournal` on `ratio-demo-execution` in `bootstrap.yaml`
already names that — and bootstrap is applied by hand. #84 added the Sid;
nobody re-ran the stack; after #126 bound before hydrate, `/version` lived
and `/balance.json` died on `AccessDenied` for
`journals/book/journal/00000000000000000001` (#129). The app stack now
carries a bucket policy for the same role, same prefix, no `DeleteObject`
(a delete would be a truncation wearing an IAM grant). Same-account S3
accepts a resource-based Allow even when the identity policy has not
caught up. Re-running bootstrap still attaches the identity grant; it is
no longer the thing that unblocks smoke.

Two assertions guard the seed, because a missing book is invisible: the image build
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
  Gateway JWT authorizer, issuer `WorkOsIssuer` (production default
  `https://auth.ratio.marsh.build`, the AuthKit custom domain that serves
  `/.well-known/openid-configuration`), audience = `WorkOsClientId` (the
  same value as `WORKOS_CLIENT_ID`, from the stack parameter, never a
  literal in this repository). The bare `https://api.workos.com/` host is
  not an OIDC issuer — CloudFormation refuses it — and must not be
  passed. The server does no crypto.
  The authorizer puts the verified claims on the request context, which
  the Lambda Web Adapter forwards as `x-amzn-request-context` — a header the
  gateway synthesizes, so a client cannot forge its own claims. The Cognito
  pool remains in the template unused so a stack update does not destroy it.
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

⭐ **The console sends the WorkOS *access* token as the API bearer.** AuthKit's
`withAuth()` hands that token over; the gateway authorizer's audience is the
WorkOS client id. Membership matches on `sub`, email if present, or
`org:{workos_org_id}`. A Cognito id token is not consulted.

⚠ The leftover Cognito user-creation and Google-provider steps below apply
only to the unused pool. Google and email sign-in for the live console are
AuthKit's, configured on the attached WorkOS application.

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
confirm the fund rail shows the seven seeded funds and the principal chip shows
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

⚠ **And re-run it again after the scale runner.** The deploy role gained
`ecs`, `ec2`, `s3`, `logs` and a narrowly-scoped `iam` so it can stand up the
one-shot Fargate task that folds a twenty-million-lot book, and the function's
execution role gained `ecs:RunTask` on one task family plus the run prefix in
one bucket. ⛔ **Bootstrap first, then push** — the app stack now creates a VPC,
a cluster, two named roles and a bucket, and CI cannot create any of them until
this has been applied. The failure if you push first is `AccessDenied` in
`aws cloudformation deploy`, after a full Bazel build, the whole test suite, a
docker build and an ECR push. `//deploy:iac_test` checks the two templates agree
about what may be created, but it cannot check that the account has caught up —
only running this can.

⛔ **AND ONCE MORE AFTER THAT, IF YOU APPLIED IT BEFORE 2026-08-14.** The first
version of the grant enumerated five `ec2:Describe*` actions — the ones matching
resources the template creates — and the deploy died on a sixth:

    ScaleSubnet CREATE_FAILED  AccessDenied. User doesn't have permission
    to call ec2:DescribeAvailabilityZones

`!GetAZs ""` needs it, and nothing about a subnet's resource type says so. The
read-only actions are wildcards now (`ec2:Describe*`, `s3:Get*`), which narrows
nothing that was ever narrow — those calls take no resource ARN — and removes a
whole class of deploy that fails after the image is already pushed.
`//deploy:iac_test` now fails on the exact grant that shipped.

⛔ **And a third time, for the ECS service-linked role.** The first real button
press failed with the IAM simulator saying `ecs:RunTask` was ALLOWED — because
the role ECS itself assumes, `AWSServiceRoleForECS`, had never been created in
this account. AWS creates it on first cluster creation only when the creating
principal may `iam:CreateServiceLinkedRole`, which the deploy role deliberately
may not. Bootstrap now declares it (`AWS::IAM::ServiceLinkedRole`), and
`//deploy:iac_test` refuses an app stack that runs ECS tasks without it.

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
