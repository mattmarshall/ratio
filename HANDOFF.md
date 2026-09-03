# Handoff — tax lots, corporate actions, and the dimensional chart

**State**: 51 bazel tests green, 18 `lean_test`, 20 `tla_check`, 13 `manual`
probes all red for the reasons they name.

Issues #4 and #7 are closed. Open work is #5, #6, #8, #9. This file is the part
that does not fit in an issue: what was learned, what is load-bearing, and what
will bite.

## ⛔ Both closed issues had a false premise, and finding it was most of the work

**#4 said "the plumbing exists — this is rendering, not design".** It did not.
`fold_lots` called `relief::relieve`, which is FIFO whatever the fund elected, so
`RuleSet.lot_method` was parsed, stored, content-addressed, pinned by every
journal entry — and read by nobody. Rendering the method on top of that would
have put a screen behind a value the engine did not use.

**#7 said "make the postings carry currency and the law starts doing work".**
Carrying it was necessary and not sufficient: `Totals.by_dim` keyed on the
dimension and dropped the currency, so the first multi-currency book would have
had a NAV adding dollars to euros and reporting the result as USD.

⚠ **The lesson is not "the issues were sloppy".** Both were written by someone
who had just built the surrounding code. The premise that fails is the one
nobody thought to check, and in both cases it was "the value I stored is the
value that gets read".

---

## Read this first: how things fail here

Every defect found in this stretch had one shape. **The books tie and the number
is wrong.** Conservation, the trial balance, and the digest are all satisfied by
the wrong answer, so nothing downstream reports anything.

| what happened | what noticed |
|---|---|
| An unbalanced transaction summed to zero by `i64` wraparound and passed the door | a test written for something else |
| `per_share(x, 0)` panicked on the NAV path — a fund with no units in issue | a sweep of the emitted surface |
| A husk (zero units, carrying cost) handed its whole basis to a sale that got no units | a Lean `example` |
| HIFO silently performed FIFO — the tiebreak overrode the method | Lean's `decide` reporting a theorem FALSE |
| A projection's lots and its positions drifted, each internally consistent | a reconciliation test between the two paths |
| `[USD +100, EUR −100]` passed as balanced | `Ratio.Chart.Dimensions` |
| A fund electing HIFO was relieved FIFO — the configuration was read by nobody | tracing a call site while planning to render it |
| A NAV summed dollars and euros and called the answer USD | the same trace, one layer out |
| An FX rate of "70.00 dollars to the euro" made a gain 70× its basis | translating for real; the only consumer had been discarding the result |
| The console and the CLI reported different NAVs for one book, neither saying which | reading both on the same book |

⛔ **The realized gain is the figure with no counterparty.** A wrong NAV is caught
by a reconciliation. A wrong gain is caught by nobody until a tax authority asks.
Most guards in the lot engine are about it rather than about conservation.

⚠ **Mutation testing found three green suites that tested nothing** — the ex-date
boundary, the per-instrument filter, and the lot's trade-date propagation. All
passed; all covered the code and none covered the case. Break the thing you
think a test protects, and check the test fails.

---

## The gap that is not in any proof

Every theorem here is over Lean's `Int`, which is unbounded. Every emitted
function runs on `i64`, which is not. `Ratio.Bounded` is where that hypothesis is
finally written down; before it, the failure was not a crash:

```
4_000_000_000_000_000_000 * 3        wraps to  -6446744073709551616
(-6446744073709551616).rem_euclid 2  ==        0
```

A divisibility guard says YES about a product that never happened, and the proof
cannot see it because in `Int` the multiplication simply happened.

⛔ **Any new emitted function that multiplies or negates needs a bounds check
before the emitted code is asked anything** — asking first is asking about a
wrapped number. `ratio_common::checked` is the Rust side.

⚠ Still unguarded, low consequence and named rather than hidden: `money_add` /
`money_neg` in the emitted kernel (barely reached — `Ratio.Core` conserves over
`Vec Dim`, not `Money`), and `neg_part` in reporting.

---

## The architecture decision worth not re-litigating

`Ratio.Core.Conserves` is `v = Vec.zero` — zero in **every** dimension — and
`Dim` is the conserved-quantity basis. The running kernel had collapsed it to one
sum. The correction is a classification, and it is the whole chart design:

| | |
|---|---|
| **conserved** | **currency**. Two currencies are two independent laws, not one law over a sum. |
| **partitioning** | account, instrument, share class, counterparty. They say *where* value sits, do **not** net to zero, and roll up. |
| **measured** | quantity. Buying 100 shares *creates* 100 in the book, so a conservation check over units would refuse every purchase. |

A chart of accounts is the partitioning dimension people argue about. It is not
the conserved one, and the kernel never said it was.

---

## Load-bearing things a change could quietly break

- **`Terms` is resolved PER ENTRY, from the config that entry pinned** — not once
  per projection. The lot method, the chart roles and the holding-period
  threshold are all terms of an administration agreement, and a fund that changes
  method mid-year must not have its earlier sales restated.
  `//tla:stale_method_relief_check`. Cached by digest, because a book has a
  handful of configurations and millions of entries naming them.
- **A config that cannot be read REFUSES the relief.** There is no fallback to
  FIFO: FIFO is a method real funds elect, so a book relieved under it by
  accident is indistinguishable from one relieved under it by agreement.
