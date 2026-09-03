# console — the operations console

A Next.js application. It reads `ratio.console.v1.Console` and gives every
resource on it a URL.

```bash
bazel run //crates/ratio -- watch --book <dir>   # the API on :7373
pnpm install && pnpm dev                          # the console on :3000
```

⭐ **A local run needs no WorkOS, no secret and no network.** `ratio watch` sets
none of the `WORKOS_*` / `RATIO_WORKOS_*` variables, so `/authconfig.json`
answers with empty strings, this app skips its sign-in gate, and the server
answers as `Subject::Local` — unrestricted, and not a tenant.

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
src/lib/screens.ts   kind selects the places under a book — read by the hub AND ⌘K
src/lib/deeplink.ts  a pasted resource name → a URL here, or a refusal
src/app/             the screens
src/components/      what more than one screen renders — including `Ticket`
fixtures/            one captured response per RPC, for the render suite
scripts/             the checks Bazel runs, and the fixture capture
```

### ⌘K

Thirty-nine screens across eight tabs, two books of record and four tickets, and
until now the only ways in were the rail, the tabs and the address bar. The
palette — [kbar](https://github.com/timc1/kbar), the one UI library in this tree
— makes the route tree this console already publishes reachable by typing.

⛔ **It navigates and it never writes.** Every action ends in a `router.push`.
"Preview, then post" is enforced on the screen that owns each write, and an
action that could submit from a command palette would be a second way round that
fence. ⛔ **And it holds no figure**: a palette that accumulated filters or
selections would be the 1801-line `useState` coming back through a door marked
convenience.

⭐ **It costs no upstream call.** `KBarProvider` mounts in `funds/layout.tsx`,
which has already awaited the fund list for the rail; `FundActions` registers the
screens, books of record and tickets from `funds/[fund]/layout.tsx`, which has
already awaited `GetFund` and `ListViews`. Nothing new is fetched, and nothing is
fetched from the browser at all — there is no search RPC in the contract, and
`connect-src 'self'` means the tab could not call the API if there were.

⛔ **So a pasted id offers every route it could name and guesses none.** The id
namespaces collide and it is not close: a break is `cash-usd-2026-02-26`, a NAV
strike is `2026-02-26`, an account is `1010` and so is a lot. Nothing here can
look any of them up, so `deeplink.ts` offers all thirteen under *Open by id* and
shows the URL each would go to. A palette that picked one would be the console
asserting a fact it has not checked. A pasted *resource name* is different — the
string says which resource it is, so that resolves to exactly one URL, through the
one table in this codebase that translates a whole name. ⚠ Eight of its
collections are spelled differently on the two sides (`navStrikes` → `strikes`,
`changeLogEntries` → `changes`, and six more), which is why the table exists and
why `deeplink.test.ts` pins every one.

⚠ **#52 and #53 were the two contract resources with no screen**, and the
palette is where that showed up. Both now have pages: a view lands on
`/books/{book}/views/{view}`, and a journal entry lands on
`/books/{book}/entries/{entry}`. The posting screen's provenance is a link,
not `entry {id}` as plain text.

⚠ **kbar's `KBarAnimator` and `KBarPositioner` are not used**, and that is
deliberate — the animator is invisible without the Web Animations API and builds
an unguarded `ResizeObserver`, and the positioner's type accepts no `role`. The
panel is nine lines of CSS in `globals.css` and a `role="dialog"` here.

⚠ **The expensive half loads on first ⌘K, not on page load.** kbar is CommonJS,
so `next build` cannot shake `fuse.js` or the virtualizer out of its barrel;
`lib/kbar.ts` re-exports the light half by deep import and `PaletteUI.tsx`
carries the rest behind `next/dynamic`. Measured in Chromium against the fixture
set: every fund screen pays 8.7 KB gzipped for the provider and the hint, and the
first open fetches 21.3 KB more. Importing `PaletteUI` statically from anywhere —
or re-exporting `KBarResults`/`useMatches` from `lib/kbar.ts` — quietly undoes
the split; both files say so at the line it would happen.

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

`console.yml` also runs six checks in `scripts/`. They are plain Python over
source text and need no node, so a developer can run any of them directly:

| | |
|---|---|
| `route_manifest_test` | the console calls exactly the contract's routes, every screen exists, and ⛔ **no RPC goes unread** |
| `fields_test` | the twenty fields and phrases whose absence has already shipped once |
| `fixtures_test` | every fixture is shaped like its message in `console.proto` |
| `tokens_test` | the design tokens still agree with `site/style.css` |
| `no_secrets_test` | nothing that belongs in the environment is committed |
| `book_kinds_in_plan_test` | every `BookKind` in `wire/types.ts` is named in `PLAN.md` |

⚠ **These were Bazel `sh_test`s and are not any more.** They went red twice on
Bazel wiring rather than on anything they check, and neither failure could be
reproduced without Bazel — which most environments editing this directory do not
have. ⛔ `console/BUILD.bazel` survives as a single `exports_files`, because
`//proto:mirrors_test` reads `src/wire/types.ts` through it.

