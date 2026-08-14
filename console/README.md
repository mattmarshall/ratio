# console — the operations console

A Next.js application. It reads `ratio.console.v1.Console` and gives every
resource on it a URL.

```bash
bazel run //crates/ratio -- watch --book <dir>   # the API on :7373
pnpm install && pnpm dev                          # the console on :3000
```

⭐ **A local run needs no Cognito, no secret and no network.** `ratio watch` sets
none of the `RATIO_COGNITO_*` variables, so `/authconfig.json` answers with empty
strings, this app skips its sign-in gate, and the server answers as
`Subject::Local` — unrestricted, and not a tenant. That is the same test the old
console's `authConfigured()` made, kept because it is what makes the thing
runnable on a laptop.

## Why this exists rather than the page that was in the binary

The console used to be one HTML document — React bundled by esbuild, inlined
with its stylesheet, embedded in the `ratio` binary as a `&str` at compile time.
It worked, and it cost three things:

- **A figure could not be cited.** Every screen was a `useState` in an 1801-line
  file, so the whole console was one URL. An operator who found the 2,000.00
  break on `harbourline-global-value` could describe it and could not send
  anybody to it. Ratio's argument is that a number can be *checked* — that it
  names the journal prefix it was folded from and the configuration it ran
  under. A console with no citations argues against the product.
- **The surface could not grow in parallel.** Every new screen was an edit to the
  same file and the same four-way ternary.
- **A CSS change was a backend deploy.** Rebuild `//crates/ratio`, push an image
  to ECR by digest, roll a CloudFormation stack.

## The shape

```
src/wire/types.ts    the wire types, hand-written to mirror console.proto
src/wire/client.ts   ⛔ server-only. One typed fn per google.api.http rule
src/lib/session.ts   the httpOnly cookie the id token lives in
src/lib/oidc.ts      authorization-code + PKCE, run on the SERVER
src/routes.ts        every screen, and which RPCs it reads
src/app/             the screens
src/components/      what more than one screen renders — including `Ticket`
fixtures/            one captured response per RPC, for the render suite
scripts/             the checks Bazel runs, and the fixture capture
```

### The screens that write

There are five routes over four writes — `/record`, `/ingest`, `/mark`, and
`/trade`, which is `ApplyEvent` again in the terms a trade actually happens in.
They were four forms doing the same thing four ways, each a stack of unstyled
`<label>`s none of which used the form styling `globals.css` had been carrying
since the concept. They are now one pattern, `components/Ticket.tsx`, and each
offers both ways through:

- **Guided** — a tree of the steps carrying the answers so far, and one question
  on screen at a time. The tree is what makes a stepper honest: what has been
  answered is legible without clicking back to find out, and a step is reachable
  only when every step before it has an answer.
- **Form** — every field at once, compact, for the fortieth ticket of the day.

⛔ **Two renderings of one state, never two forms.** The screen owns the state
and hands `Ticket` two views of it, so switching mid-ticket keeps every answer.
Two components each holding half of them is how a compact mode comes to silently
drop the field the other one had.

⛔ **And "preview, then post" is enforced rather than printed.** Every one of
these screens said it and not one made it so: you could preview, change a
figure, and commit the changed one with the old preview still on screen
describing what the button was about to do. Each action now returns the inputs
it answers for, and the commit stays shut until they match what the screen
holds.

⚠ **No control of a ticket is inside its `<form>`.** React resets a form after a
`<form action>` submission. A controlled `<input>` is put back by the next
render because its `value` prop is reapplied; a controlled `<select>` is not,
because the prop has not changed, so React writes nothing and the element keeps
the reset. Both of the trade ticket's selects fell back to "Choose…" the moment
a preview returned, while the state behind them still said otherwise. The form
carries hidden inputs off state, and state is the one source of truth.

### The browser never calls AWS

Pages fetch on the server, through `wire/client.ts`, which attaches the bearer
from the session cookie. Three things follow, and each was the point:

- **No CORS on that path.** A server-to-server `fetch` sends no `Origin`, so the
  gateway's `CorsConfiguration` is never consulted. ⛔ The **absence** of
  `authorization` from its `AllowHeaders` is therefore a fence: it makes a
  browser-direct call impossible. Do not add it.
- **An XSS on this origin cannot read the token**, because `document.cookie`
  cannot.
- **A server component can await a figure**, which is the reason to server-render
  at all.

⛔ **The id token, not the access token.** The gateway authorizer accepts either
— an id token's `aud` is the client id, which is what it validates — but only the
id token carries the `email` claim the tenant boundary matches on at
`Console::book_path`. Sending the access token passes the gateway and then shows
every signed-in person an empty fund rail. ⚠ And `RATIO_DEMO_OPEN=1` hides that
completely: an open demo grants any authenticated caller every fund, so the
mistake would only surface the day tenancy is turned on for a real customer.