- **`Totals.by_dim` keys on (dimension, currency).** A total over both is not a
  figure — `a_flat_total_hides_a_currency_mismatch`. `nav` and `realized`
  translate through an explicit `Rates` or refuse, and `Rates` carries its BASE
  explicitly because there is no rate fact for it and "the one that is missing"
  is not something to infer from a data file.
- **`Realized::unclassified` is DERIVED, never accumulated** — `gain − short −
  long`. That is what makes the three parts sum to the total by construction. It
  absorbs the translation residue (at most one minor unit per currency; integer
  translation does not distribute over a sum), and a fourth accumulator would put
  that residue nowhere and leave four figures that do not add up.
- **The credit-normal flip lives in `format.gain`, in one place.** Applied per
  call site it gets applied twice somewhere and nowhere else, and both mistakes
  produce a plausible number.
- **`FUND_CURRENCY` is one constant** because it is two answers to one question
  otherwise: what `Fund.currency_code` labels the figures, and the base `Rates`
  translates into. A NAV translated into euros and labelled USD is wrong by the
  rate and looks entirely ordinary.
- **`AsOf<T>`** (`ratio-project`). Every read carries the journal prefix it was
  folded from, and there is no other way to get a number out — so a caller
  cannot pin the head while reading a lagging projection. `//tla:projection_
  check`. ⚠ Weaker than it looks: nothing stops taking `.value` and pairing it
  with a position from elsewhere. What the type buys is that doing so requires
  saying so.
- **`advance` uses `max`, not assignment.** A shorter journal must not rewind the
  prefix, or the next advance re-folds and double-counts.
- **`follow` refuses a journal shorter than it read.** An append-only log does
  not shrink, so that is a *different book*; resuming would splice two histories.
- **Announcements are journal entries.** Under the factor representation nothing
  is applied, so an action in a side plane is pinned by no strike and a replay
  answers differently as the world tells us more. `Ratio.Actions.Factor.replay_
  is_determined_by_the_prefix`.
- **Corporate actions are read through per STEP, never as a composed ratio.**
  3-for-2 then 2-for-1 composes to ⟨6,2⟩, divides five units cleanly, and
  silently swallows the half-share the holder was paid cash for.
- **`relieve` sorts by `seq` itself.** `Ratio.Lots.relieveFifo` takes the head, so
  it is FIFO exactly when the caller supplied acquisition order — and a
  projection keyed by instrument does not.
- **The gain leg is derived, never supplied.** A parameter for it would be drift
  written down as an API, and the drift is silent because the gain leg absorbs
  whatever the other two legs leave.
- **A break is graded by the configuration ITS REPORT NAMES**, resolved once in
  `breaks_for` and never from `active()`. A severity is part of a comparison
  between two figures produced under one agreement; regrading a stored report
  because somebody promoted a new rule set changes an answer whose inputs never
  moved — `an_unpinned_announcement_changes_the_answer`, applied to a grade.
  `a_break_is_graded_by_the_configuration_its_report_names_not_the_one_in_
  force_now` is the only thing standing there, and pointing this at `active()`
  passes every other test in the file.
- ⛔ **What the grader cannot read grades HIGH, and that is the design.** An
  unparseable digest, bytes the book does not hold, bytes that are not a rule
  set — all blocking, none defaulting. The temptation is to "fix" it to the
  custom bands because grading everything HIGH looks like a bug; doing that
  certifies a difference as small using a tolerance nobody could read, on a book
  that ties. `Tolerance` is `None` on such a break rather than claiming bounds.
- ⭐ **`Console::blocking_at` IS THE ONE FOLD OF WHAT BLOCKS, AND BOTH THE BADGE
  AND THE REFUSAL READ IT.** `get_fund` derives `STATE_BLOCKED` from it and
  `ratio strike` refuses on it. Two folds would be individually plausible,
  independently maintained, and one field apart within a month — which is
  exactly what the seam WAS: a screen saying BLOCKED and a command that never
  asked. `the_gate_and_the_fund_state_are_one_derivation` holds them together.
- ⛔ **AN EXPLANATION IS KEYED BY THE BREAK'S ID WITHIN ITS BOOK, NOT BY THE
  RESOURCE NAME.** The fund half of `funds/{fund}/breaks/{id}` says how the book
  is being SERVED — the same directory is `demo` on loopback and
  `pennington-select-income` under a funds root — so a note written by the
  seeder never matched the break the console showed. The explanation sat on
  disk, the break sat on screen, and nothing connected them.
  ⚠ **No unit test could have caught it**: every one of them uses a
  root-that-is-a-book, where the fund is always `demo`. The seeded demo found
  it, which is the argument for `//deploy:seed_test` asserting what the demo
  DEMONSTRATES.
- ⭐ **AN EXPLANATION'S CURRENCY TEST IS `(difference, config_digest)` AND
  DELIBERATELY NOT THE JOURNAL POSITION.** Both directions are a real failure and
  both are negative-tested. Add the prefix to the test and every explanation
  retires on the next posting — on a NAV morning that is a gate nobody can ever
  clear, which looks like the software being careful
  (`posting_an_entry_does_not_unexplain_a_break`). Drop the figure and
  explanations become eternal, so a fund is struck on a difference somebody
  explained in February (`a_reconciliation_that_moves_the_figure_retires_the_
  explanation_and_says_what_moved`). The position and digest ARE recorded — they
  say what the accepter was looking at — and nothing reads them to decide
  currency.
