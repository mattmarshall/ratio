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
- **`Break.tolerance` is null on a lot break ON PURPOSE.** Lot breaks are HIGH
  by what they mean — the lot book and the position disagreeing corrupts the
  realized gain, which no reconciliation reaches — so reporting bounds beside
  one would suggest some other number would have graded it differently. None
  would.
- **`ChartRoles` are checked when the configuration is READ.** A chart that
  cannot express a gain is wrong the moment it is written down; finding out at
  the first disposal means finding out in production.

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
| `tla/` | `Projection`, `Executor`, `ReliefEngine`, `LotEngine`, `Actions`, `Valuation`, `ControlPlane`. Each has `manual`-tagged probes that must go RED |
| `crates/ratio-project` | the read model, the lot book, the relief engine |
| `crates/ratio-gen` + `ratio bench` | the generated fund and the measurement |
| `crates/ratio-console` | the console's BFF — 34 RPCs, transcoded onto `/v1` |
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