## The checks

`pnpm check` is `tsc --noEmit`, the render suite, and `next build`. CI runs the
same script — `.github/workflows/console.yml`, a required check, because
`bazel test //...` has no JavaScript toolchain any more.

`console.yml` also runs five checks in `scripts/`. They are plain Python over
source text and need no node, so a developer can run any of them directly:

| | |
|---|---|
| `route_manifest_test` | the console calls exactly the contract's routes, every screen exists, and ⛔ **no RPC goes unread** |
| `fields_test` | the twenty fields and phrases whose absence has already shipped once |
| `fixtures_test` | every fixture is shaped like its message in `console.proto` |
| `tokens_test` | the design tokens still agree with `site/style.css` |
| `no_secrets_test` | nothing that belongs in the environment is committed |

⚠ **These were Bazel `sh_test`s and are not any more.** They went red twice on
Bazel wiring rather than on anything they check, and neither failure could be
reproduced without Bazel — which most environments editing this directory do not
have. ⛔ `console/BUILD.bazel` survives as a single `exports_files`, because
`//proto:mirrors_test` reads `src/wire/types.ts` through it.

### The fixtures are captured, not written

```bash
bazel build //crates/ratio
./deploy/seed-demo-funds.sh bazel-bin/crates/ratio/ratio /tmp/demo-funds
RATIO_FUNDS=/tmp/demo-funds bazel run //crates/ratio -- watch --book /tmp/demo-funds &
scripts/capture_fixtures.sh
```

⛔ A fixture somebody typed is a claim about what the server sends; a fixture the
server sent is what it sends. `fixtures_test` checks the SHAPE of whatever is
there against the proto on every build, so a hand-edit cannot invent a field —
but it cannot check the values, which is why the capture script exists.

⛔ **Negative-test every render case.** Take the field out of the fixture and
watch the case go red before believing it. `CONTRIBUTING.md` records three suites
in this repository that were green, covered the code, and tested nothing.

## Environment

Two required, one optional.

| | |
|---|---|
| `RATIO_API_ORIGIN` | where the API is — **scheme and host, no path**. `http://127.0.0.1:7373` locally |
| `RATIO_SESSION_KEYS` | comma-separated base64 32-byte keys, newest first. A keyring so rotation does not sign everyone out |
| `RATIO_CONSOLE_ORIGIN` | *optional.* This app's own origin, for the OAuth `redirect_uri`. Only consulted when the API publishes none — which means local development |

⛔ **The console origin comes from the API, not from here.** `deploy/app.yaml`
builds the Cognito app client's callback URL and the API's `RATIO_CONSOLE_URL`
out of one `ConsoleOrigin` parameter, and the API publishes it at
`/authconfig.json` as `consoleOrigin`. That is the byte-identical string Cognito
compares the `redirect_uri` against, so taking it from there means it cannot
disagree. It once did: Vercel held `https://ratio-console.vercel.app` while
Cognito had `https://ratio-ims.vercel.app`, and every sign-in failed. The
variable survives as an override for `next dev`, where a local `ratio watch`
publishes an empty origin because it has no console to point at.

⛔ **Still never the `Host` header.** Whoever chooses this value chooses where an
authorization code is delivered. `/authconfig.json` is the same TLS document
already trusted for `issuer`, `clientId` and `domain` — a forged `domain` sends
somebody to an attacker's hosted UI, which is worse than a forged redirect —
so reading one more field of it adds no trust the console did not already extend.
Deriving it from the request would have added one.

⚠ **A wrong value fails the build, not the sign-in.** `pnpm build` runs
`scripts/preflight.mjs` first: it fetches `${RATIO_API_ORIGIN}/authconfig.json`
and exits non-zero on a 404 or a wrong shape, and checks the session keyring's
length without printing it. A 5xx or an unreachable host only warns — transient
is not misconfigured. **With `RATIO_API_ORIGIN` unset it skips everything**,
which is what keeps CI hermetic and `next dev` offline.

⛔ **No refresh token in the cookie.** A Cognito refresh token is good for thirty
days and a cookie is a bearer; id + refresh sealed also exceeds the 4096-byte
cookie limit, which a federated Google token would find first. The session is
about an hour and then you sign in again — which is exactly what the old console
did.

⚠ **Cognito accepts no wildcards in callback URLs**, so a Vercel preview on its
own generated hostname cannot sign in. Previews render from `fixtures/`. If live
preview data is ever needed, the pattern is a bounce through a registered origin
carrying the preview host in the OAuth `state`, **with a server-side allowlist on
the way back** — without one that is an open redirect on a route carrying tokens.