- **`Plane::Explanations` is beside the journal, and that is the opposite call
  from `Plane::Actions`.** An announcement changes what the books say once it is
  applied, so a side plane left it pinned by no strike; an explanation changes
  no figure at all, only whether the break is worked. ⚠ The moment something
  lets one move a number, it belongs in the journal with everything else that
  does.
- **`ratio accept` writes `accepted` to CHANGELOG, and `config_versions` filters
  on `approved`.** That filter exists because a line keyed by the same digest
  under a different verb would report the last person who did something under a
  configuration as the one who approved it. `an_accepted_line_is_not_read_as_a_
  configuration_promotion` holds it.
- **A lot break cannot be explained.** Its name is `lot-{n}` — a POSITION IN A
  LIST — so an explanation keyed on one would follow the position rather than
  the sale the moment an earlier lot break clears, with every citation still
  resolving and the words attached to a different disposal. Making those names
  durable is a `ratio-project` change; until then `accept_explanation` refuses
  and names the correcting entry instead.
- **`Break.tolerance` is null on a lot break ON PURPOSE.** Lot breaks are HIGH
  by what they mean — the lot book and the position disagreeing corrupts the
  realized gain, which no reconciliation reaches — so reporting bounds beside
  one would suggest some other number would have graded it differently. None
  would.
- **`ChartRoles` are checked when the configuration is READ.** A chart that
  cannot express a gain is wrong the moment it is written down; finding out at
  the first disposal means finding out in production.
- **EVERY VIEW FOLDS ONE PREFIX, IN ONE PASS.** A book keeps more than one book
  of record — ABOR recognises a trade when it is struck, a settlement view when
  cash and stock move — and they are N folds inside ONE `Projection` with one
  `at` and one `read_to`. A projection per view is `//tla:sql_projection_check`'s
  `AFigureIsFoldedFromOnePrefix` with views where that spec has tables: both
  folds correct, both books tying, and the difference between them partly a
  settlement convention and partly one of them being three entries behind, in
  one number, with nothing saying which part is which.
  `//tla:views_at_two_prefixes_check`. It also pays `parse` once, which at 65%
  of the cold build is the difference between N views and N × the build.
- **`recorded` IS NOT `settlement 0`.** A book that declares no view has one,
  and it consults no date — which is the only thing that answers over the
  entries carrying no `trade_date`, i.e. most of every book written so far. A
  same-day settlement convention reads the calendar and REFUSES such an entry.
  `Ratio.Views.nobody_said_is_not_a_settlement_convention`; `View.declared` is
  the field that keeps the two apart, for the reason `lot_method_declared`
  exists.
- **The view SET comes from ACTIVE; the CONVENTION comes from the digest the
  entry pinned.** Which views exist is a question about now. How an entry is
  recognised is a term of the agreement in force when it was posted — `Terms`'
  rule, one level out. ⛔ An entry whose pinned configuration does not declare a
  view must be REFUSED by it and reported, never folded as `recorded`: that is
  the no-fallback-to-FIFO argument with a date instead of a lot method.
- ⚠ **THE MAINTAINED PROJECTION DOES NOT FOLD PER VIEW YET, AND THE BFF REFUSES
  RATHER THAN PRETENDING.** `ratio strike` cuts — it derives the day from the
  valuation point — so the RECORDED NAV is already per view and correct. The
  projection behind the console's screens folds the whole journal with no cut,
  so it can only answer for a `recorded` view; `Console::view_the_projection_
  can_answer` refuses anything else, and `ReconcileViews` refuses outright.
  ⛔ THE REFUSAL IS THE POINT. Serving a trade-date or settlement view off a
  cut-less fold would return the recorded view's figures under another name —
  two labels, one number, and nothing saying so. That is the defect the whole
  feature exists to prevent, not a rough edge in it.
  ⚠ AND `console/fixtures/reconcile.json` DESCRIBES A SHAPE NOTHING SERVES.
  The render test is green against it; the endpoint 400s. The fixture documents
  the intended response and the screen shows the server's refusal prose, which
  is honest — but a fixture that outlives the gap it was written for is how a
  captured fixture stops being a capture.
- ⛔ **A DIFFERENTIAL TEST BETWEEN TWO VIEWS THAT DOES NOT CUT IS VACUOUS.**
  Folded to the end of history every view agrees, because everything eventually
  settles — `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_gap` is the
  theorem, written down before anybody fell into it. And the second trap is
  sharper: a PURCHASE moves cash into investments, both assets, so recognising
  it or not moves a NAV by ZERO. **Subscriptions are the shape that works**,
  exactly as they were for the multi-currency version that was vacuous twice.
- ⚠ **WHAT `//deploy:seed_test` ACTUALLY PROVES, WHICH IS LESS THAN IT LOOKS.**
  It checks the two views' recorded NAVs DIFFER and that the book ties. It does
  NOT check that the entries in flight account for the difference, because
  `ratio reconcile` does not exist and waits on the same per-view fold the BFF
  does. The stronger claim — `Ratio.Views.two_views_differ_by_exactly_what_is_
  in_flight` — is proved in Lean and asserted nowhere in the demo. ⛔ The gap is
  written into the script rather than left to be inferred from its absence.
