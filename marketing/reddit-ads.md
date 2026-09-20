# Reddit ad concepts — accounting for developers

Nine paid-post concepts positioning Ratio to **engineers** the way Resend
positions email: the product is the API call, and the snippet is the pitch.
Ratio is a machine-checked ledger kernel you post to over gRPC, embed, run on
your own store, and point a simulation at.

**Figma:** <https://www.figma.com/design/cEk1cBfIJiHnKhSS0cmn48>
— `Ad concepts` (creatives), `In-feed previews` (how they read in a feed),
`Brief & claims ledger` (this document, laid out).

This file is the checkable copy of the brief. Every claim that appears on a
creative is cited to the file that backs it, for the same reason the site
carries its status sections: a campaign that asks the reader to *check* rather
than trust has to be accurate about itself, or it argues against itself.

---

## The position, and what makes each claim true

### Correct

The kernel's sole invariant is machine-checked in Lean 4, and the Rust records
are emitted from that Lean — **never committed**, so nothing hand-written can
drift from the proof.

- `lean/Ratio/Core.lean` — `theorem ledger_conserves`
- `lean/Ratio/Kernel/Emit.lean`, and 46 Lean sources under `lean/`
- `crates/ratio-kernel/BUILD.bazel` — `genrule` emits `src/generated.rs` from
  `//lean:ratio_kernel_ast` and consumes it directly as a `src`
- `git ls-files crates/ratio-kernel/` returns `BUILD.bazel`, `Cargo.toml`,
  `src/lib.rs` — and no `generated.rs`
- `tla/` — 82 TLA+ models

⚠ The `ratio-kernel` and `ratio-common` doc comments say the emission is
"gated by a `diff_test`". No target of that name exists in the tree. The
structural argument above is both true and stronger, so the creatives make
that one instead. The stale doc comments are worth a separate look.

### Embeddable

One crate, one dependency. CLI, gRPC **and REST on one port**, and MCP over
stdio, from one binary — the one you build locally is the one that runs on
Lambda. `CreateTransaction` refuses an unbalanced post with
`FAILED_PRECONDITION`, over either protocol, from one implementation.

- `crates/ratio-kernel/Cargo.toml` — `[dependencies]` is `ratio-common`, alone
- `proto/ratio/v1/ledger.proto` — `Ledger.CreateTransaction`, and its HTTP
  binding `POST /v1/{parent=books/*}/transactions`, served natively by
  `crates/ratio-api/src/rest.rs` from the same trait impl
- `crates/ratio-api/src/lib.rs` — `tonic::Status::failed_precondition(
  "transaction does not conserve value: postings must net to zero")`, and the
  test `create_rejects_unbalanced_with_failed_precondition`
- `crates/ratio/src/main.rs` — `USAGE`: `server` · `mcp` · `watch`
- `crates/ratio-mcp` — MCP over stdio, line-delimited JSON-RPC

`ratio server --book DIR` serves gRPC and REST on one port; the `POST` on
concept 00's creative is a real call. `crates/ratio-api:rest_routes_test`
holds the route table to the contract. SDKs: `crates/ratio-client` (Rust),
`sdk/python`, `sdk/typescript` — see `sdk/README.md`. The public `/balance`
on the demo host is the console API, a different surface.

### Pluggable persistence

Three seams stated as traits — `ConfigStore`, `FactStore`, `Journal` — so the
implementation can change without the callers noticing, over a three-method
`ObjectStore`: memory, a directory, or S3, one object per entry with one
conditional PUT. `RATIO_PG_URL` adds a Postgres read projection; the journal
stays the book of record wherever it sits.

- `crates/ratio-store/src/lib.rs` — the three seams, stated as traits
- `crates/ratio-store/src/objects.rs` — `ObjectStore`: `put_if_absent` ·
  `get` · `list`; `MemoryStore`, `DirStore`
- `crates/ratio/src/scale.rs` — the S3 adapter, installed once at process start
- `tla/S3Journal.tla` — what the backend must guarantee: create the key or
  report it held, never overwrite
- `crates/ratio-sql-project/src/reads.rs` — `RATIO_PG_URL`; unset is the
  in-memory fold

### One model

Personal, Investment, Project and Operating are kinds of book over one kernel.
Kind selects the chart `chart_for` writes and the screens `screensFor` offers —
different partitions of the same conserved quantities, not forked products.
Every label a posting carries is exactly one of conserved, partitioning or
measured.

- `crates/ratio-console/src/book.rs` — `BookKind`; `chart_for`: "personal and
  project are different partitions of the same conserved quantities"
- `console/src/lib/screens.ts` — `screensFor`: "kind selects the list"
- `lean/Ratio/Chart/Dimensions.lean` — conserved / partitioning / measured;
  currency nets to zero, an account says where it sits, a quantity is counted
- `crates/ratio/src/main.rs` — `ratio init --kind` accepts all four

### For simulation

