<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/mattmarshall/ratio/main/images/wordmark-dark.png">
  <img alt="ratio" width="240" src="https://raw.githubusercontent.com/mattmarshall/ratio/main/images/wordmark-light.png">
</picture>

**An accounting book whose core properties are machine-checked.**

Ratio keeps the books as an append-only journal of conserved postings and
proves what must be true of them: value is conserved on every dimension, a trial
balance ties because the kernel says so rather than because a check passed, a
figure cites the journal prefix it was folded from, and a sale gives up the lots
the terms dictate. The same kernel serves personal finance, investment
accounting and project finance — different charts and dimensions, not forked
products. The proofs are Lean 4, the concurrency and staleness arguments are
TLA+, the running system is Rust — and parts of the Rust are **emitted from the
Lean**, so the theorem and the code make the same decision.

### ▶ [Try the live demo](https://ratio.marsh.build/)

Sign in with Google or email and watch a real fund's books tie out — seven
seeded funds, one blocked by a break nobody has explained, one struck because
somebody did, and one keeping two books of record over a single journal. Every
break, NAV strike and configuration version has a URL, so a figure can be sent
to somebody rather than described.

The console is a Next.js application on Vercel; the API it reads is the same
binary you build below, running on AWS Lambda. If you would rather see a book
than a sign-in form, the
[trial balance](https://1h4q8av2gb.execute-api.us-east-1.amazonaws.com/balance)
is public and needs no account.

## Quickstart

Bazel is the only build path — `cargo build` does not work here, because the
kernel is emitted from the Lean at build time.

```bash
git clone https://github.com/mattmarshall/ratio.git && cd ratio

bazel test //...                                 # the proofs, specs, crates, and demos
bazel run //crates/ratio -- watch --book <dir>   # the API and the screens — open /balance
```

No database, no daemon, no container: the journal is a file and the
configuration is content-addressed beside it.

The **operations console** — the authenticated one, with a URL for every book,
break, strike and configuration — is a separate Next.js application in
[`console/`](console/), deployed to Vercel while the API keeps deploying to AWS.
It runs against the loopback server above with no identity provider and no
secrets:

```bash
cd console && pnpm install && pnpm dev          # then open http://localhost:3000
```

## Documentation

- **[HANDOFF.md](HANDOFF.md)** — what is load-bearing and what will bite. The most
  accurate document here; read it before changing the lot engine, the projection,
  or anything emitted from Lean.
- **[DEVELOPING.md](DEVELOPING.md)** — build, layout, and workflow.
- **[deploy/README.md](deploy/README.md)** — the hosted demo: the stacks, the
  WorkOS AuthKit setup, and how sign-in and tenancy work.
- **[Whitepaper](paper/)** · **[Marketing site](https://mattmarshall.github.io/ratio/)**
  · **[PLAN.md](PLAN.md)** (⚠ written 2026-08-07, since overtaken in places).
- **[docs/connect-scopes.md](docs/connect-scopes.md)** — WorkOS Connect
  scope catalog ([#150](https://github.com/mattmarshall/ratio/issues/150)).
  Connect tokens accepted with catalog scopes; first-party
  Connect apps call ConnectApiUrl. WorkOS dashboard registration
  stays leftover on issue 22.
- **[connect/bank-feed/](connect/bank-feed/)** — first-party Connect app
  that maps household bank rows onto Personal templates
  ([#165](https://github.com/mattmarshall/ratio/issues/165)). Scaffold;
  first-party Connect apps call ConnectApiUrl, and live bank OAuth is leftover.
- **[connect/tax-pack/](connect/tax-pack/)** — first-party Connect app
  that emits 8949-ish CSV from lot, wash, and lot-terms cites
  ([#166](https://github.com/mattmarshall/ratio/issues/166)). Scaffold;
  mixed dates stay unclassified; first-party Connect apps call
  ConnectApiUrl; IRS e-file is refused.
- **[connect/goals/](connect/goals/)** — first-party Connect app
  for household net-worth goals and what-if scenarios
  ([#168](https://github.com/mattmarshall/ratio/issues/168)). Scaffold;
  cites sheet / bridge / cash-flow; scenario journals post only on
  opt-in; first-party Connect apps call ConnectApiUrl; not a cash forecast.
- **[connect/aia-pay-app/](connect/aia-pay-app/)** — first-party Connect
  app that emits G702-ish / G703-ish CSV from Project billing and
  budget cites ([#184](https://github.com/mattmarshall/ratio/issues/184)).
  Scaffold; missing cites stay unset; first-party Connect apps call
  ConnectApiUrl; a licensed AIA form is refused.
- **[connect/vendor-portal/](connect/vendor-portal/)** — first-party
  Connect app for a Project vendor / GC portal
  ([#172](https://github.com/mattmarshall/ratio/issues/172)). Scaffold;
  cites billed / earned / retainage / collections; vendor invoices are
  allowlisted `journals:post` for `vendor_invoice*`; first-party
  Connect apps call ConnectApiUrl; no vendor user directory in core.
- **[connect/eac-forecast/](connect/eac-forecast/)** — first-party
  Connect app that emits EAC / ETC CSV from Project budget and billing
  cites ([#169](https://github.com/mattmarshall/ratio/issues/169)).
  Scaffold; remaining to spend stays revised − incurred − awarded;
  unset EAC is blank, not a silent 0; first-party Connect apps call
  ConnectApiUrl; `/budget` still does not forecast.
- **[connect/program-rollup/](connect/program-rollup/)** — first-party
  Connect app that rolls per-book Project budget and billing cites
  across membership-visible PROJECT books
  ([#179](https://github.com/mattmarshall/ratio/issues/179)). Scaffold;
  unset billed / collected stay blank, not a silent program 0.00; an
  `org_id` claim is not membership; first-party Connect apps call
  ConnectApiUrl; no mega-book in the kernel.

## License

**AGPL-3.0** ([LICENSE](LICENSE)), and available under a commercial license
([LICENSING.md](LICENSING.md)). Copyleft because the argument for Ratio is that a
figure can be *checked*, and that argument is weak if the code cannot be read;
§13 means a modification made by someone operating this over a network comes back
rather than becoming a private advantage. The hosted service is the path with no
copyleft obligation on the customer.