- ⚠ **THE GENERATOR'S SETTLEMENT TAIL IS ANCHORED TO THE DAY IT RUNS, AND ONLY
  IT.** `FIRST_TRADE_DAY` is a constant so that every measurement taken against
  a generated book is reproducible, and that stays true: `--settle-tail` writes
  the ONLY entries dated from a day the caller passes in. It has to be, because
  `ratio strike` values at NOW and a tail that has already settled by then makes
  the two views agree. So `ratio gen` with no `--views` is still byte-identical
  run to run, and `ratio gen --settle-tail` deliberately is not.

- ⚠ **THE PLAN SCREEN EXPLAINS TWO PATHS AND MODELS ONLY ONE OF THEM, WHICH IS
  THE MODEL BEING HONEST RATHER THAN A GAP TO FILL IN.** `ExplainNavStrike`
  draws `ratio_nav::strike` beside `Projection::nav`. `Ratio.Closure` costs a
  period end off a projection that is ALREADY CURRENT — it has no term for
  parsing a journal and there is no theorem about what a fold costs — so every
  step of the recorded fold carries an estimate only where the strike record
  supplies one (the pinned prefix) or the shape does (accounts, currencies).
  ⛔ The rest are blank, never zero. Anybody tempted to "fill in the missing
  estimates" would be inventing a cost model for the one curve this repository
  measures instead.
- ⛔ **AND WHAT `?analyze=true` MEASURES IS THIS MACHINE RE-DERIVING THE PREFIX
  NOW, NOT WHAT THE STRIKE COST.** Nothing is recorded at strike time. Putting
  per-stage costs in the `NAVS` file was considered and refused: it widens an
  append-only record whose eight-field legacy read is already documented, for a
  number that is a property of a machine on a day. Every actual on the screen
  says so beside itself rather than once at the bottom.
- ⚠ **`Rates::len()` IS THE FACTS, `Ratio.Closure.fxCost` IS THE CURRENCIES, AND
  THEY DIFFER BY ONE ON EVERY BOOK.** The base has no rate fact — a fund does
  not record what a dollar is worth in dollars. So the plan's `Read Rates` step
  measures one lower than it estimates, always, and the node carries a note
  saying why. Read as a defect it would send somebody looking for a missing
  rate; the missing one is the denomination the figure is reported in.
- ⚠ **`capital_txns` IS PASSED AS ZERO AND REPORTED AS BLANK, AND THE TWO ARE
  NOT THE SAME CLAIM.** Counting subscriptions and redemptions needs the chart
  roles and `Projection` deliberately does not know the chart, so the model gets
  a zero it needs to typecheck and the screen gets nothing. ⛔ Every total on
  that screen therefore EXCLUDES capital activity and says so. A future change
  that starts counting it must move both, or the steps will stop adding to the
  figure beside them.
- ⛔ **`explain.json` IS THE ONE FIXTURE IN `console/fixtures/` THAT WAS NEVER
  CAPTURED.** Bazel could not fetch two `tomato-bazel` modules in the
  environment it was written in (403 under an egress policy), so no `ratio
  watch` could be built to capture from. The steps, citations, notes and every
  modelled figure came out of `ratio_nav::explain::plan_of` itself and the
  encoding was copied from `transcode.rs` — but the FUND'S SHAPE is chosen to
  agree with `view.json` rather than read off a book. `capture_fixtures.sh` says
  so at the top and now captures it; run it and commit whatever comes back.
  ⚠ This is the same shape as `console/fixtures/reconcile.json` two entries
  above: a fixture that outlives the gap it was written for is how a captured
  fixture stops being a capture.

---

## Traps that cost real time

- ⛔ **`bazel cquery --output=files` can hand back a stale binary.** Twice: an API
  returned an empty list and the code was right. Explicit `bazel build` first,
  then query.
- ⛔ **The console is not in the binary and Bazel does not build it.** This entry
  used to say a console change needed `//crates/ratio` rebuilt, because
  `//web:console_html` → `//crates/ratio:console_rs` → `src/console_html.rs`
  embedded the whole page as a `&str` at compile time. `console/` is a Next.js
  application deployed to Vercel; the JS toolchain left `MODULE.bazel` with it.
  Three consequences worth knowing before you look for something that is gone:
  - `bazel test //...` **is no longer the whole gate.** `.github/workflows/
    console.yml` is a required check and `CONTRIBUTING.md` says so.
  - `//crates/ratio:ratio_test`'s `the_served_console_carries_the_lot_engine`
    is **deleted, not weakened**. All eleven of its literals moved to
    `console/scripts/fields_test.py`, and the screens that carry them are rendered
    against fixtures by `console/src/app/screens.test.tsx`.
  - `console/scripts/route_manifest_test.py` is the new load-bearing one: it
    asserts the console calls exactly the contract's routes, **and** that no RPC
    goes unread by any screen. That second direction is the old rendered-test
    defect one level up.
  - ⚠ THOSE FIVE CHECKS RUN IN `console.yml`, NOT UNDER BAZEL. They were
    `sh_test`s and went red twice on Bazel wiring rather than on anything they
    check — a package-relative path that does not survive the runfiles root, then
    a label its own `glob` already matched. Neither was reproducible in an
    environment without Bazel, which is most of them. `console/BUILD.bazel` is
    now one `exports_files`, and it is load-bearing: `//proto:mirrors_test` reads
    the wire types through it.
  ⚠ The earlier version of this entry also said "`//web:console` and
  `//web:console_html` are different targets" — there was no `//web:console`
  target and there never was. Two wrong entries in one bullet; the lesson is the
  one AGENTS.md already states, that a comment nothing tests will drift.
