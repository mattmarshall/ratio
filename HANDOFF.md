# Handoff — tax lots, corporate actions, and the dimensional chart

**State**: bazel tests green, 32 `lean_test`, 49 `tla_check`, 30 `manual`
probes all red for the reasons they name.

Issues #4, #5, #7, and #9 are closed. #5's leftover was the console
wash-flag cite; that cite landed. #9's leftovers were the MinTax /
SpecID / average-cost engines, their console cites, and the pooled
holding-period category rule; those landed. #153 landed the
lots/positions projection schema and digest replay; #8 and #159
stay open (live engine, planner pushdown, measured 20M lots).
Open work is #6, #8. This
file is the part that does not fit in an issue: what was learned, what
is load-bearing, and what will bite. Wash sales have a Lean/TLA model
and a Rust window (`RuleSet.wash_window_days`). `WashRestatement` is a
citeable record (`Ratio.Lots.WashRestatement`; a restatement cites the
strike, it does not rewrite it). The non-US holding-period variant is
an election (`wash_keep_holding_period`; unset stays unset, not a
silent keep; the US `replacementAcquired` transfer stays in force
until somebody writes the keep). MinTax, SpecID, and average cost
each have a Lean surface, a TLA probe that fails if they are treated
as a Method, and a Rust election that is not a `LotMethod` variant.
The console cites those elections (lot-terms for
`min_tax_short_weight` and `average_cost`; journal entry for
`identified_lots`; unset stays unset). The pooled holding-period
category is a date (`Ratio.Lots.PoolPeriod`; a shared acquisition
date is carried when every lot agrees; mixed or missing dates stay
unset; no category is invented — not FIFO's oldest date, not two
Form 8949 boxes). The household tax-pack Connect app
(`connect/tax-pack/`, #166) cites that same rule; it does not
close #166 (WorkOS dashboard registration leftover #22 and IRS e-file remain). The Project AIA
pay-app Connect app (`connect/aia-pay-app/`, #184) refuses to
invent billed / retainage / change-order zeros; that does not
close #184 (WorkOS dashboard registration leftover #22 and a licensed AIA form remain).
The Project EAC / forecast Connect app (`connect/eac-forecast/`,
#169) computes estimate-at-completion outside the journal; unset
remaining-to-spend stays revised − incurred − awarded, never a
silent EAC of 0. That does not close #169 (WorkOS dashboard registration leftover
#22 and live EAC product UX remain). The Project program roll-up
Connect app (`connect/program-rollup/`, #179) cites per-book
budget and billing figures across PROJECT books the subject can
see (`books:read` membership; an `org_id` claim is not membership)
and sums only the books that cited each figure — never a silent
program billed or collected of 0.00. That does not close #179
(WorkOS dashboard registration leftover #22 remains).
Personal cash-forecast Connect predictors
(`connect/bank-balance-predictor/` and `connect/calendar-bills/`,
#163) post allowlisted `forecast_*` / `scheduled_*` journal
material and read statements; they do not invent envelopes or
payroll, and they do not grow a kernel RPC. That does not close
#163 (WorkOS dashboard registration leftover #22 and live bank / calendar OAuth
remain). It does not reopen #164.
The audit-export Connect app (`connect/audit-export/`, #185)
packages cited period closes, NAV strikes, break reports /
explanations, and config / journal digests as a ZIP; a missing
cite stays unset in the manifest, never a silent empty file or
empty-digest-as-success. first-party Connect apps call ConnectApiUrl.
That does not close #185 (WorkOS dashboard registration leftover
#22 and a live walk-through ZIP remain).
Equalization, drip, and side-pocket stay Connect (#177) — a
PLAN decision, not a landing. None of the three changes
conservation or journal integrity; do not implement them as
kernel primitives, `screensFor` forks, or `Method` / `Order` /
`lot_method` variants. Drip elections stay on #161. Equalization
and side-pocket first-party apps are not filed. This file
closes the #177 decision. It does not close #161 or #150.
This file closes #9. It does not reopen #5 or #151.
Multi-view FX / translation refuses stay citeable (#160): a
translation residue, an entry only one view can place, a missing
rate, or a view declared after the fold refuses on `ratio
reconcile`, ReconcileViews, and the reconcile screen — never a
silent 0.00 that looks like agreement. An entry neither view can
place cites `why` with an unset effect. FX rate vendors stay
Connect. This file closes #160. It does not close #159.
The ConfigStore / fact-plane seam (#158): control plane is
`ConfigStore` (directory, SHA-256, atomic pointer); fact plane is
`FactStore` (append-only `facts.jsonl`, provenance required, same
id refused). A parallel mutable shadow book is refused on
`--post` — no `shadow/` sibling journal, no recon history beside
ingested facts. `compare_configs` answers "what moved?" in
memory. geetch / crova stay later. This file closes #158. It
does not close #159. It does not reopen #155 or #160.
Personal books declare currencies (#178): `[personal] currencies`
is the election, first code is the reporting base, empty is unset
— not a silent USD on ListBooks / GetBook. ApplyEvent stamps a
named declared code; the journal door still refuses unbalanced
FX; a missing rate reuses the #160 `Rates` refuse. FX rate
providers stay Connect. This file does not close #178 (live rate
Connect apps and a `/trade` picker remain). Optional lot relief
on household Investments (#187): `[personal] lot_relief = true`
elects the wash / MinTax / SpecID / average-cost engines already
on the book; unset stays unset. This file closes #187. It does
not start #163 or #164.
NAV gate refuse reasons stay citeable (#188): unexplained break,
unpriced, and unresolved trade are first-class copy on GetFund /
GetView (`nav_gate`, the same `blocking_at` fold the badge already
reads) and on the fund overview / view chrome — never a bare HTTP
400. Unpriced stays empty on that field unless a valuation date
was named; the mark ticket cites unpriced when a date is. No new
gate semantics. This file closes #188. It does not close #26.
#159 stays blocked.
Kind-aware IA (#175): `screensFor` already hid Exceptions /
Positions / NAV on Personal / Project / Operating; a typed URL,
legacy `/breaks` redirect, or palette "Open by id" row still
rendered them. `wearsFundOps` 404s those pages and drops the
fund-ops deep-links. Investment keeps the warehouse. One
`screensFor` list. GetBook for every fund in `funds.json` must
be INVESTMENT in `books.json` — the phone pass opens northstar
strikes through `requireFundOps`, and falling back to the
household fixture 404s those pages. Leftover typed `/wip` /
`/billing` on Personal and typed `/sheet` / `/pnl` on Investment
stay on #26. This file closes #175. It does not close #26.
#159 stays blocked.
Point-in-time / restatement reporting stays citeable (#186):
console `/asof` browses a pinned prefix + config digest from a
close or a strike, and WashRestatement as a citeable record
(cites the strike, does not rewrite `net_asset_value`). `now`
leaves digest and config unset — the head is not a pin. Empty
is unset, not a silent 0.00. This file closes #186. It does
not close #26.
The lots/positions projection (#153): `crates/ratio-sql-project`
replays a journal prefix into `lots` / `positions` / `aggregates`
under one watermark (`ratio_nav::prefix_digest`). A stale
watermark refuses. Relief loads the rows and calls
`relieve_by` — physical seq order is not FIFO. The journal
stays the system of record. This file closes #153. It does
not close #8 or #159. A live Postgres engine, planner
pushdown, and the measured 20M-lot claim stay on those
cards. leftover #22 stays on WorkOS.
The demo API journal-env unset (#230) stays; this file
does not reopen S3 hydrate on `ratio-demo`.

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

⛔ **Equalization, drip, and side-pocket stay Connect (#177).** None
changes `Conserves`. A late-subscriber credit is a valuation of NAV
and the named cut; a drip is `distribute_*` then `subscribe_*`; a
side pocket is a share-class / instrument partition plus a named
cut. Inventing `equalize` / `drip` / `SidePocket` as a journal
primitive, a `screensFor` fork, or a `Method` / `Order` /
`lot_method` variant is the coverage-creep this repository already
named. A silent 1/N of pocket NAV is the defect #180 already
refused. Drip elections stay on #161. Do not mint `equalization:*`
or `sidepocket:*`.

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
- **MinTax is not a `Method`.** `Ratio.Lots.MinTax` ranks at a sale PRICE;
  `lot_method = "min_tax"` stays refused. The election is
  `min_tax_short_weight: Option<i64>` — `None` is unset, not a silent 2 —
  and it cannot share a configuration with `lot_method`.
  `//tla:sort_and_walk_mintax_check` is the engine that pretends otherwise.
- **SpecID is not a `Method`.** `Ratio.Lots.SpecId` takes the lots the
  taxpayer names on the sale; `lot_method = "specific_id"` stays refused.
  The election is `JournalEntry.identified_lots: Option<Vec<u64>>` —
  `None` is unset, `Some([])` is elected and unnamed and refuses rather
  than walking FIFO. `//tla:sort_and_walk_specid_check` is the engine
  that pretends otherwise.
- **Average cost is not a `Method`.** `Ratio.Lots.AverageCost` pools the
  holding; `lot_method = "average_cost"` stays refused. The election is
  `average_cost: Option<bool>` — `None` is unset, not a silent true, and
  `Some(false)` is refused at read. It cannot share a configuration with
  `lot_method` or `min_tax_short_weight`.
  `//tla:sort_and_walk_average_cost_check` is the engine that pretends
  otherwise. 10 / 40 / 70 pools to 40 (a lot's own basis); the
  load-bearing holding is 10 / 20 / 60, which pools to 30.
- **The pooled holding-period category is not a `Method`.**
  `Ratio.Lots.PoolPeriod` carries a shared acquisition date when
  every lot agrees; mixed or missing dates stay unset. US
  single-category would take FIFO's oldest date and invent
  long-term; double-category would invent two pools. No
  `lot_method` variant and no RuleSet election — unset is not a
  silent long and not a silent short. Day 0 / day 400 / dispose
  400 / threshold 365: FIFO is long, the other lot is short, the
  pool is neither. `//tla:sort_and_walk_pool_period_check` is the
  engine that pretends the rule is an Order.
- **The non-US wash holding period is not a `Method`.**
  `Ratio.Lots.WashHolding` elects whether a replacement keeps its own
  acquisition date; `lot_method = "wash"` stays refused. The election
  is `wash_keep_holding_period: Option<bool>` — `None` is unset, not
  a silent keep, and `Some(false)` is refused at read. Keep without a
  wash window refuses. Assuming `replacementAcquired` everywhere
  flips a later disposal between tax rates while the books still
  tie. `//tla:universal_us_transfer_check` is the engine that
  pretends the US transfer is universal.
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
- **Commitment and undrawn are equity, so they cancel in the NAV filter.**
  Putting undrawn on the asset side would make an unfunded commitment look
  like cash that had arrived. `call_lp` is the four-leg fact that moves
  cash; `commit_lp` does not. Remaining undrawn is unset until
  `postingCount` is not `"0"` — a silent 0.00 is the defect.
- **`FUND_CURRENCY` is one constant** because it is two answers to one question
  otherwise: what `Fund.currency_code` labels the figures, and the base `Rates`
  translates into. A NAV translated into euros and labelled USD is wrong by the
  rate and looks entirely ordinary. ⛔ **A Personal book does not inherit
  it.** `[personal] currencies` is the election; first code is the
  reporting base; empty is unset, not a silent USD on ListBooks /
  GetBook. A missing rate still refuses through the same `Rates` door.
  FX rate providers stay Connect.
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
  Chrome cites the same fold as `Fund.nav_gate` / `View.nav_gate` — unexplained
  break, unpriced, unresolved trade — via `withRefusal` / `NavGateCite`, not a
  bare HTTP 400. Unpriced stays empty on that field unless a valuation date
  was named. `the_nav_gate_on_the_fund_is_the_same_fold_as_blocking_at`
  holds the wire to the fold. This closes #188. It does not close #26.
  `/asof` (#186) cites a pinned prefix + digest on a close or a
  strike, and WashRestatement when a later wash moved the realized
  gain — the struck figure is not rewritten. Unset stays unset.
  This closes #186. It does not close #26.
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
- ⭐ **THE PER-VIEW FOLD LANDED, AND THE REMAINING REFUSES ARE THE CITE.**
  The ten view-scoped screens and `ReconcileViews` answer per view. What
  still refuses — and must stay a sentence, never a silent 0.00 — is a
  translation residue (integer translation does not distribute over a
  sum), an entry only one view can place, a missing FX rate, or a view
  declared after the fold read past its history. `ratio reconcile` and
  the reconcile screen cite those sentences via `withRefusal`. An entry
  neither view can place is the third list: `why` is set, the effect is
  empty, and printing `"0"` looks like agreement.
  ⚠ `console/fixtures/reconcile.json` IS A HAPPY-PATH CAPTURE. The
  residue / missing-rate / cannot-place-in-one refuses are asserted in
  `ratio-project` and `ratio-console` tests and rendered as a `Refused`
  sentence, not as a fixture that pretends they returned a difference of
  zero.
- ⛔ **A DIFFERENTIAL TEST BETWEEN TWO VIEWS THAT DOES NOT CUT IS VACUOUS.**
  Folded to the end of history every view agrees, because everything eventually
  settles — `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_gap` is the
  theorem, written down before anybody fell into it. And the second trap is
  sharper: a PURCHASE moves cash into investments, both assets, so recognising
  it or not moves a NAV by ZERO. **Subscriptions are the shape that works**,
  exactly as they were for the multi-currency version that was vacuous twice.
- ⚠ **WHAT `//deploy:seed_test` ACTUALLY PROVES, WHICH IS LESS THAN IT LOOKS.**
  It checks the two views' recorded NAVs DIFFER and that the book ties. The
  stronger claim — `Ratio.Views.two_views_differ_by_exactly_what_is_in_flight`
  — is proved in Lean, and `ratio reconcile` / `ReconcileViews` now walk the
  list (and refuse when the list cannot sum to the NAV difference). The demo
  seed still does not assert that walk. ⛔ The gap is written into the script
  rather than left to be inferred from its absence.
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
- ⚠ **`capital_txns` IS COUNTED WHEN THE CHART IS IN HAND, AND BLANK WHEN IT
  IS NOT.** Projection still does not know the chart, so `shape_of` without a
  count passes zero to the model and the capital node stays blank — those two
  remain different claims. Console `ExplainNavStrike` counts journal entries
  that posted a quantity onto a capital account (`count_unit_capital_entries`)
  and passes `Some(n)`. A contribution without units does not count. When
  counted, the node shows `n` and `estimated_reads` includes it, so the steps
  still add to the figure beside them. ⛔ A silent rendered zero on the
  uncounted path is still the defect.
- ⛔ **`explain.json` IS THE ONE FIXTURE IN `console/fixtures/` THAT WAS NEVER
  CAPTURED.** Bazel could not fetch two `tomato-bazel` modules in the
  environment it was written in (403 under an egress policy), so no `ratio
  watch` could be built to capture from. The steps, citations, notes and every
  modelled figure came out of `ratio_nav::explain::plan_of` itself and the
  encoding was copied from `transcode.rs` — but the FUND'S SHAPE is chosen to
  agree with `view.json` rather than read off a book. `capture_fixtures.sh` says
  so at the top and now captures it; run it and commit whatever comes back.
  ⚠ A fixture that outlives the gap it was written for is how a captured
  fixture stops being a capture. `reconcile.json` is no longer that shape —
  the endpoint serves it — but `explain.json`'s fund shape is still chosen
  to agree with `view.json` rather than read off a book.

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
  one conserved entry),   `brokerage-statement` (Personal: the same
  custodian / broker CSV the fund trade loop reads → household
  transfers onto Investments, not lot-opening purchases, unless
  `[personal] lot_relief = true` remaps the admit path) **and**
  `brokerage-positions` (Personal: holdings snapshot, recorded and
  never posted; live recon reuses the fund refuse paths),   `project-invoices` (Project: job-cost / AP / progress-bill CSV
  → `project_cost*` / `vendor_invoice*` / `progress_bill` / `pay_vendor` /
  `earn_progress`; an unidentified vendor pends; `hold_retainage` and
  `capitalize_wip` in Kind are refused — ingest does not invent retainage
  or WIP), `change-orders` (Project: Kind `approve_co_site` /
  `deduct_co_site` / … → the work-package equity pair; no entity master —
  the phase is a chart dim), `purchase-orders` (Project: Kind
  `award_commitment_site` / `release_commitment_site` / … → the awarded-
  commitment equity pair on the same grain), and on Investment `custodian-positions`
  (holdings snapshot, recorded and never posted), `prime_equity_trades`
  (the same trade column contract the demo delivers: `B/S` →
  `equity_purchase` / `disposal_proceeds`, amount `consideration`, dated by
  the trade date), and `capital-calls` (Kind `commit_lp` / `call_lp` /
  `commit_gp` / `call_gp` → the partner-scoped commitment and call rules;
  no entity master — the partner is a chart dim), and on Operating
  `customer-invoices` (Kind `invoice` / `collect` → receivable against
  operating revenue, then cash against AR) and `vendor-bills` (Kind `bill`
  / `pay` → expense against AP, then AP against cash). The live list is the book's
  configuration, so a household book cannot be asked to pick a fund feed.
  `console/src/lib/templates.ts` `templatesForKind` is the fixture-side filter;
  `//crates/ratio-console:ratio-console_test` holds the seed and the closed
  loop: CreateBook → entity master → ingest → admit, journal only the
  admitted trades, VWRL left pending the same way `LEAVE_ONE_PENDING` does;
  a Project book runs the same loop on `project-invoices` (identified AP /
  progress-bill / cost post, UNKNOWN SUB pending, retainage and WIP kinds
  refused);   a Personal book runs it on `brokerage-statement` /
  `brokerage-positions` (identified buys post as transfers unless
  `[personal] lot_relief` is elected, VWRL pending,
  unidentified or foreign-currency holdings refuse the live recon).
  ⭐ **The live custodian loop (#155) is that path plus recon.** Ingest
  `prime_equity_trades` and `custodian-positions`, admit, then
  `ratio recon --from-ingest` (or `Console::recon_from_ingest`). The
  holdings snapshot is compared to the journal's Investments carrying
  value — cash is out of the compare, or every purchase is a false
  break. One unidentified holding refuses the whole run and writes no
  report. A difference is the same `BreakReport` the NAV gate already
  reads. `demo/custodian-feed.sh` is the CLI walk-through.
  ⛔ The demo script still posts recon history so the blocked-NAV story has
  a break; that is a different book. A blank CreateBook investment book does
  not invent those rows.
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
- ⚠ **A household net-worth-bridge walk-through (#94 / #27).** It can show
  beginning and ending net worth for a month or year, cite ΔNW against
  period income and expense, and name the balance-sheet plugs the P&L
  ignores (principal paid on a named loan, transfers on Investments /
  Credit cards). An empty journal, or a cut with no dated prefix, stays
  **unset** — not a measured $0.00 NW. Origination that nets to zero is a
  real zero. It cannot show a purchase account the chart does not have
  (asset purchases stay unset), a fire number, lifestyle coaching, bank
  OAuth, a credit score, or a client portal. Envelope budgeting is #83;
  loan schedules are #87.
- ⚠ **A household cash-flow walk-through (#98 / #27).** It can show beginning
  and ending cash for a month or year, and classify the movement into
  operating (income, expenses, credit-card working capital), investing
  (Investments — the same account the net-worth bridge names as a transfer),
  and financing (named-loan principal and draws, opening equity). Beginning
  plus classified movement equals ending cash; a move the chart cannot name
  is a residual line an operator can open, not silent absorption. An empty
  journal, or a cut with no dated prefix, stays **unset** — not a measured
  $0.00 cash. Spending down to zero is a real zero. A Personal cash
  forecast (#163) cites scheduled net cash from posted `scheduled_*` /
  `forecast_*` journal kinds on the same `/cashflow` URL
  (`filter=forecast-YYYY[-MM]`); unset when none exist — not a
  measured $0.00. Actuals folds skip those kinds. Connect predictor
  scaffolds (`connect/bank-balance-predictor/`,
  `connect/calendar-bills/`) can show a fixture row mapping onto
  those templates and the closed-through / empty-allowlist /
  `journal:append` refusals. They cannot show a live bank or
  calendar OAuth grant, a Connect token opening a book, envelope
  coaching, payroll, a credit score, or a client portal. Envelope
  budgeting is #164; loan schedules are #87; the net-worth bridge
  is #94. This file does not close #163. It does not reopen #164.
- ⭐ **A live custodian-feed walk-through (#155 / #27) can show, and cannot show.**
  CreateBook(Investment) or `ratio init --kind investment`, add the
  master, ingest `prime_equity_trades` and `custodian-positions`, admit,
  `ratio recon --from-ingest`: the Investments difference has an address,
  `ratio strike` refuses, the trial balance still ties, the journal was
  not rewritten. Unidentified holdings refuse the whole run (exit 3, no
  breaks). It cannot show broker OAuth, a multi-custodian adapter, an
  LP portal, equalization / drip / side-pocket, or a feed at a fund's
  volume. Those stay Connect (#150 / #161 / #177) or the Phase two
  leftover. This file closes #155. It does not reopen #177, #180, or
  #181.
- ⭐ **A household brokerage-statement walk-through (#167 / #27) can show, and cannot show.**
  CreateBook(Personal) or `ratio init --kind personal`, add the
  master, ingest `brokerage-statement` and `brokerage-positions`,
  admit: identified buys land as transfers on Investments, the trial
  balance still ties, the lot book stays empty, an unmatched
  instrument pends. `ratio recon --from-ingest` compares the holdings
  snapshot to the journal's Investments carrying value — the same
  engine as #155, exit 2 on a difference, exit 3 on unidentified or
  foreign-currency holdings, no silent 0. `demo/brokerage-feed.sh` is
  the CLI walk-through. It cannot show broker OAuth, a household NAV,
  a cash forecast, envelope coaching, or household multi-currency.
  Lot opening is the #187 election, not this ingest. Those stay
  Connect (#165 / #150) or their own issues (#163, #164, #178). This
  file closes #167. It does not close #165, #163, #164, #178, #155,
  or #150.
- ⭐ **A household lot-relief walk-through (#187 / #27) can show, and cannot show.**
  CreateBook(Personal) leaves `[personal] lot_relief` unset — a
  brokerage buy is a transfer, the lot book stays empty. Elect
  `lot_relief = true` with `[chart_roles]` (investments = 2, cash =
  1, realized_gain = 31): an instrumented `equity_purchase` opens a
  lot, `equity_disposal` relieves it, and GetFund cites wash /
  min-tax / average-cost the same way the fund path does. SpecID is
  `identified_lots` on the sale. `Some(false)` is refused at read.
  A transfer still does not open a lot. `screensFor(PERSONAL)` is
  unchanged — no Positions / NAV / Exceptions. It cannot show fund
  ABOR chrome on that household, a broker OAuth grant, IRS e-file, a
  household NAV, a cash forecast, or envelope coaching. This file
  closes #187. It does not close #165, #166, #163, #164, #178, or
  #150.
- ⭐ **A capital-call walk-through (#82) can show, and cannot show.**
  CreateBook(Investment) seeds partner-scoped `Commitments — LP/GP` and
  `Undrawn commitments — LP/GP` (equity, so they cancel in the NAV filter)
  and the `commit_*` / `call_*` rules plus the `capital-calls` ingest
  mapping. Record or ingest a `commit_lp` then a `call_lp`: cash and
  partner capital move, remaining undrawn falls, the trial balance still
  ties, and `/capital` cites the remaining figure. A book that has never
  posted a commitment shows **unset**, not a callable zero — `postingCount
  === "0"` is the distinction; a fully-drawn line is a real zero.
  `contribute_lp` is still funded capital without a draw. A call or
  distribution also produces a citeable **notice**
  (`Ratio.Partners.Notice`; digest + the pinned cut + the amounts
  the journal posted) linked from `/capital`. Applying the cut to a
  partner-scoped call invents the other partners — that is the
  waterfall the notice refuses. Empty is unset, not a silent
  notice. ⛔ The walk-through
  cannot show a future call schedule, IRR / TVPI / DPI, a waterfall or
  carried-interest formula, a client portal, or CRM. The seeded demo fund
  has no commitment postings, so its `/capital` must refuse undrawn the
  same way — a silent 0.00 there would be the defect.
  This file closes #157. It does not close #161. It does not reopen
  #82.
- ⚠ **A period NAV roll-forward walk-through (#96 / #27).** It can show
  beginning and ending NAV for a month or year, cite ΔNAV against the
  same contribution / distribution accounts `/capital` already names, and
  income / expense / unrealized when those accounts moved. Commitment and
  undrawn stay equity and cancel — they do not inflate NAV. An empty
  journal, or a cut with no dated prefix, stays **unset** — not a measured
  $0.00 NAV. A commitment-only prefix that nets to zero NAV is a real
  zero. It cannot show IRR, TVPI / DPI, a waterfall, carried interest, a
  client portal, or wash sales. NAV strikes stay on `/strikes`. Remaining
  undrawn stays on `/capital`.
- ⚠ **A per-partner capital-account walk-through (#102 / #27).** `/capital`
  cites beginning → contributions → distributions → allocated income /
  expense / unrealized → ending for each partner, composed from the same
  partner In / Out #70 already names and the Loan-shaped `nav-*` fold
  #96 already uses. Allocated plugs stay **unset** without a named
  `[[partner_cut]]` — an equal split of book NAV or a silent 0.00 share
  is the defect. CreateBook(Investment) and the live demo write
  `[[partner_cut]]` LP 80 / GP 20 so allocated plugs fill when the
  figure divides (`Ratio.Partners.Cut`; `RuleSet.partner_cut`). A
  book that omits the table stays unset. Journal specials fold
  first (`Ratio.Partners.applyFacts`); a remainder uses the cut.
  GetBook cites `allocation_facts`. `allocate_*_lp` posts an exact
  amount into partner capital (already on In / Out). Since
  inception leaves beginning unset; `capital-*` is Activity and cannot
  name a beginning stock. ⛔ The walk-through cannot show IRR, TVPI /
  DPI, a waterfall, carried interest, management-fee billing as a desk
  product, an LP portal, a future call schedule, K-1 packaging,
  equalization, or a side-pocket class. `/strikes` stays ABOR NAV.
  Remaining undrawn stays on #82. Book NAV roll-forward stays on #96.
  This file closes #180. LP portal / K-1 / waterfall stay Connect
  (#161). Equalization, drip, and side-pocket stay Connect (#177).
- ⚠ **A subscription / redemption walk-through (#181 / #27).** Record
  `subscribe_lp` with a unit count: cash and partner capital move, the
  trial balance still ties, no lot opens, `/capital` cites the partner's
  units, `/nav` cites units in issue, issued / redeemed this window,
  and per-share NAV (Euclidean; residual stays with the fund), and
  explain `capital_txns` counts the entry. `contribute_lp` is still
  funded capital without units — those stay **unset**, not a silent 0.
  Issued / redeemed / per-share stay **unset** when the window has no
  unit event or no units in issue — never a fake zero plug or a
  divided-by-zero per-share. Redeeming when unset, redeeming zero, and
  over-redemption refuse. A full redeem is `"0"`, a real zero.
  Book-level `subscribe` / `redeem` do not split 1/N across partners.
  CreateBook seeds the `subscriptions` ingest mapping; the live demo's
  `sub-0001` posts 500,000 units on Capital contributions, dated, so
  `/nav` can cite them. Ingest or record `subscribe_lp`. ⛔ The
  walk-through cannot show an LP portal, drip, or payment initiation
  (Connect — #161 / #150). Equalization, drip, and side-pocket stay
  Connect (#177) — a drip is still two existing postings plus an
  election, not a kernel primitive. This file closes #181. It does
  not reopen #180. It does not close #161.
- ⚠ **A management-fee accrual walk-through (#182 / #27).** Write
  `management_fee_accrual` (75 bp, act/365, expense 10 / payable 40)
  and accrue a dividing basis: expense debit and payable credit
  conserve, the trial balance still ties, no lot opens, GetBook
  and `/capital` cite the receivable. CreateBook writes no fee
  rule — the figure stays **unset**, not a silent 0. A zero-day
  no-op and same-sign legs refuse. ⛔ The walk-through cannot show
  an invoice PDF, an LP statement, or payment collection — those
  stay Connect (`fees:read` / `fees:accrue`).   This file closes
  #182. It does not reopen #181. It does not reopen #180.
- ⚠ **A project change-order walk-through (#91 / #27).** CreateBook(Project)
  seeds `Approved change orders` / `Change-order authorization` keyed by
  work package (site / structure / finishes, plus unpartitioned) as equity,
  the `approve_co_*` / `deduct_co_*` rules, and the `change-orders` ingest
  mapping. `[project] budget` stays the original contract. From `/budget`,
  record or ingest `approve_co_site`: the trial balance still ties, no lot
  opens, the same page cites original / approved / revised, and `/billing`
  uses the revised total as the billing basis with the same phase grain as
  cost-by-phase.
  A book that has never posted a CO shows **unset**, not a silent zero —
  `postingCount === "0"` is the distinction. A window chip (`change-YYYY-MM`)
  is which COs were approved in-period; committed spend stays as-of, because
  a project's period is still the project. ⛔ The walk-through cannot show a
  client portal, GC/sub marketplace, e-signature, full AIA G702 product UI,
  CRM, or a live construction job — CreateBook seeds the patterns; the
  seeded demo funds remain investment books.
- ⚠ **A project committed-cost / remaining-to-spend walk-through (#104 / #27).**
  CreateBook(Project) seeds `Awarded commitments` / `Commitment authorization`
  keyed by work package (site / structure / finishes, plus unpartitioned)
  as equity, the `award_commitment_*` / `release_commitment_*` rules, and
  the `purchase-orders` ingest mapping. From `/budget`, record or ingest
  `award_commitment_site`: the trial balance still ties, no lot opens,
  actual cost is unchanged, and the same page cites awarded committed cost
  on the same grain as cost-by-package. Remaining to spend is revised −
  incurred − awarded. A book that has never posted an award shows
  **unset**, not a fake zero committed — `postingCount === "0"` is the
  distinction; treating awarded as zero would print budget − actual as
  headroom. A posted award then a matching release is a real zero
  committed. `/budget` says plainly that it does not forecast — EAC and
  cost to complete are not a journal fact. Over/under-billing
  (costs in excess of billings) is not this page; billed minus earned
  stays on `/billing`. A Project EAC / forecast Connect app scaffold
  (`connect/eac-forecast/`, #169) computes those figures outside the
  journal from the same cites and leaves them unset when remaining
  cannot be supported. ⛔ The walk-through cannot show scheduling, Gantt,
  resource loading, e-procurement, a live vendor-portal Connect grant,
  AIA G702 product UI, EAC fields on `/budget`, a Connect token opening
  a book, live EAC product UX,
  a client portal, e-signature, or CRM. Remaining to bill stays on #100.
  This file does not close #169 (WorkOS dashboard registration leftover #22 and live EAC
  product UX remain). #170's leftover was posting / ingesting that award
  on the `/budget` chrome itself — that landed; estimating-tool Connect
  grants stay on #150 / #22. The seeded demo funds remain investment books.
- ⚠ **A project remaining-to-bill / collections walk-through (#100 / #173 / #27).**
  `/billing` cites remaining to bill (revised − billed) and collections vs
  billed (cash against AR: billed − outstanding receivable − retainage
  held). They compose from the same ListAccounts / `projectProgress` /
  `[project] budget` cuts change orders and billing already use — not a
  third chrome URL. From `/billing`, record `collect_receivable`: the
  trial balance still ties, no lot opens, and the same page cites
  collected / outstanding receivable with journal provenance. An unbilled
  job, or a book with no `[project] budget`, stays **unset** — not the
  whole contract as remaining, and not collected 0.00. Billed but
  uncollected is a real zero collected. `collect_receivable` is the
  cash-against-AR rule CreateBook already seeds — not a new journal
  kind. ⛔ The walk-through cannot show AIA G702 / SOV product UI, a
  client portal, e-signature, CRM, Stripe / ACH, a subcontractor
  pay-app marketplace, or SPI/CPI dashboards. A Project pay-app
  Connect app scaffold (`connect/aia-pay-app/`, #184) emits G702-ish /
  G703-ish CSV from those cites; missing billed / retainage / CO
  lines stay unset. It cannot show a Connect token opening a book or
  a licensed AIA form. Change orders stay on #91; retainage / billed
  vs earned stay on #85. #173's leftover was posting that collection
  on the `/billing` chrome itself — that landed; payment-processor
  Connect grants stay on #150 / #22. This file does not close #184
  (WorkOS dashboard registration leftover #22 and licensed form remain). A Project
  program roll-up Connect app scaffold (`connect/program-rollup/`,
  #179) cites those same per-book budget and billing figures
  across PROJECT books the subject can see and sums only the books
  that cited each figure — never a silent program billed or
  collected of 0.00, never a mega-book that merges journals. An
  `org_id` claim is not membership. It cannot show a Connect token
  opening a book. This file does not close #179 (WorkOS dashboard registration leftover
  #22 remains). The seeded demo funds remain
  investment books.
- ⚠ **A project job-cost / AP statement-ingest walk-through (#171 / #27).**
  CreateBook(Project) seeds `project-invoices` as the job-cost / AP /
  progress-bill mapping. Ingest a fixture: identified `invoice_site` /
  `progress_bill` / `cost` rows admit onto `journal.jsonl`, an unmatched
  vendor pends (the same queue as a recon break), the trial balance
  still ties, `/billing` cites billed from the progress-bill, and
  phase cost lands on the named work package. `hold_retainage` and
  `capitalize_wip` in Kind are refused — retainage and WIP stay
  **unset**, not a silent split of the invoice. `collect_receivable`
  stays on `/billing` (#173). ⛔ The walk-through cannot show a vendor
  portal, GC SaaS sync, AIA G702 product UI, EAC / forecast, or a
  client portal. Those stay Connect (#172 / #184 / #169). This file
  closes #171. It does not close #172, #184, #169, #155, or #150.
  The seeded demo funds remain investment books.
- ⚠ **An operating-business walk-through (#108).** CreateBook(Operating)
  writes an independent Book — no Fund, no WorkOS organization — with
  cash, AR, AP, operating revenue/expense, owner equity, and retained
  earnings. Record or ingest `invoice_customer` / `collect_receivable` /
  `vendor_bill` / `pay_vendor`: the trial balance still ties, no lot
  opens, `/sheet` cites the control accounts, and `/pnl` cites the
  period (month or year, not since inception). An undated entry is in
  no period. ⛔ The walk-through cannot invent due dates. `/aging` cites
  due-date buckets when invoices and bills carry a due date and
  collections/payments name the item they apply to; missing due dates
  stay **unset**, not current, and an unapplied reduction unsets that
  side — no FIFO, no equal split. Project `/billing` is one job's
  billed/earned/collections, not entity-wide aging. Payroll, tax filing,
  inventory/COGS, CRM, payment initiation, and bank-feed OAuth stay
  refused. `KIND_UNSPECIFIED` is not this kind and still falls through
  to fund operations. The seeded demo funds remain investment books.
- ⚠ **An operating cash-flow walk-through (#118 / #27).** It can show
  beginning and ending cash for a month or year, and classify the
  movement into operating (revenue, expenses, AR/AP working capital)
  and financing (owner contribution and draw). Investing stays **unset**
  — `chart_for(Operating)` has no PPE / securities account, and a
  silent 0.00 investing class is the defect. An invoice without
  collection is not operating cash; a vendor bill without payment is
  not an operating outflow. Beginning plus classified movement equals
  ending cash; a move the chart cannot name is a residual line an
  operator can open, not silent absorption. An empty journal, or a cut
  with no dated prefix, stays **unset** — not a measured $0.00 cash.
  Spending down to zero is a real zero. Aging stays on #117; a period
  close on #114. It cannot show a bank reconciliation, a cash
  forecast, payroll, tax filing, inventory/COGS, payment initiation,
  bank OAuth, or a client portal. Sheet / P&L stay on #108. The screen
  is the same `/cashflow` URL Personal already uses — one `screensFor`
  list.
- ⚠ **Write-route actor is the WorkOS `sub` (#151).** `Console::for_request`
  is the production constructor: AuthKit sessions take the open demo
  only when `RATIO_DEMO_OPEN` is set (local / CI). The deployed demo
  leaves it unset, so two AuthKit subjects isolate via membership.
  A Connect token is always scoped and does not match `org:{id}`.
  `applyEvent` / `ingest` / `admit` / CreateBook / period close record
  `ChangeLogEntry.actor` from the verified subject, never a request-body
  string. User B sees authorized-empty / "no fund" for user A's book on
  the `scoped` path. Connect tokens accepted with catalog scopes on
  `/v1` (frozen names only; aliases and hard non-scopes refused).
  RATIO_DEMO_OPEN defaults off on the deployed demo. first-party
  Connect apps call ConnectApiUrl. The demo API Lambda leaves
  `RATIO_JOURNAL_BUCKET` / `RATIO_JOURNAL_PREFIX` unset (local
  `/tmp` journals; S3 hydrate 503ed production `/books` with
  “the journal is still hydrating”). Scale keeps ScaleBucket.
  Leftover on
  #22: unused Cognito CloudFormation resources removed;
  `DEMO_MEMBERS` naming a live WorkOS `sub` and WorkOS dashboard
  registration remain. API Gateway JWT verifies Connect tokens
  on a second HTTP API (Connect `iss` =
  `https://auth.ratio.marsh.build`; AuthKit session `iss` stays
  `/user_management/{client_id}`). This file leaves #22 open
  for those leftovers.
- ⚠ **A period-close walk-through (#114 / #27).** It can show a book period
  closed against a named view, journal prefix and configuration digest,
  with actor and time; a back-dated posting into that period refused;
  beginning retained earnings → period surplus → ending retained earnings
  tying to the P&L and the post-close trial balance; and unset when no
  close has been recorded, when the destination is missing, or when the
  period had no income or expense to roll. CreateBook seeds
  `[close] equity_destination` on Personal (25), Investment (25),
  Project (29), and Operating (25). The operator verb is `ratio close`;
  the console is the evidence at `/close`. An open period may preview
  surplus and must say it is provisional. ⛔ The walk-through cannot
  show a control-plane configuration editor, a client portal,
  performance reporting, a tax-filing workflow, or a general workflow
  engine. A demo fund without `[close]` correctly refuses the close.
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
| `lean/Ratio/` | the proofs. `Bounded`, `Chart/Dimensions`, `Lots/{Relief,Methods,MinTax,SpecId,AverageCost,PoolPeriod,Edges,Posting,Wash,WashRestatement,WashHolding}`, `Partners/{Cut,Units,Notice}`, `Fees/Accrual`, `Actions/Factor`, `Closure`, `Exec` |
| `crates/ratio-rules` | `RuleSet`: `lot_method`, `chart_roles`, `long_term_days`, `wash_window_days`, `wash_keep_holding_period`, `min_tax_short_weight`, `average_cost`, `[personal] currencies` (household reporting codes; empty is unset, not a silent USD), `[personal] lot_relief` (household Investments; unset stays unset), `tolerance`, `partner_cut`, `special_allocation`, `fee_terms` (`management_fee_accrual`) — the administration agreement, as configuration |
| `lean/Ratio/Views.lean` | what a view IS: a recognition predicate. Every view conserves; two differ by exactly what is in flight; a fold with no CUT hides the difference entirely |
| `tla/Views.tla` | where the views ARE when somebody asks. One prefix, one pass, and the calendar inside the pinned config |
| `tla/` | `Projection`, `Executor`, `ReliefEngine`, `LotEngine`, `WashEngine`, `WashRestatement`, `WashHoldingPeriod`, `MinTaxEngine`, `SpecIdEngine`, `AverageCostEngine`, `PoolPeriodEngine`, `Actions`, `Valuation`, `ControlPlane`. Each has `manual`-tagged probes that must go RED |
| `crates/ratio-project` | the read model, the lot book, the relief engine — one pass, N view folds, each with a monotonic cut on the journal's own clock and a band bounded by the settlement lag. ⚠ every memory figure in this file is a ONE-VIEW figure; each view carries its own lot book |
| `crates/ratio-sql-project` | Stage E table snapshot of that fold. One watermark (`prefix` + `ratio_nav::prefix_digest`); lots / positions / aggregates replaced together. A stale pin refuses. Relief is `relieve_by`, not `ORDER BY seq`. Journal stays SoR. Live Postgres / 20M-lot claim stay #8 / #159 |
| `crates/ratio-gen` + `ratio bench` | the generated fund and the measurement |
| `crates/ratio-console` | the console's BFF — 40 RPCs, transcoded onto `/v1`. Personal `forecast-YYYY[-MM]` folds posted `scheduled` / `forecast` journal kinds only; unset when none. Connect predictors post that material; they do not close #163 |
| `crates/ratio-nav/src/explain.rs` | what a strike DOES, as a plan. ⛔ a description of two code paths, not a planner over them — nothing chooses |
| `console/` | the console itself. Next.js on Vercel; ⛔ Bazel does not build it |
| `tomato-bazel/rules_postgres` | `Pg.Rel.Semantics` — merged, PR #9 |
| `AGENTS.md` | the rules, for a person or a model, and the dispatch contract (one issue → one cloud agent → one PR). Replaces the two stale LLM guides |
| `docs/connect-scopes.md` | WorkOS Connect scope catalog ([#150](https://github.com/mattmarshall/ratio/issues/150)). Connect tokens accepted with catalog scopes on `/v1` after membership. Write-route actor = WorkOS `sub`; a Connect-shaped token never takes `RATIO_DEMO_OPEN` and never matches `org:{id}` (#151). API Gateway JWT verifies Connect tokens on the Connect HTTP API (AuthKit custom-domain issuer). `RATIO_DEMO_OPEN` defaults off on the deployed demo. first-party Connect apps call ConnectApiUrl. The demo API Lambda leaves `RATIO_JOURNAL_BUCKET` / `RATIO_JOURNAL_PREFIX` unset (local `/tmp` journals; S3 hydrate 503ed production `/books`). Scale keeps ScaleBucket. Hard non-scopes: `rules:approve`, `config:promote`, portal impersonation. leftover #22: unused Cognito CloudFormation resources removed; `DEMO_MEMBERS` naming a live WorkOS `sub` and WorkOS dashboard registration remain. Equalization, drip, and side-pocket stay Connect ([#177](https://github.com/mattmarshall/ratio/issues/177)) — existing scopes, no new grants; drip on #161; equalization / side-pocket apps not filed |
| `connect/bank-feed/` | First-party Connect app for Personal bank feeds ([#165](https://github.com/mattmarshall/ratio/issues/165)). Mapper + allowlist + closed-through / conservation refusals. first-party Connect apps call ConnectApiUrl; live bank OAuth is leftover. Does not close #165 |
| `connect/tax-pack/` | First-party Connect app for household tax-pack export ([#166](https://github.com/mattmarshall/ratio/issues/166)). 8949-ish CSV from lot / wash / lot-terms cites. Mixed acquired dates stay unclassified — `Ratio.Lots.PoolPeriod`, not an invented FIFO oldest date or two Form 8949 boxes. first-party Connect apps call ConnectApiUrl; IRS e-file is refused. Does not close #166 |
| `connect/goals/` | First-party Connect app for Personal net-worth goals and what-if scenarios ([#168](https://github.com/mattmarshall/ratio/issues/168)). Cites sheet / bridge / cash-flow; opt-in scenario journals on allowlisted Personal templates; closed-through and empty-allowlist refuse. first-party Connect apps call ConnectApiUrl. Not a cash forecast. Does not close #168 |
| `connect/aia-pay-app/` | First-party Connect app for Project G702/G703-ish pay-app export ([#184](https://github.com/mattmarshall/ratio/issues/184)). CSV from `/billing` + `/budget` cites (revised contract, billed, retainage, change orders). Missing cites stay unset — no silent billed / retainage / CO zeros. WorkOS dashboard registration leftover #22; licensed AIA PDF is refused. Does not close #184. Does not reopen #151 |
| `connect/vendor-portal/` | First-party Connect app for a Project vendor / GC portal ([#172](https://github.com/mattmarshall/ratio/issues/172)). Cites billed / earned / retainage / collections from `/billing`; remaining-to-bill and cash-against-AR stay unset until the journal can support them. Vendor invoices are allowlisted `journals:post` for `vendor_invoice*` — not `journal:append`. first-party Connect apps call ConnectApiUrl. No vendor user directory in core. No AIA G702 product UI (#184). No EAC / forecast (#169). Does not close #172 |
| `connect/eac-forecast/` | First-party Connect app for Project EAC / forecast ([#169](https://github.com/mattmarshall/ratio/issues/169)). CSV / JSON from `/budget` + `/billing` cites. Remaining to spend is revised − incurred − awarded; EAC / ETC stay unset until that cut can support a figure — never a silent 0.00. Read-only: no forecast template, `journal:append` refused. WorkOS dashboard registration leftover #22; live EAC product UX and `/budget` EAC fields stay refused. Does not close #169. Does not reopen #151 |
| `connect/program-rollup/` | First-party Connect app for Project multi-contract / program roll-up ([#179](https://github.com/mattmarshall/ratio/issues/179)). CSV / JSON from per-book `/budget` + `/billing` cites across PROJECT books the subject can see (`books:read` membership; an `org_id` claim is not membership). Program totals sum only set cites — never a silent billed / collected 0.00 for a book that lacks the cite. Read-only: no `journals:post`. No mega-book, no fifth kind, no `screensFor` fork. WorkOS dashboard registration leftover #22. Does not close #179. Does not close #169, #172, or #184. Does not reopen #151 |
| `connect/bank-balance-predictor/` | First-party Connect app for Personal bank-balance predictors ([#163](https://github.com/mattmarshall/ratio/issues/163)). Maps predicted movements (or a predicted ending balance against cited cash) onto allowlisted `forecast_income` / `forecast_spend`. Unset cited cash is not a silent 0.00 baseline. Closed-through and empty-allowlist refuse. `journal:append` is an alias. Payroll / envelope kinds refuse. WorkOS dashboard registration leftover #22; live bank OAuth leftover. Does not close #163. Does not reopen #164. Does not redo #218 |
| `connect/calendar-bills/` | First-party Connect app for Personal calendar bills sync ([#163](https://github.com/mattmarshall/ratio/issues/163)). Maps dated occurrences onto allowlisted `scheduled_income` / `scheduled_spend`. Recurrence stays in the calendar — an `rrule` is refused rather than expanded. Closed-through and empty-allowlist refuse. `journal:append` is an alias. Payroll / envelope kinds refuse. WorkOS dashboard registration leftover #22; live calendar OAuth leftover. Does not close #163. Does not reopen #164. Does not redo #218 |
| `connect/audit-export/` | First-party Connect app for an audit evidence ZIP ([#185](https://github.com/mattmarshall/ratio/issues/185)). Cites `PeriodClose`, `NavStrike`, `Break` / `BreakExplanation`, and config / journal digests already on the book. Missing cites stay unset in the manifest — never a silent empty file, a NAV 0.00, or an empty-digest-as-success. Read-only: no `journals:post`. No kernel blob store, no period-close replacement, no LP portal, no e-sign, no second journal. first-party Connect apps call ConnectApiUrl. WorkOS dashboard registration leftover #22. Does not close #185. Leaves issue 22 open. Does not close #150. Does not reopen #151 |

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