`ratio gen` builds a fund from named dials with no RNG crate — byte-identical
on any machine. `ratio bench` recorded a 20,000,000-lot projection fold at
17.4 s; `ratio closure` costs a period end before running it; `ratio replay`
re-derives a struck NAV and proves it again.

- `crates/ratio-gen/src/lib.rs` — the dials, and why there is no RNG crate
- `HANDOFF.md` — 10,000 × 2,000 = 20,000,000 lots, 17.4 s, digest cited
- `crates/ratio/src/main.rs` — `USAGE`: `gen` · `bench` · `closure` · `replay`
- `demo/rehearse.sh`, `demo/shadow-run.sh` — the demos, asserted in CI

⛔ The full ~40 GB / 140M-entry journal fold is **not** measured, and every GPU
and NAV-runtime figure on the platform page is **modeled, never measured**.
Neither may appear as achieved performance. Concept 05's creative carries that
caveat in the artwork, not in a footnote somebody can crop.

---

## Register

### Say it this way

| Line | Source |
|---|---|
| "Accounting for developers" — the umbrella; every concept hangs off it | this brief |
| "AGPL-3.0, with a commercial license alongside it" | `LICENSING.md` |
| "machine-checked" · "emitted from the Lean" · "measured in-repo" | `lean/`, `HANDOFF.md` |
| Name the shape behind every number — 10,000 securities × 2,000 lots, 17.4 s | `HANDOFF.md` |
| "modeled, never measured" on any figure that was not recorded | `site/README.md` |
| American English throughout — modeled, color, license, gray, judgment | `site/README.md` |

### Never — and the check that catches it

| Phrasing | Why | Check |
|---|---|---|
| "open core" · "MIT licen…" · "permissively licensed" · "use it however you like" | implies the permissive grant the license no longer carries | `site/verify.py`, `marketing/verify_language.py` |
| "not open source" | AGPL-3.0 is OSI- and FSF-approved, so this is now false | `marketing/verify_language.py` |
| "Rust/Python" | the core stack is Lean-authored and Rust-emitted | `marketing/verify_language.py` |
| "fastverk" · `ratio.fastverk.dev` · `com.fastverk` | internal naming; the protos now say `ratio.marsh.build`, and it must not come back on a creative | `marketing/verify_language.py` |
| A modeled figure restated as achieved performance | nothing has been run on a 20M-lot book at NAV scale | `site/README.md` |
| A named fund complex behind the 20M-lot workload | it is a scale illustration, not a customer | `site/README.md` |
| Performance, attribution or composites as features | not committed; PLAN's refusal stands | `docs/current-state.md` |

### Targeting is a register decision, not a style note

`site/verify.py` fails the build if the practitioner page (`index.html`)
acquires *kernel*, *Lean*, *conservation*, *Rust*, *append-only* or *GPU* —
because a fund accountant reading that vocabulary has been sent to the wrong
document. **Point these nine ads at engineers.** An operations or
fund-accounting audience needs `index.html`'s language, which is a different
set of creatives and is not in this file.

---

## Format notes

- Creatives are **1200 × 628** (1.91:1) for the feed link ad, with 1080 × 1080
  variants for mobile feed (concepts 00, 02, 05, 06).
- On Reddit the **headline is the post title** and the **CTA button is Reddit's
  own chrome** — neither is baked into the image. The image's job is to prove
  the claim the title makes.
- Titles may run to 300 characters; these are kept near 120 so nothing
  truncates on mobile.
- Subreddit lists below are a starting point. Confirm each is ad-eligible in
  Reddit Ads Manager before buying.

---

## The nine concepts

### 00 · Accounting for developers — dark · the hero

> Accounting for developers — a machine-checked ledger kernel you post to over
> gRPC, embed as a crate, and run on any store

- **CTA** Try the demo
- **Subreddits** r/programming · r/rust · r/ExperiencedDevs · r/webdev
- **Creative** the one call: `POST /v1/books/fund-1/transactions` with two
  postings that net to zero, then the same call with `124999` returning
  `FAILED_PRECONDITION: transaction does not conserve value`
- **Backed by** `proto/ratio/v1/ledger.proto`; `crates/ratio-api/src/lib.rs`
  and `rest.rs`; `crates/ratio/src/main.rs` — `server` · `mcp` · `watch`

### 01 · Theorem — cream

> The balance check in our accounting kernel isn't a unit test — it's a Lean 4
> theorem, and the Rust that runs is emitted from it

- **CTA** Read the proof
- **Subreddits** r/rust · r/ProgrammingLanguages · r/programming · r/haskell
- **Creative** `theorem ledger_conserves` set in a dark panel beside the claim
- **Backed by** `lean/Ratio/Core.lean`; `crates/ratio-kernel/BUILD.bazel`;
  `generated.rs` is untracked

### 02 · Overflow — dark

> An unbalanced journal entry that passes an i64 balance check — and the
> accounting kernel that sums in i128 instead

- **CTA** Read the kernel
- **Subreddits** r/rust · r/programming · r/cpp · r/embedded
- **Creative** the three-row table: `[i64::MAX, i64::MAX, 2]`, true sum
  `18446744073709551616`, `i64 .sum()` → `0`