- ⛔ **`replace_sections` LIFTS ONLY `rule` AND `template`, so a fifth `RuleSet`
  field is silently dropped at approval.** It round-trips the previous
  configuration through a generic `toml::Table` and replaces those two keys —
  which is what stops an approval deleting the templates beside it, and which
  means a `[tolerance]` in a proposal would serialize, be discarded, and approve
  cleanly having changed nothing. That is the same silent drop the function was
  written to fix, one section along. `ratio approve` now REFUSES a proposal
  declaring one; any future top-level key faces the same choice, and choosing
  "merge it" means adding an arm rather than assuming one is there.
- ⚠ **`ratio config set` does not parse what it promotes**, and the seed scripts
  use it for the opening configuration. A malformed `[tolerance]` is therefore
  caught at the first READ — `get_fund`, `list_breaks` — rather than at the
  promotion that introduced it.
- ⛔ **`route_manifest_test.py` MAKES ANY NEW CONSOLE RPC DEMAND A SCREEN.**
  Check 1 is bidirectional between the contract's http rules and `client.ts`,
  and check 3 refuses an RPC no screen reads. So adding an
  `AcceptBreakExplanation` "just for the API" forces the write screen the fence
  forbids — which is why acceptance is a CLI verb. The mechanism protecting the
  buttonless console is that test, not discipline.
- ⚠ **`ratio strike` STILL TAKES ITS VALUATION POINT FROM `SystemTime::now()`,
  EVEN WITH `--as-of`.** The flag is only a gate parameter; a strike labelled
  as-of a past day is stamped today. Surfaced while building the gate and
  deliberately not fixed there — it changes the strike id, the `NAVS` ledger's
  meaning and `one_answer_per_day`'s subject, and belongs in its own commit.
- ⭐ **CreateBook seeds ingest templates per kind** — `bank-statement`
  (Personal: bank/card CSV → cash and expense claims) **and** `loan-payment`
  (Personal: principal + interest columns → two balanced rules merged into
  one conserved entry), `project-invoices` (Project: vendor invoice/cost CSV
  → costs and payables), and on Investment both `custodian-positions`
  (holdings snapshot, recorded and never posted) and `prime_equity_trades`
  (the same trade column contract the demo delivers: `B/S` →
  `equity_purchase` / `disposal_proceeds`, amount `consideration`, dated by
  the trade date). The live list is the book's configuration, so a household
  book cannot be asked to pick a fund feed. `console/src/lib/templates.ts`
  `templatesForKind` is the fixture-side filter; `//crates/ratio-console:ratio-console_test`
  holds the seed and the closed loop: CreateBook → entity master → ingest →
  admit, journal only the admitted trades, VWRL left pending the same way
  `LEAVE_ONE_PENDING` does. ⛔ The demo script still posts recon history so
  the blocked-NAV story has a break; that is a different book. A blank
  CreateBook investment book does not invent those rows.
  ⚠ `[personal.loan]` is **not** seeded. CreateBook writes the posting
  pattern (mortgage/auto/student interest and principal against cash) and
  leaves the schedule table absent — a new household has no named loan, and
  `/books/{id}/views/{view}/loans` says so rather than rolling zeros. Name a
  liability with `ratio config set` (`41 = 12` pairs mortgage with mortgage
  interest). Grain is the liability dimension, not a single debt bucket.