## The plan screen

`/funds/{fund}/views/{view}/strikes/{strike}/plan` draws how a NAV was computed.

⛔ **IT IS A DESCRIPTION OF TWO CODE PATHS, NOT A PLAN THE ENGINE CHOSE**, and
the screen says so. Nothing in Ratio selects between them: `ratio_nav::strike`
folds the journal, `Projection::nav` reads maintained totals, and a caller picks
one by calling it. `Ratio.Plan` proves the two agree and is not emitted into
Rust at all. A diagram that implied a planner would be checked by nothing.

⛔ **BOTH GROUPS ARE ALWAYS ON THE PAGE, AND SO ARE THE THREE COSTS.** The fold
grows with the journal; the maintained read does not. Hiding the plans not taken
(the default) collapses sub-graphs and never the comparison — `ratio bench`
"reports two curves and both must be quoted", and this is the same rule.

⚠ **`?analyze=true` RE-FOLDS THE JOURNAL.** It is the slowest thing the API
does, which is why it is a control rather than something the page asserts on
load — the argument the replay screen already makes. What it measures is this
machine re-deriving the pinned prefix now, never what the original strike cost.

The diagram is inline SVG over the existing design tokens, laid out by
`src/lib/planLayout.ts` — pure arithmetic, no DOM measurement, so the server's
render and the browser's are the same. There is no charting library here and
this was not the place to add the first one.

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

Two required on a laptop; four more on a Vercel deploy (AuthKit).

| | |
|---|---|
| `RATIO_API_ORIGIN` | where the API is — **scheme and host, no path**. `http://127.0.0.1:7373` locally |
| `RATIO_CONSOLE_ORIGIN` | *optional locally.* This app's own origin |
| `WORKOS_CLIENT_ID` | Ratio Staging: `client_01M1JJZT4T0NN1WWT65NE6CV3W`. Empty skips IdP. **Never copied from another product** |
| `WORKOS_API_KEY` | AuthKit API key. Placeholder only in docs; never commit a real one |
| `WORKOS_COOKIE_PASSWORD` | ≥32 characters; `openssl rand -base64 32` |
| `NEXT_PUBLIC_WORKOS_REDIRECT_URI` | Must match a Redirect URI on the attached WorkOS application |

⭐ **WorkOS AuthKit is the sign-in path.** Cognito is not consulted. The API
Gateway JWT authorizer uses issuer
`https://api.workos.com/user_management/client_01M1JJZTFXFDZJ0XJM1NPNSEJB`
(the `iss` AuthKit session tokens mint; the bare `https://api.workos.com/`
host has no OIDC discovery and must not be used).
`https://auth.ratio.marsh.build` is the hosted AuthKit UI, not the
authorizer issuer. Audience = `WORKOS_CLIENT_ID`. Membership is still
`MEMBERSHIP.tsv`: `sub`, email, or `org:{workos_org_id}`. Creating a book
grants only the creator's `sub`.

The callback path is the one [AuthKit for Next.js](https://workos.com/docs/authkit/nextjs)
and the [authkit-nextjs README](https://github.com/workos/authkit-nextjs)
name (`handleAuth()` at `/app/callback/route.ts`), not Cognito's
`/api/auth/callback`. The Sign-in URL is `/sign-in`
(`app/sign-in/route.ts` in that README).

This repo does not compile a client id. Set `WORKOS_CLIENT_ID` to the
Ratio project's public identifier for the environment you are attaching:

| | Staging (local / AuthKit sandbox) | Production (Vercel + ratio.marsh.build) |
|---|---|---|
| `WORKOS_CLIENT_ID` | `client_01M1JJZT4T0NN1WWT65NE6CV3W` | `client_01M1JJZTFXFDZJ0XJM1NPNSEJB` |
| Redirect URI | `http://localhost:3000/callback` and `https://ratio.marsh.build/callback` | `https://ratio.marsh.build/callback` |
| Sign-in URL | `http://localhost:3000/sign-in` | `https://ratio.marsh.build/sign-in` |
| Sign-out URI | `http://localhost:3000` | `https://ratio.marsh.build` |

`WORKOS_API_KEY` and `WORKOS_COOKIE_PASSWORD` are secrets. They are never
committed. `/login` and `/api/auth/login` are the same initiate-login
handler as `/sign-in`. `/api/auth/callback` only sends the browser to
`/signin` (the prompt page).

⚠ **A wrong value fails the build, not the sign-in.** `pnpm build` runs
`scripts/preflight.mjs` first: it fetches `${RATIO_API_ORIGIN}/authconfig.json`
and, on Vercel, checks the WorkOS variables without printing secrets. **With
`RATIO_API_ORIGIN` unset it skips everything**, which is what keeps CI hermetic
and `next dev` offline.