- **Backed by** `crates/ratio-kernel/src/lib.rs` — the `transaction_total_exact`
  doc comment, near-verbatim; `Cargo.toml` — one dependency

### 03 · Bring your own store — cream

> The store under our accounting kernel is a three-method trait. Memory, a
> directory, or S3 — and a TLA+ spec for what it must guarantee

- **CTA** Read the trait
- **Subreddits** r/rust · r/softwarearchitecture · r/programming · r/aws
- **Creative** `pub trait ObjectStore` verbatim — `put_if_absent`, `get`,
  `list` — over chips for `MemoryStore`, `DirStore`, `S3` and
  `RATIO_PG_URL → reads`
- **Backed by** `crates/ratio-store/src/objects.rs`;
  `crates/ratio-store/src/lib.rs`; `crates/ratio/src/scale.rs`;
  `tla/S3Journal.tla`; `crates/ratio-sql-project/src/reads.rs`

### 04 · One kernel, four kinds — green surface

> Personal, investment, project and operating books run on one accounting
> kernel — kind selects the chart, not a fork

- **CTA** Build a book
- **Subreddits** r/programming · r/SideProject · r/fintech · r/selfhosted
- **Creative** `ratio init --kind` × four, over the classification —
  CONSERVED currency nets to zero · PARTITIONING account says where it sits ·
  MEASURED quantity is only counted
- **Backed by** `crates/ratio-console/src/book.rs`; `console/src/lib/screens.ts`;
  `lean/Ratio/Chart/Dimensions.lean`

### 05 · Simulation — cream

> A deterministic ledger generator with no RNG crate: same dials, same seed,
> byte-identical 20M-lot book on any machine

- **CTA** Run the bench
- **Subreddits** r/algotrading · r/rust · r/fintech · r/ExperiencedDevs
- **Creative** `ratio gen --securities 10000 --lots-per 2000 --seed 7`, then
  `ratio bench --fold` → 20,000,000 lots folded in 17.4 s
- **Backed by** `crates/ratio-gen/src/lib.rs`; `HANDOFF.md`;
  `crates/ratio/src/main.rs`
- ⚠ The creative states in the artwork that this is the **projection** fold,
  not the full 140M-entry journal fold.

### 06 · Agent fence — dark

> We gave an LLM write access to an accounting ledger and deleted the approve
> tool. Not permission-checked — not dispatched

- **CTA** Read the fence
- **Subreddits** r/programming · r/LocalLLaMA · r/mcp · r/ExperiencedDevs
- **Creative** the four-row capability table, with the `approve a rule` row
  struck for the model and reading `ratio approve` for the human
- **Backed by** `crates/ratio-mcp/src/lib.rs` — the fence table, verbatim;
  `ORCHESTRATION.md`; `docs/connect-scopes.md` — `rules:approve` is absent

### 07 · Emitted, not written — dark

> cargo build doesn't work in our repo — and the kernel's Rust isn't in the
> repo either. It is emitted from the Lean on every build

- **CTA** See the build graph
- **Subreddits** r/rust · r/bazel · r/ProgrammingLanguages · r/programming
- **Creative** the emit pipeline: `Emit.lean` → `ratio_kernel_ast` →
  `json_to_rust` → `generated.rs`
- **Backed by** `README.md` — "Bazel is the only build path";
  `crates/ratio-kernel/BUILD.bazel`; `lean/BUILD.bazel` —
  `lean_emit(name = "ratio_kernel_ast")`

### 08 · The mark — cream

> However the entries are divided, the totals are equal — an accounting book
> whose core properties are machine-checked

- **CTA** See the live book
- **Subreddits** r/rust · r/programming · r/fintech · r/opensource
- **Creative** `mark-ratio.svg` drawn to scale, with its split annotated
  38 + 62 = 100
- **Backed by** `README.md`; `site/marks/mark-ratio.svg` — two rows of equal
  ink, the top one cut 38/62

---

## Open before any of this is bought

- **The CTAs have no destination.** `site/README.md` already flags this for the
  website's buttons, and an ad is worse: a paid click that lands on a page with
  nothing to say yes to is a paid bounce. Decide what "Read the trait" opens —
  the repository, the live trial balance, or a contact route.
- **`ratio.marsh.build` is the URL on every creative.** Confirm that is the
  destination for a developer audience rather than the console sign-in, which
  is what that host serves today.
- **The repository must be public** for most of the CTAs to work at all. The
  same checklist item is open on the website.
- **Reddit disclosure.** These run as Promoted posts from a Ratio account.
  Decide the account name before the first buy; it appears above every headline.
- **`marketing/BUILD.bazel` still describes the license as "NOT open source and
  NOT self-hostable"** in a comment, which `marketing/verify_language.py` now
  bans in the prose it checks. The comment is not checked, so it has not gone
  red. Unrelated to this campaign, but it is the same class of drift.