- ⚠ **A household loan-payment walk-through (#87 / #27).** It can show a
  conserved payment (interest expense + principal reduction against cash) and
  two named loans that do not collapse into one debt bucket. It cannot show
  loan origination as a product, refinance shopping, credit scores, bank OAuth,
  or a client portal — those stay refused. Envelope budgeting is #83.
- ⚠ **`console/scripts/capture_fixtures.sh` takes `navStrikes.json` and
  `replay.json` from `RATIO_FIXTURE_STRUCK_FUND`, not from `RATIO_FIXTURE_FUND`.**
  The fixture fund is the BLOCKED book on purpose, and a blocked book now has no
  strikes, so `id navStrikes.json` would index an empty list.
- ⛔ **THE COMMITTED FIXTURES CORRESPOND TO NO SINGLE SEEDED FUND, so a
  wholesale re-capture is not a refresh — it is a rewrite that breaks nine
  render tests.** `capture_fixtures.sh` pulls every file from one `$FUND`, but
  `screens.test.tsx` asserts two currencies, forty open lots, a realized gain
  and a declared lot method — `ashcombe`-shaped data — while the default
  fixture fund is the blocked reconciliation book, which has none of it. Tried
  it; reverted it. ⚠ So `fixtures_test.py` passing means the FIELD SETS match
  the contract and nothing more, and the values are a hand-maintained
  composite. Refreshing them honestly means either capturing per-fixture from
  the fund that can produce it, or accepting new expected values in the render
  tests. Either is its own change.
- ⚠ **`vi.mock`'s factory SPREADS the `wire` object, so reassigning
  `wire.getBreak` inside a test does nothing.** The mock captured the function
  values when it ran. A case that needs different data needs a different
  fixture, not a mutated stub — one afternoon went into a render test that was
  passing the same fixture back to itself.
- ⚠ **`//proto:ratio_aip_lint` bans prepositions in field names** (only
  `core::0192::has-comments` is disabled). `explained_at_position` fails;
  `journal_position` does not. Verified by adding one and watching it go red.
- ⛔ **`append` and `append_all` are two doors with the same law**, with different
  indentation. A fix applied by string replace hits one.
- ⛔ **Python edits collapse Rust `\` line continuations** into runs of spaces
  inside string literals. Three occurrences so far; sweep with
  `grep -rnE '"[^"]*[a-z,] {8,}[a-z]'` and expect column headers as false hits.
- ⚠ **`decide` cannot reduce `List.mergeSort`** (well-founded recursion). Use a
  structural insertion sort; `native_decide` is forbidden by
  `//lean:audit_proofs_test` and would trade a checked theorem for a trusted
  compiler.
- ⛔ **zsh does not word-split `$var`, AND IT HAS COST THREE MISTAKES IN ONE
  DAY.** `R="bazel run -- "; $R foo` fails; `for p in "500 500"; do set -- $p`
  passes one argument, not two; and `for cmd in "strike --as-of X"; do ratio
  $cmd` runs an unknown command — which I then TIMED and reported as a
  measurement of `strike`. Use `${v%%:*}`/`${v##*:}` or write the calls out.
- ⛔ **Two tests naming the same book wipe each other's directory.** Every helper
  begins with `remove_dir_all` and tests run in PARALLEL, so a duplicated name
  leaves ACTIVE pointing at a config blob that has just been deleted. It failed
  about one run in three, in whichever test lost the race, reporting a
  stored-config error unrelated to what either was testing. Book names must be
  unique across the file; `tmp_root()` uses `TEST_TMPDIR` so a run cannot inherit
  the previous one's wreckage either.
- ⚠ **`--test_output=errors` hides a passing test's stdout, and a grep for
  `"not found"` will not match `"not in types.ts"`.** Twice I read a test as
  reporting less than it did. `//proto:mirrors_test` prints one line per
  message; read the whole thing before concluding it missed something.
- ⚠ **A number nothing reads is a number nothing checks.** `fx_rate` returned a
  rate two orders of magnitude wrong for months. Its only consumer multiplied by
  it and discarded the result to time the multiplication.

---

## What the demo shows, and what it does not

```
lots/sec  open lots     entries   COLD BUILD   NAV STRIKE   PEAK FOOTPRINT
     500     252843     1769907      11.4 s       403 µs         —   ← recorded
     500     252843     1769907      12.5 s       385 µs     36 MB   ← now
    2000    1022625     7158381      91.7 s       395 µs         —   ← recorded
    2000    1022625     7158381      50.2 s       418 µs        51*  ← now
    2000   20004324   140030274     995.0 s        12 µs   1.00 GB   ← ⭐ #6, MEASURED
```

⭐ **TWENTY MILLION TAX LOTS, STRUCK IN 12 µs, IN A GIGABYTE.** Issue #6 asked
for this to stop being extrapolated. 140,030,274 entries, 20,004,324 open lots,
trial balance 0. The fold takes 16.6 minutes and the strike off it is twelve
microseconds — `Ratio.Closure.factored_nav_never_reads_the_lots`, at the size the
claim was always about.

    parse    655.2 s   reading and deserializing
    fold     339.7 s     of which relieve  5.0 s  over 60,012,972 reliefs
    mark      10.8 ms  10000 prices          fx  210 µs   2 rates, not 10000

⛔ **QUOTE `peak memory footprint`, NOT `maximum resident set size`.** macOS RSS
EXCLUDES COMPRESSED PAGES, and at this size it reports **52 MB** for a process
whose real footprint is **1.00 GB** — a nineteen-fold understatement of the
number that decides whether a book can be folded at all. Every memory figure in
this file was taken with `/usr/bin/time -l`; the ones marked `*` above are RSS
and are not corrected. The lot data is written once and then cold, which is
exactly what the OS compresses.

⛔ **AND THE 12 µs IS WHY THE NEXT DEFECT WAS FOUND: THE RECORDED NAV DID NOT
TRANSLATE CURRENCIES.** Disbelief in that number is what prompted checking it,
and the check found something worse than a bad measurement. `ratio strike` — the
*recorded* NAV, the one signed, digested, and re-derived by `ratio replay` — summed
dollars, euros and pounds and labeled the total USD. On a twelve-security book it
returned the **identical** figure for `--currencies 1` and `--currencies 3`,
because it never read `PostingRecord::currency` at all.

    flat sum (the bug)           133,915,377.28
    ratio strike (fixed)         134,439,187.51
    console GetFund              134,439,187.51
    recomputed from the raw files 134,439,187.51

**$523,810.23 on a $134M fund — 0.39%.** Small enough to read as a rounding
difference, large enough to be the whole fee dispute. And it tied the entire way:
trial balance 0, digest reproducible, `ratio replay` reporting *reproduced* — of
the wrong figure, permanently. `Ratio.Chart.Dimensions.a_flat_total_hides_a_
currency_mismatch` had proved this exact shape impossible-to-notice since the
chart work landed; the proof was right and three Rust call sites ignored it.

⚠ **THE TEST THAT SHOULD HAVE CAUGHT IT ALREADY EXISTED AND WAS VACUOUS.**
`the_projection_strikes_the_same_nav_as_a_full_fold` compares the two NAV paths —
over a one-currency book with `Rates::none()`. Its multi-currency replacement was
*also* vacuous on the first attempt: buying securities with cash puts both legs
in assets, so every currency nets to zero and both paths say 0. It went green
against the bug on purpose-reintroduced code. **Subscriptions are the shape that
works** — capital is equity, the NAV filter excludes it, and the asset side is
left holding a non-base balance. ⛔ Negative-test every differential test; two
paths agreeing is worth nothing until you have watched them disagree.

⚠ **`parse` IS 66% OF THE COLD BUILD AND INTERNING DOES NOT TOUCH IT.** Interning
the config digest — 64 identical bytes on every one of 140 million lines, 22% of
the file — measured 8.5/8.6/8.6 s against 8.2–8.6 s before it. A null result. The
cost is serde tokenizing half a gigabyte of JSON, not allocating. The levers that
remain are a faster parser (`simd-json`) or a denser format, and the format is a
product decision: the journal is the system of record and its readability is part
of what is being sold.

`ratio bench` generates a fund and measures a period end. 100× the lots, NAV
strike unchanged.

⛔ **It reports two curves and both must be quoted.** Folding the journal grows;
only the strike off a maintained projection is flat.

⛔ **AND THERE IS A THIRD CURVE, LARGER THAN EITHER, THAT NOTHING TIMED UNTIL
NOW: GENERATING THE BOOK.** Measured on one Linux machine, both shapes:

| shape | generate | COLD BUILD | ratio |
|---|---|---|---|
| 500 × 500 | 28.6 s | 24.0 s | 1.19 |
| 500 × 2000 | 115.8 s | 94.7 s | 1.22 |

**Building the fund costs more than the cold build it exists to feed**, and
consistently so. Everything about this command's shape implies the two curves it
argues about are the expensive ones; they are downstream of a third that is
bigger than both. At the twenty-million-lot shape that is roughly twenty minutes
of generation before a fold that takes sixteen — so **a book that size is built
once and kept, never regenerated per run**, and any plan that assumes otherwise
has budgeted for a third of the work. `--json` now carries `generate_ns` (null,
never 0, when folding) so this cannot go untimed again.

⛔ **TWO BENCHMARKS AT ONCE DELETED EACH OTHER'S BOOK, AND THE WRONG ANSWER
TIED.** `bench` generated into `$TMPDIR/ratio-bench-book` — one fixed name for
every run — and `ratio_gen::generate` opens with `remove_dir_all`. A second run
starting while a first was folding wiped the directory under it. A 500 × 2000
run measured:

    alone      1,022,625 open lots   94.7 s   trial balance 0
    concurrent   224,852 open lots   20.7 s   trial balance 0

**22% of the lots in 22% of the time**, entry count correct, nothing on stderr.
It does not read as a broken run; it reads as a fund with less fragmentation than
expected, which is a thing funds are. This file already named the trap for TESTS
— "two tests naming the same book wipe each other's directory" — and it was in
the benchmark the whole time, which is the one place a number gets published.
The directory now carries the pid, and `//demo:bench_concurrent_test` fails if it
stops doing so.

⚠ **THE 20M ROW IS 10,000 SECURITIES × 2,000 LOTS, NOT 500 × 40,000.** Both are
twenty million open tax lots; they are not the same fund. The mark phase reads
one price per SECURITY (`Ratio.Closure.markCost`), so the recorded shape marks
ten thousand names and `ratio closure`'s default dial marks five hundred — a
twentyfold difference in the term that grows with the chart. Estimating one and
running the other, both captioned "twenty million tax lots", gives two figures
that each tie and describe different books. `//:scale_shapes_test` holds the
shapes the scale screen offers to the rows in this table.

⛔ **AND THE REAL LIMIT WAS NEVER TIME, IT WAS MEMORY.** Every fold in this
codebase materialized what it folded: 1.85 GB resident to fold 1.77M entries into
a projection holding 8 MB of lots, and `ratio balance` — which prints a dozen
rows — held 1.26 GB. At the ~80M entries a twenty-million-lot book implies, the
`Vec<JournalEntry>` alone is about **eighty gigabytes**.

| | before | after |
|---|---|---|
| `ratio balance` | 1.26 GB | **9 MB** |
| `ratio bench` (generate AND fold) | 1.85 GB | **50 MB** |
| console serving the book | — | 39 MB |

⛔ **AND EVERY MEMORY FIGURE ABOVE IS A ONE-VIEW FIGURE.** Multi-view books
landed after these were taken. Each view carries its own lot book — a settlement
view has recognised a different set of open lots when a sale arrives, which is
the feature working — so the projection is roughly linear in views where the
fold is not. At the twenty-million-lot shape that is ~640 MB per view against a
1.00 GB peak, and NOBODY HAS MEASURED TWO. `ratio bench` needs a views dial
before the ⭐ claim below is quoted about a fund keeping an ABOR and an IBOR.
Quote `peak memory footprint`, not RSS: at that size RSS understates by 19×.

`Journal::for_each_entry_since` is the streaming primitive. `entries()` and
`entries_since()` still exist, expressed on top of it and documented as
materializing — reach for them only on a book you know is small.

⛔ **SO #6 WAS PLANNED AGAINST THE WRONG CONSTRAINT ENTIRELY.** "~25 minutes of
generation, worth doing once" is a TIME estimate for something that would have
died on memory, and no amount of extrapolating the time curve would have shown
it. Re-plan it: the fold is now bounded by the projection (open lots), which at
20M lots is ~640 MB.

⚠ **TWO SITES ARE STILL O(entries) AND SAY SO IN THE CODE.** The console's
`posted` set and the ingest duplicate check build a set of EVERY id in the
journal. Streaming removed the entry bodies, not the set; the real fix is an
index rather than a scan.

⭐ **AND THE FIRST CURVE IS NOW ACTUALLY O(entries), WHICH IT SAID FOR MONTHS
AND WAS NOT.** A relief used to copy and sort the whole holding on every sale,
so the build scaled with FRAGMENTATION as well as with entries. Holding entries
constant at ~1.8M and raising lots-per-position 4x:

| lots/position | parse | relieve | COLD BUILD |
|---|---|---|---|
| 500 → 2000, before | 8.2 → 8.5 s | 7.4 → **31.1 s** | 20.2 → **44.5 s** |
| 500 → 2000, after | 8.2 → 8.6 s | 60.3 → **59.3 ms** | 12.6 → **12.8 s** |

`relief::Holding` keeps the holding in the order its method gives lots up in, so
a relief is a POP rather than a re-sort. `parse` is now 65% of the cold build and
is the floor.

⛔ **THE LESSON IS THE LABEL, NOT THE FIX.** `O(journal)` was written down, was
wrong, and nothing checked it — the same shape as every other defect here. What
found it was holding one dial fixed and moving another, which is the cheapest
experiment available and was never run.

⚠ The generator sold at cost until the gain posting landed, so every disposal
realized nothing and the gain account never moved — six lot methods and the whole
relief layer running against a book where every gain was zero.
⚠ **It happened again, twice.** Trades carried no `trade_date`, so every lot had
no acquisition date and the entire short/long split fell into `unclassified`;
and every posting carried `currency: None`, so a book with three currencies in
its entity master formed exactly one conservation group. Both features were
built, shipped, and exercised by nothing. **If you add a scenario, check it
exercises what you think it does** — and prefer a test that asserts BOTH sides
appear, because a book that is entirely long-term tests the threshold no better
than one that is entirely unclassified.

---

## Where things are

| | |
|---|---|
| `lean/Ratio/` | the proofs. `Bounded`, `Chart/Dimensions`, `Lots/{Relief,Methods,Edges,Posting}`, `Actions/Factor`, `Closure`, `Exec` |
| `crates/ratio-rules` | `RuleSet`: `lot_method`, `chart_roles`, `long_term_days`, `tolerance` — the administration agreement, as configuration |
| `lean/Ratio/Views.lean` | what a view IS: a recognition predicate. Every view conserves; two differ by exactly what is in flight; a fold with no CUT hides the difference entirely |
| `tla/Views.tla` | where the views ARE when somebody asks. One prefix, one pass, and the calendar inside the pinned config |
| `tla/` | `Projection`, `Executor`, `ReliefEngine`, `LotEngine`, `Actions`, `Valuation`, `ControlPlane`. Each has `manual`-tagged probes that must go RED |
| `crates/ratio-project` | the read model, the lot book, the relief engine — one pass, N view folds, each with a monotonic cut on the journal's own clock and a band bounded by the settlement lag. ⚠ every memory figure in this file is a ONE-VIEW figure; each view carries its own lot book |
| `crates/ratio-gen` + `ratio bench` | the generated fund and the measurement |
| `crates/ratio-console` | the console's BFF — 40 RPCs, transcoded onto `/v1` |
| `crates/ratio-nav/src/explain.rs` | what a strike DOES, as a plan. ⛔ a description of two code paths, not a planner over them — nothing chooses |
| `console/` | the console itself. Next.js on Vercel; ⛔ Bazel does not build it |
| `tomato-bazel/rules_postgres` | `Pg.Rel.Semantics` — merged, PR #9 |
| `AGENTS.md` | the rules, for a person or a model. Replaces the two stale LLM guides |

⚠ Every `tla_check` tagged `manual` is a probe that must FAIL. Run them after
changing a spec; a probe that goes green means the invariant stopped checking.

⛔ **AND A PROBE THAT GOES RED FOR THE WRONG REASON LOOKS IDENTICAL TO ONE DOING
ITS JOB.** Adding a `CONSTANT` to `ReliefEngine.tla` made a neighbouring probe
die with "the constant parameter MaxConfigs is not assigned a value" — still red,
still passing as a probe, no longer reaching the model at all.

    tla/probes.sh          runs all 13 and matches TLC's reason against the
                           invariant each cfg claims. NOT a bazel test: each is
                           a full model check, and 13 more on every commit is
                           how a suite stops being run.
    //tla:probes_test      the static half, and it DOES run in CI — every
                           CONSTANT assigned, the claimed invariant named in the
                           house form, listed so TLC is asked, and spelled the
                           way the spec spells it.

It found three probes that could not say what they claimed on its first run,
including one naming an invariant that does not exist.
