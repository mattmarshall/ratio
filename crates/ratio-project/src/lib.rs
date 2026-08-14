//! ratio-project — the derived read model, and the type that makes its one
//! catastrophic failure unrepresentable.
//!
//! # Why there is a projection at all
//!
//! `FileBook::positions` folds the whole journal on every call. At the rate
//! `ratio_nav::closure::measure` reports, twenty million lots is 89 seconds —
//! and `Ratio.Exec.no_partition_beats_the_io_floor` says no number of workers
//! touches that, because it is IO. So reads move to a projection folded once
//! and advanced incrementally, while the journal stays the system of record
//! because replay and content-addressed digests are the product.
//!
//! # ⛔ The one way this goes catastrophically wrong
//!
//! `//tla:projection_check` proves the safety condition and
//! `//tla:unpinned_projection_check` shows it failing: a figure PINS one
//! journal position and READS a projection built from another. Nothing
//! downstream can notice. The trial balance ties on whatever it is handed, the
//! digest is well-formed, and `ratio replay` recomputes from the pinned prefix
//! and disagrees — by which time the first number is what somebody was paid on
//! and `Ratio.Period.one_answer_per_view_per_day` refuses to restate it.
//!
//! ⭐ SO THE POSITION IS NOT A FIELD A CALLER LOOKS UP. Every read returns
//! [`AsOf`], which carries the prefix it was folded from, and there is no other
//! way to get a number out of this crate. A caller cannot pin the journal head
//! while reading a lagging projection because it never has the head to hand —
//! it has only what it read. The TLA property is `StrikeFoldsItsOwnPrefix`;
//! here it is the type.
//!
//! ⚠ That is a stronger guarantee than a test, and a weaker one than it looks:
//! nothing stops a caller writing `.value` and pairing it with a position from
//! somewhere else. What the type buys is that doing so requires saying so.

use std::collections::BTreeMap;

/// The per-lot relief decisions, authored in Lean.
mod generated_lots;

/// Relieving tax lots — the walk, over decisions made in Lean.
pub mod relief;

/// Recognising an entry, under one book of record — the walk, over decisions
/// `Ratio.Views` proves.
pub mod views;

use anyhow::Result;
use ratio_ingest::factor::Step;
use ratio_common::intern::Text;
use ratio_store::{FileBook, Journal, JournalEntry};

/// A value read from the projection, carrying the journal prefix it was folded
/// from.
///
/// ⛔ THE PREFIX TRAVELS WITH THE VALUE. A strike records `prefix`, not the
/// journal's length, and that is the whole safety argument — see the module
/// docs and `//tla:unpinned_projection_check`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsOf<T> {
    pub value: T,
    /// Entries folded. Also the position a figure built from this must pin.
    pub prefix: usize,
}

impl<T> AsOf<T> {
    /// Transform the value, keeping the prefix. The prefix cannot be changed by
    /// this or any other method — it is set once, by the fold that produced it.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AsOf<U> {
        AsOf { value: f(self.value), prefix: self.prefix }
    }
}

/// Positions, folded from a journal prefix.
///
/// Shaped like `FileBook::positions` returns them so the two can be compared
/// directly — which `the_projection_agrees_with_a_full_fold` does, because a
/// read model that drifts from the record it derives from is worse than no read
/// model at all.
/// Running totals a NAV needs, accumulated as the journal is folded.
///
/// ⛔ ACCUMULATED, NOT RECOMPUTED. `ratio_nav::fold_nav` walks every entry on
/// every strike — O(journal), and the journal holds every trade ever made. These
/// move by exactly what each new entry contributes, so a strike off a maintained
/// projection is O(positions): `Ratio.Plan.aggregate_agrees_with_scan`, and
/// `Ratio.Plan.a_stale_total_makes_the_plans_disagree` is what goes wrong if
/// they ever drift.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Totals {
    /// Postings by dimension, whatever the account type. The NAV picks out
    /// assets and liabilities; the projection does not know the chart.
    ///
    /// ⛔ `i128`, LIKE `ratio_nav::fold_nav`. These accumulate over the whole
    /// journal, so `debits` in particular grows with HISTORY rather than with
    /// the fund — it adds the magnitude of every posting ever made. An `i64`
    /// accumulator wraps, and a wrapped total does not look wrong; it looks
    /// like a NAV.
    /// ⛔ KEYED BY DIMENSION **AND CURRENCY**, because a total over both is not
    /// a figure. `Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch`
    /// is about exactly this sum: a hundred dollars and minus a hundred euros
    /// add to zero and nothing balanced. This map used to key on the dimension
    /// alone, which was harmless only for as long as no posting carried a
    /// currency — and the moment one did, every NAV would have been a mixture
    /// of denominations reported as one number.
    pub by_dim: BTreeMap<(i64, Option<Text>), i128>,
    pub debits: i128,
    pub credits: i128,
}

/// How many hundredths of the base currency one unit of a currency is worth.
///
/// ⚠ HUNDREDTHS, matching the rate facts the data plane already carries and
/// `Ratio.Closure.fxCost`'s one-rate-per-currency shape. It is a coarse rate,
/// and it is the rate this system actually has.
pub const RATE_SCALE: i64 = 100;

/// What each currency is worth, for translating a book into one figure.
///
/// ⛔ A TRANSLATION IS NOT A CONSERVATION. The kernel checks that every currency
/// nets to zero on its own — that is the law, and it needs no rates. This is the
/// separate question of what the whole book is WORTH in one denomination, which
/// cannot be answered without saying at what rate. Keeping them apart is why
/// `is_balanced` takes no rates and this exists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Rates {
    /// The currency everything is translated INTO.
    ///
    /// ⛔ EXPLICIT, BECAUSE IT CANNOT BE INFERRED. There is no rate fact for the
    /// base — a fund does not record what a dollar is worth in dollars — so the
    /// base is exactly the currency the rates are silent about, and "the one
    /// that is missing" is not something to work out from a data file. A
    /// translation whose destination is unstated is a number without a unit.
    base: Option<String>,
    per: BTreeMap<String, i64>,
}

impl Rates {
    /// No rates at all: an untyped book translates at par and a typed one is
    /// refused.
    ///
    /// ⚠ THE DEFAULT REFUSES RATHER THAN ASSUMING PAR. A missing rate treated
    /// as 1.00 would report a fund holding yen at its yen figure, which is off
    /// by two orders of magnitude and looks like an ordinary number.
    pub fn none() -> Self {
        Self::default()
    }

    /// `code -> hundredths of the base currency`.
    pub fn of(base: &str, per: impl IntoIterator<Item = (String, i64)>) -> Self {
        Self { base: Some(base.to_string()), per: per.into_iter().collect() }
    }

    /// The rates the data plane has resolved.
    ///
    /// ⛔ ONE PLACE THAT KNOWS WHAT A RATE FACT LOOKS LIKE. `ratio bench` and
    /// the console both need this, and two readers of one record format is two
    /// chances to disagree about which field is the rate — with the disagreement
    /// showing up as a fund valued at a number nobody chose.
    ///
    /// ⚠ ONE RATE PER CURRENCY, not per position:
    /// `Ratio.Closure.fx_does_not_grow_with_the_chart`. Three currencies is
    /// three rows at five holdings or five hundred.
    pub fn of_facts(base: &str, facts: &[ratio_ingest::Fact]) -> Self {
        Self::of(
            base,
            facts.iter().filter(|f| f.kind == "rate").filter_map(|f| {
                Some((
                    f.values.get("currency")?.as_text()?.to_string(),
                    f.values.get("rate")?.as_minor()?,
                ))
            }),
        )
    }

    /// How many rate FACTS this carries.
    ///
    /// ⛔ THE FACTS, NOT THE CURRENCIES, AND THE TWO DIFFER BY EXACTLY ONE. The
    /// base has no rate fact — a fund does not record what a dollar is worth in
    /// dollars — so a book holding three currencies carries two rows here.
    /// `Ratio.Closure.fxCost` counts the CURRENCIES. A caller quoting this as
    /// that would be one short on every book, and short by precisely the
    /// denomination the figure is reported in, which is the one nobody checks.
    pub fn len(&self) -> usize {
        self.per.len()
    }

    /// Whether any rate was supplied. `Rates::none()` is empty and refuses.
    pub fn is_empty(&self) -> bool {
        self.per.is_empty()
    }

    /// The factor for a named currency, for a caller costing a translation
    /// rather than performing one.
    pub fn factor_of(&self, currency: &str) -> Option<i64> {
        self.factor(Some(currency))
    }

    /// The factor for a posting's currency, `None` included.
    ///
    /// ⚠ For a caller doing its own fold — `ratio_nav::NavFold` — which must
    /// translate exactly as `Projection::nav` does or the recorded NAV and the
    /// maintained one disagree. They did.
    pub fn factor_of_optional(&self, currency: Option<&str>) -> Option<i64> {
        self.factor(currency)
    }

    /// The factor for one posting's currency.
    ///
    /// ⛔ AN UNTYPED LEG TRANSLATES AT PAR, and that is a DIFFERENT decision
    /// from the one `PostingRecord::currency` makes for conservation, where
    /// `None` is its own group. Both are right: for the law, an untyped leg
    /// cannot be assumed to be any particular currency, so it groups alone; for
    /// translation, a book with no currencies in it is already in one
    /// denomination and there is nothing to translate.
    fn factor(&self, currency: Option<&str>) -> Option<i64> {
        match currency {
            None => Some(RATE_SCALE),
            // ⛔ THE BASE IS AT PAR AND HAS NO FACT. A fund does not record what
            // a dollar is worth in dollars, so looking the base up in `per`
            // would refuse every book that holds any of its own currency.
            Some(c) if self.base.as_deref() == Some(c) => Some(RATE_SCALE),
            Some(c) => self.per.get(c).copied(),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Positions {
    /// `(dim, instrument) -> (cost, quantity)`.
    pub held: BTreeMap<(i64, Text), (i64, i64)>,
    /// `dim -> amount`, for postings naming no instrument.
    pub rest: BTreeMap<i64, i64>,
}

/// The corporate actions a projection has seen, and whether each was applied by
/// REWRITING the lots or is still to be read through as a factor.
///
/// ⛔ AN ACTION IS ONE OR THE OTHER, NEVER BOTH, and the journal says which.
/// Every book written before `Ratio.Actions.Factor` has `action-{id}` entries
/// that already walked the lots — the units in the projection include those
/// splits. Applying a factor on top would square them, silently, while the
/// trial balance went on tying: `Ratio.Actions.applying_twice_is_not_applying_
/// once` in a new costume.
///
/// So the rule is derived rather than configured: an announcement whose
/// `action-{id}` entry is in the prefix has ALREADY been rewritten and must not
/// be read through; one without is read through and costs nothing to leave
/// open. That is the whole win — an outstanding action stops being a cliff.
#[derive(Clone, Debug, Default)]
struct Actions {
    /// `instrument -> (ex_date, id, num, den)`, in journal order.
    announced: Vec<(String, String, String, i64, i64)>,
    /// Ids whose rewrite entry is in the prefix.
    rewritten: std::collections::BTreeSet<String>,
}

/// What one entry's configuration tells the lot engine.
///
/// ⛔ RESOLVED FROM THE DIGEST THAT ENTRY PINNED, never from whatever is active
/// now. All three of these are terms of an administration agreement rather than
/// implementation choices, and all three decide a REALIZED GAIN — the figure
/// with no counterparty, which no reconciliation reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// Which lots a sale gives up.
    pub method: relief::Method,
    /// Which dimensions play which part. `None` when the chart names no roles,
    /// in which case a gain cannot be attributed to a disposal at all.
    pub roles: Option<ratio_rules::ChartRoles>,
    /// Days held for a gain to be long-term. `Ratio.Lots.Methods.isLongTerm`.
    pub long_term_days: i64,
}

impl Terms {
    /// The terms a rule set sets.
    pub fn of(set: &ratio_rules::RuleSet) -> Self {
        Self {
            method: set.effective_lot_method().into(),
            roles: set.chart_roles,
            long_term_days: set.long_term_days,
        }
    }

    /// Terms for a caller folding a slice of entries, which carries no chart.
    ///
    /// ⚠ `roles: None`, so nothing is CLASSIFIED short or long — and that is
    /// visible rather than assumed, because the split is reported against a
    /// total that includes what could not be classified.
    pub fn under(method: relief::Method) -> Self {
        Self { method, ..Self::of(&ratio_rules::RuleSet::default()) }
    }
}

/// Where a cold build's time actually went.
///
/// ⛔ MEASURED, BECAUSE EVERY UNMEASURED ANSWER TODAY HAS BEEN WRONG AT LEAST
/// ONCE. The cold build was believed to be O(entries) and labeled that way in
/// two places; holding entries constant and raising fragmentation 4× more than
/// doubled it, so a term proportional to entries × lots-per-position dominates.
/// This says which line that term is on rather than leaving it to be reasoned
/// about.
///
/// ⚠ COARSE ON PURPOSE. `parse` and `fold` are timed once per `follow`, not per
/// entry — seven million `Instant::now()` calls would be a measurement that
/// changed what it measured. `relieve` is timed per SALE, which is the one
/// place the extra resolution is worth its cost.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FoldCost {
    /// Reading and deserializing the journal.
    pub parse: std::time::Duration,
    /// Folding the parsed entries into totals, positions, actions and lots.
    pub fold: std::time::Duration,
    /// Of `fold`, the part inside `relief::relieve_by`.
    pub relieve: std::time::Duration,
    /// How many reliefs that was.
    pub reliefs: u64,
}

/// What a fund has realized, and how much of it is classified.
///
/// ⛔ `unclassified` IS DERIVED, NOT ACCUMULATED: `gain − short − long`. The
/// split is a partition of a figure the fold does not own — the total is the
/// realized-gain DIMENSION, which ties to the trial balance — so computing the
/// remainder is the only way the three cannot drift from it. A fourth
/// accumulator would be a fourth thing to keep in step, and the failure would be
/// three figures that each look reasonable and do not sum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Realized {
    /// ⛔ CREDIT-NORMAL: a gain reads NEGATIVE. `Ratio.Lots.Posting` has the
    /// convention. A screen printing this unflipped shows every profitable fund
    /// with a minus sign.
    pub gain: i128,
    /// Cost given up by sales.
    pub basis: i128,
    /// The part of `gain` on holdings held less than the threshold.
    pub short_term: i128,
    /// And at least the threshold. `the_threshold_day_is_long_term`.
    pub long_term: i128,
}

impl Realized {
    /// The part of the gain no holding period could be established for.
    ///
    /// ⚠ NOT AN ERROR, AND NOT ZERO IN PRACTICE. A lot opened by an entry with
    /// no `trade_date` has no acquisition date, and the honest answer about its
    /// holding period is that there is not one — both defaults being wrong in
    /// opposite directions is the same argument `relieve_by` already makes.
    ///
    /// ⚠ AND ON A MULTI-CURRENCY BOOK IT CARRIES A TRANSLATION RESIDUE OF AT
    /// MOST ONE MINOR UNIT PER CURRENCY. Integer translation does not distribute
    /// over a sum: `(a + b) × r / s` and `a × r / s + b × r / s` differ by the
    /// dropped remainders. The total and the two parts are each translated, so
    /// the difference lands here.
    ///
    /// ⛔ WHICH IS WHY THIS IS DERIVED RATHER THAN ACCUMULATED. Computed as the
    /// remainder, the three parts sum to the total exactly, by construction. A
    /// fourth accumulator would put the residue nowhere and leave four figures
    /// that do not add up — and a reader has no way to tell a rounding residue
    /// from a disposal that went missing.
    pub fn unclassified(&self) -> i128 {
        self.gain - self.short_term - self.long_term
    }
}

/// Open tax lots, per position, and what relieving them has cost.
///
/// ⛔ THE LOTS ARE MAINTAINED BY THE FOLD, not derived on demand. A buy opens a
/// lot; a sale relieves through `relief::relieve_by` under the method the
/// entry's own configuration named, which is the walk `Ratio.Lots` proves.
/// Deriving them on demand would mean re-walking the journal per query — the
/// cost this whole crate exists to remove.
///
/// ⚠ AND THIS IS WHERE THE MEMORY IS. Positions are a chart: five hundred
/// entries for an S&P tracker whatever its history. Lots are a HISTORY: twenty
/// million of them is roughly 800 MB at 40 bytes each, which is the number the
/// scale argument has to survive and the reason `//tla:lot_engine_check` models
/// paging rather than assuming everything is resident.
#[derive(Clone, Debug, Default)]
struct LotBook {
    /// `(dim, instrument) -> open lots`, oldest first.
    open: BTreeMap<(i64, Text), relief::Holding>,
    /// Cumulative cost given up by sales. ⛔ NOT the realized gain: that needs
    /// PROCEEDS, which is a property of the transaction rather than of the
    /// position, and the fold does not know which leg was the cash.
    ///
    /// ⛔ PER CURRENCY, like every other total here. A sum over denominations is
    /// not a figure.
    relieved: BTreeMap<Option<Text>, i128>,
    /// The part of the realized gain a holding period could be established for.
    ///
    /// ⛔ ONLY THE CLASSIFIABLE PART IS ACCUMULATED HERE. The TOTAL is the
    /// realized-gain dimension in `totals`, which ties to the trial balance; if
    /// this held its own copy of the total there would be two answers to one
    /// question, and the one that drifted would be the one nothing checks.
    ///
    /// ⛔ AND PER CURRENCY, WHICH IS NOT COSMETIC. These are reported as parts
    /// of a TRANSLATED total, so they have to be translated the same way. Held
    /// as flat sums they were local-currency figures being subtracted from a
    /// base-currency one, and the difference landed in `unclassified` — which
    /// then meant two unrelated things at once: disposals whose holding period
    /// was unknown, and an FX translation difference. A figure that means two
    /// things is the failure this whole file is about.
    short_term: BTreeMap<Option<Text>, i128>,
    long_term: BTreeMap<Option<Text>, i128>,
    /// ⛔ SALES THAT COULD NOT BE RELIEVED, named rather than propagated.
    ///
    /// A husk, a pro-rata split that will not divide, a holding that is short —
    /// each is a real refusal from `relief::relieve`, and each concerns ONE
    /// position. A projection that refused to build because one instrument's
    /// lots would not divide would take the whole fund down over a line item,
    /// so these surface as breaks, which is already what this product calls a
    /// thing an operator must look at.
    breaks: Vec<String>,
}

/// One book of record's fold over the shared journal.
///
/// ⭐ EXACTLY THE STATE THAT DEPENDS ON WHICH ENTRIES ARE RECOGNISED, and
/// nothing else. `Ratio.Views` classifies a view against the three kinds of
/// label a posting carries and it is none of them — it says WHICH ENTRIES ARE
/// IN SCOPE — so what it changes is the set folded, and every figure derived
/// from that set moves with it. The chart, the interner, the corporate actions
/// and the journal position stay on `Projection`, one copy each.
///
/// ⚠ `actions` IS DELIBERATELY NOT HERE. An announcement is a fact about an
/// instrument, not a posting in scope of a recognition convention: two views
/// have heard about the same split. Putting it per view would leave two copies
/// of one fact to keep in step.
#[derive(Clone, Debug, Default)]
struct ViewFold {
    positions: Positions,
    totals: Totals,
    lots: LotBook,
}

/// The read model.
#[derive(Clone, Debug)]
pub struct Projection {
    /// The books of record this projection folds, by view id.
    ///
    /// ⛔ ONE ENTRY, `UNDECLARED_VIEW`, UNTIL THE CUT LANDS. This map is the
    /// structural half of the per-view fold: the state has moved, the number
    /// has not. Every existing assertion passing unchanged is what says the
    /// extraction was faithful — see `PLAN.md`'s design note for the half that
    /// remains, which is a monotonic cut and the band of what it has not
    /// reached.
    views: BTreeMap<String, ViewFold>,
    actions: Actions,
    /// How far into the journal FILE this has read.
    ///
    /// ⛔ BYTES, NOT ENTRIES, and the two are not interchangeable. `at` says how
    /// many entries were folded; this says where to resume reading without
    /// parsing what came before. A projection that tracked only `at` would have
    /// to read and discard the whole journal to find entry `at + 1`.
    read_to: u64,
    /// Entries folded so far.
    ///
    /// ⛔ PRIVATE, and there is no setter. `advance` moves it by exactly what
    /// it folded. `//tla:rebuild_double_counts_check` is the failure this
    /// prevents: a projection whose claimed position stays honest while its
    /// contents are folded twice is not detectably wrong — the number is simply
    /// too big.
    at: usize,
    /// Where the fold's time went. Accumulated across every `follow`.
    cost: FoldCost,
    /// One copy of each instrument name and currency code this book mentions.
    ///
    /// ⛔ THE HOT PATH IS `key = (dim, instrument)`, BUILT PER POSTING. At four
    /// thousand lots a security that is fourteen million `String` allocations
    /// to look up rows that already exist. Interned, a key costs a refcount
    /// bump — and the five hundred distinct names are stored once rather than
    /// once per posting that mentions them.
    names: ratio_common::intern::Interner,
    /// The relief method each configuration names, resolved once per digest.
    ///
    /// ⛔ CACHED BY DIGEST BECAUSE THE FOLD IS PER ENTRY. A book holds a handful
    /// of configurations and millions of entries naming them; reading and
    /// parsing the TOML per entry would be most of the cost of a cold build.
    /// `FileBook::append_all` caches its provenance check the same way and for
    /// the same reason.
    ///
    /// ⚠ AN UNRESOLVABLE CONFIG IS HELD AS ITS ERROR, never as a default. This
    /// is the one place where guessing is worst: FIFO is the right answer for
    /// some fund somewhere, so a guess that lands on it is indistinguishable
    /// from having read it.
    terms: BTreeMap<ratio_store::Digest, Result<Terms, String>>,
}

/// ⛔ HAND-WRITTEN, FOR THE REASON `RuleSet`'s IS. `#[derive(Default)]` gives
/// every field its type's default, and `views` would come out EMPTY — a
/// projection with no book of record at all, which is not a state this type has.
/// Every book has at least one view; a book that declares none has exactly one,
/// recognising in journal order. `Ratio.Views.nobody_said_is_not_a_settlement_
/// convention` is that as a theorem, and this is where the code has to agree
/// with it.
impl Default for Projection {
    fn default() -> Self {
        Self {
            views: [(ratio_rules::UNDECLARED_VIEW.to_string(), ViewFold::default())]
                .into_iter()
                .collect(),
            actions: Actions::default(),
            read_to: 0,
            at: 0,
            cost: FoldCost::default(),
            names: ratio_common::intern::Interner::default(),
            terms: BTreeMap::new(),
        }
    }
}

impl Projection {
    /// The one book of record this folds, until the cut lands.
    ///
    /// ⚠ `expect` RATHER THAN A `Result`, AND THAT IS AN INVARIANT NOT A RISK:
    /// `Default` puts the undeclared view in and nothing removes it. When this
    /// map grows a second entry these become `fold_of(view)` returning a
    /// refusal, because THEN a caller can name a view a book does not keep.
    fn only(&self) -> &ViewFold {
        self.views
            .get(ratio_rules::UNDECLARED_VIEW)
            .expect("every projection holds the view every book has")
    }
}

impl Projection {
    /// An empty projection, at position zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// How many entries this has folded.
    pub fn prefix(&self) -> usize {
        self.at
    }

    /// Fold the entries this has not seen.
    ///
    /// ⛔ FROM `self.at`, NEVER FROM ZERO. Re-folding onto state already held
    /// double-counts, and the result carries an honest position over doubled
    /// contents. `advance` takes the WHOLE journal and skips what it has,
    /// rather than taking a delta the caller computed — a delta is one more
    /// thing a caller can get wrong, and getting it wrong is silent.
    /// Returns how many entries it folded.
    ///
    /// ⛔ RETURNED SO THE INCREMENTAL PROPERTY IS OBSERVABLE. "It advances
    /// rather than rebuilding" is otherwise only checkable by timing, and a
    /// timing test that passes on a rebuild fast enough to look incremental
    /// proves nothing. A maintained projection folds the DELTA; this is the
    /// number that says so.
    ///
    /// ⛔ THE METHOD IS A PARAMETER AND HAS NO DEFAULT. A slice of entries does
    /// not carry the configurations they named — only `follow` can read those —
    /// so a caller folding this way must say which method the whole slice was
    /// relieved under. It read `Method::Fifo` implicitly for a long time, which
    /// meant a fund declaring HIFO was relieved FIFO while every other figure
    /// agreed: `//tla:stale_method_relief_check`. An unparameterized `advance`
    /// is that defect with a shorter signature.
    pub fn advance(&mut self, journal: &[JournalEntry], method: relief::Method) -> usize {
        for (i, entry) in journal.iter().enumerate().skip(self.at) {
            self.fold(i, entry, &Ok(Terms::under(method)));
        }
        let folded = journal.len().saturating_sub(self.at);
        // ⛔ `max`, NOT `= journal.len()`. A SHORTER journal must not rewind the
        // prefix — the entries were folded and the totals still hold them.
        // Assigning the length outright let a truncated or sliced read move the
        // position BACKWARD, after which the next advance re-folds everything
        // between and double-counts it: `//tla:rebuild_double_counts_check`,
        // reachable without any rebuild at all.
        //
        // ⚠ Found by `a_maintained_projection_folds_only_the_delta`, which was
        // written to check the incremental property and caught this instead.
        // Every other test in this file passed — none of them ever handed
        // `advance` a journal shorter than one it had already seen.
        self.at = journal.len().max(self.at);
        folded
    }

    /// Fold ONE entry into the totals, the positions, and the action index.
    ///
    /// ⛔ THE ONLY PLACE AN ENTRY IS FOLDED. `advance` takes a slice and
    /// `follow` reads bytes off disk, and both go through here — two folds
    /// would be two chances to disagree about what an entry means, and the
    /// disagreement would be a NAV that changed depending on how the projection
    /// happened to be brought up to date.
    fn fold(
        &mut self,
        at: usize,
        entry: &JournalEntry,
        terms: &Result<Terms, String>,
    ) {
        self.fold_lots(at, entry, terms);
        if let Some(a) = &entry.announcement {
            self.actions.announced.push((
                a.instrument.clone(),
                a.ex_date.clone(),
                a.id.clone(),
                a.numerator,
                a.denominator,
            ));
        }
        if let Some(id) = entry.id.strip_prefix("action-") {
            self.actions.rewritten.insert(id.to_string());
        }
        // ⛔ THE INTERNER AND THE FOLD ARE BORROWED SEPARATELY, ONCE, OUTSIDE THE
        // LOOP. Both are fields of `self`, and reaching through `self` for each
        // in turn would borrow the whole thing twice per posting — so the map
        // lookup that finds the view would land on the hot path, which is per
        // POSTING across millions of entries. Destructuring splits the borrow at
        // no cost, which is the same reason `names` exists at all.
        let Self { names, views, .. } = self;
        let fold = views
            .get_mut(ratio_rules::UNDECLARED_VIEW)
            .expect("every projection holds the view every book has");
        for p in &entry.postings {
            // ⚠ `-p.amount` is not the magnitude at the floor: `-i64::MIN`
            // overflows. Widened first, it does not.
            let amount = p.amount as i128;
            // ⛔ THE LAST PER-POSTING ALLOCATION. Five distinct currency codes
            // across seven million entries; cloning the code to look up a row
            // that already exists is the same waste the instrument key was.
            let ccy = p.currency.as_deref().map(|c| names.intern(c));
            *fold.totals.by_dim.entry((p.dim, ccy)).or_default() += amount;
            if amount >= 0 {
                fold.totals.debits += amount;
            } else {
                fold.totals.credits += -amount;
            }
            match &p.instrument {
                Some(i) => {
                    let key = (p.dim, names.intern(i));
                    let slot = fold.positions.held.entry(key).or_insert((0, 0));
                    slot.0 += p.amount;
                    slot.1 += p.quantity.unwrap_or(0);
                }
                None => *fold.positions.rest.entry(p.dim).or_default() += p.amount,
            }
        }
    }

    /// Build from scratch.
    ///
    /// Discards everything first, so this is a rebuild rather than a second
    /// advance — the distinction `//tla:rebuild_double_counts_check` is about.
    ///
    /// Takes the method for the same reason `advance` does: a slice of entries
    /// is not a book, and the configurations they name are only readable from
    /// one.
    pub fn rebuild(journal: &[JournalEntry], method: relief::Method) -> Self {
        let mut p = Self::new();
        let _ = p.advance(journal, method);
        p
    }

    /// Open a book and build a projection of it.
    pub fn of_book(path: &std::path::Path) -> Result<Self> {
        Self::of_book_with_progress(path, &mut |_| {})
    }

    /// The same cold build, reporting how far through it is.
    ///
    /// ⛔ BECAUSE SILENCE AND A HANG LOOK IDENTICAL. The cold build of the book
    /// issue #6 measured takes 995 seconds, and `of_book` says nothing until it
    /// returns — so anything watching one has no way to tell a fold in progress
    /// from a process that died holding the file open. The callback is the only
    /// difference; the fold is the same fold.
    ///
    /// The count handed back is entries folded SO FAR, not a fraction: a journal
    /// does not know its own length without reading it, and reading it twice to
    /// print a percentage would double the cost of the thing being reported on.
    pub fn of_book_with_progress(
        path: &std::path::Path,
        on: &mut dyn FnMut(usize),
    ) -> Result<Self> {
        let mut p = Self::new();
        p.follow_with_progress(path, on)?;
        Ok(p)
    }

    /// Fold whatever has been appended since this last read. Returns how many.
    ///
    /// ⭐ THIS IS WHAT MAKES THE FLAT CURVE TRUE IN A RUNNING PROCESS. A
    /// projection that called `entries()` would parse the whole journal on
    /// every call just to learn nothing had changed; this seeks to where it
    /// stopped and reads forward. Calling it on an unchanged book costs a
    /// `stat` and a seek.
    ///
    /// ⚠ It can FAIL, and the failure is worth having. A journal shorter than
    /// the offset means the file was replaced — a different book at the same
    /// path — and resuming from a stale offset would splice two histories and
    /// fold the result as one.
    pub fn follow(&mut self, path: &std::path::Path) -> Result<usize> {
        self.follow_with_progress(path, &mut |_| {})
    }

    /// `follow`, reporting entries folded as it goes. See `of_book_with_progress`.
    ///
    /// ⚠ The count is entries folded BY THIS CALL, which is the same thing as
    /// the total only when the projection started empty. `follow` on a
    /// projection that has already read a prefix counts the delta, exactly as
    /// its return value does.
    ///
    /// ⚠ THE CALLBACK FIRES EVERY `PROGRESS_EVERY` ENTRIES, NOT EVERY ENTRY, and
    /// the reason is the one `FoldCost` already documents: at a hundred and forty
    /// million entries a per-entry call is a measurement that changes what it
    /// measures. It also fires once at the end, so a fold of fewer than
    /// `PROGRESS_EVERY` entries still reports its total rather than nothing.
    pub fn follow_with_progress(
        &mut self,
        path: &std::path::Path,
        on: &mut dyn FnMut(usize),
    ) -> Result<usize> {
        /// Matches `ratio_gen`'s `FLUSH_EVERY`: the generator writes in chunks of
        /// this, so a reader reporting on the same boundary reports on whole
        /// chunks rather than on an offset that means nothing to either side.
        const PROGRESS_EVERY: usize = 65_536;

        let book = FileBook::open(path)?;

        // ⛔ STREAMED, ONE ENTRY AT A TIME. `entries_since` returned a `Vec` of
        // the whole journal — 1.85 GB resident to fold 1.77M entries into a
        // projection holding 8 MB of lots, and about eighty gigabytes at the
        // shape issue #6 asks for. The fold was never going to run out of time.
        //
        // ⚠ THE CONFIG RESOLUTION MOVED INSIDE THE WALK for the same reason: it
        // used to pre-scan `fresh` for distinct digests, which needed `fresh` to
        // exist. Resolving on first sight of a digest costs the same one read
        // per configuration and needs nothing held.
        let started = std::time::Instant::now();
        let mut n = 0usize;
        let mut at = self.at;
        let mut parse_and_fold = std::time::Duration::ZERO;
        let now = book.for_each_entry_since(self.read_to, &mut |entry| {
            let mark = std::time::Instant::now();
            if !self.terms.contains_key(&entry.config) {
                let resolved = Self::terms_of(&book, &entry.config);
                self.terms.insert(entry.config.clone(), resolved);
            }
            let terms = self.terms.get(&entry.config).cloned().unwrap_or_else(|| {
                Err("the configuration was not resolved before folding".to_string())
            });
            self.fold(at + n, entry, &terms);
            n += 1;
            parse_and_fold += mark.elapsed();
            // ⛔ WHATEVER THIS DOES IS CHARGED TO THE COLD BUILD. The timed span
            // above has closed, so the cost lands in `parse` rather than `fold`,
            // but it lands: a callback that wrote to the network here would be
            // reporting a number it had itself inflated. Store the count and
            // return; publish it from somewhere else.
            if n % PROGRESS_EVERY == 0 {
                on(n);
            }
            Ok(())
        })?;
        // The tail, so a fold shorter than one chunk still reports its total
        // rather than nothing. ⚠ Skipped on an exact multiple, where the loop
        // has already reported this very number.
        if n % PROGRESS_EVERY != 0 {
            on(n);
        }
        at += n;
        self.at = at;
        self.read_to = now;
        // ⚠ `parse` is what the whole read cost MINUS the folding inside it,
        // which is the only way to separate them when they interleave.
        self.cost.fold += parse_and_fold;
        self.cost.parse += started.elapsed().saturating_sub(parse_and_fold);
        Ok(n)

    }

    /// Where the cold build's time went.
    pub fn cost(&self) -> FoldCost {
        self.cost
    }

    /// The terms a stored configuration sets for the lot engine.
    ///
    /// ⚠ THE ERROR IS CARRIED, NOT RAISED. A configuration that cannot be read
    /// concerns the sales posted under it, not the whole fund — the same
    /// judgement `LotBook::breaks` already makes. A projection that refused to
    /// build would take a fund's NAV down over one bad config, and the NAV does
    /// not read the lots at all (`Ratio.Closure.factored_nav_never_reads_the_
    /// lots`).
    fn terms_of(book: &FileBook, config: &ratio_store::Digest) -> Result<Terms, String> {
        use ratio_store::ConfigStore;
        let bytes = book
            .get(config)
            .map_err(|e| format!("config {} could not be read — {e:#}", config.as_str()))?;
        let set = ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&bytes))
            .map_err(|e| format!("config {} is not a rule set — {e:#}", config.as_str()))?;
        Ok(Terms::of(&set))
    }

    /// The positions, as of the prefix folded.
    ///
    /// ⛔ THE ONLY WAY OUT OF THIS TYPE, and it hands back the prefix with the
    /// value. There is deliberately no `fn positions(&self) -> &Positions`.
    pub fn positions(&self) -> AsOf<&Positions> {
        AsOf { value: &self.only().positions, prefix: self.at }
    }

    /// Total cost held in one instrument, across every account.
    pub fn cost_of(&self, instrument: &str) -> AsOf<i64> {
        AsOf {
            value: self
                .only()
                .positions
                .held
                .iter()
                .filter(|((_, i), _)| &**i == instrument)
                .map(|(_, (cost, _))| *cost)
                .sum(),
            prefix: self.at,
        }
    }

    /// Maintain the lot book for one entry.
    /// ⛔ `at` IS THE ENTRY'S JOURNAL POSITION, and it is a parameter because
    /// the obvious source was wrong. `self.at` does not move within a batch —
    /// `advance` folds a whole slice before updating it — so every lot opened in
    /// one call got the SAME ordinal. FIFO survived by accident, on the
    /// stability of the sort, and the ordinals differed between a cold build and
    /// an incremental one. `the_lot_book_advances_with_everything_else` caught
    /// it; nothing else would have.
    ///
    /// ⛔ `method` IS THE ONE THE ENTRY'S OWN CONFIGURATION NAMED, resolved by
    /// the caller. Which lots a sale gives up is a term of an administration
    /// agreement — `Ratio.Lots.Methods.the_method_decides_the_taxable_gain` is
    /// four methods giving four different taxable incomes from one holding and
    /// one trade — so relieving under anything else is relieving somebody
    /// else's book. It is silent: the units left are right, the proceeds are
    /// right, the trial balance ties, and only the realized gain moves.
    fn fold_lots(
        &mut self,
        at: usize,
        entry: &JournalEntry,
        terms: &Result<Terms, String>,
    ) {
        // ⛔ SPLIT ONCE, LIKE `fold`. `lots` is per view and `names` and
        // `cost` are not, and this walks every disposal in the entry — so
        // reaching through `self` for each in turn would put the lookup that
        // finds the view inside the loop over lots, which is the one loop in
        // this crate that grows with a fund's trading history.
        let Self { names, views, cost, .. } = self;
        let fold = views
            .get_mut(ratio_rules::UNDECLARED_VIEW)
            .expect("every projection holds the view every book has");
        // ⛔ THE GAIN IS ATTRIBUTED ONLY WHEN THE ENTRY IS UNAMBIGUOUS: exactly
        // one sale, and a chart that names where a gain goes. An entry disposing
        // of two instruments carries ONE gain leg between them, and splitting it
        // would be inventing an attribution the journal does not record.
        //
        // ⚠ Ambiguous entries are not dropped — they land in
        // `Realized::unclassified`, which is the total MINUS what was
        // classified, so nothing can go missing without showing up there.
        // ⛔ PARSED ONCE PER ENTRY, NOT ONCE PER LOT AND AGAIN PER CLASSIFY. The
        // date is stored on every lot this entry opens and read again by every
        // disposal that relieves one, and it used to be an ISO string at both
        // ends — so a book with a million open lots retained a million small
        // allocations and re-parsed text to decide a holding period.
        //
        // ⚠ A DATE THAT WILL NOT PARSE IS A BREAK, NOT A SILENT ABSENCE. Left as
        // `None` it is indistinguishable from an entry that never carried one,
        // and the holding-period methods would refuse the holding without
        // anybody learning why.
        let trade_day: Option<relief::Day> = match entry.trade_date.as_deref() {
            None => None,
            Some(d) => match ratio_common::days_from_iso_date(d) {
                Ok(n) => Some(n as relief::Day),
                Err(e) => {
                    fold.lots.breaks.push(format!(
                        "{}: trade date {d:?} is not a date — {e:#}. Lots it opens carry \
                         no acquisition date, and the holding-period methods refuse them",
                        entry.id
                    ));
                    None
                }
            },
        };

        let sales = entry
            .postings
            .iter()
            .filter(|p| p.instrument.is_some() && p.quantity.is_some_and(|q| q < 0))
            .count();
        let gain_leg: Option<i64> = terms.as_ref().ok().and_then(|t| t.roles).and_then(|r| {
            let legs: Vec<i64> = entry
                .postings
                .iter()
                .filter(|p| p.dim == r.realized_gain)
                .map(|p| p.amount)
                .collect();
            (sales == 1 && !legs.is_empty()).then(|| legs.iter().sum())
        });

        for p in &entry.postings {
            let (Some(inst), Some(qty)) = (&p.instrument, p.quantity) else {
                continue;
            };
            if qty == 0 {
                continue;
            }
            let key = (p.dim, names.intern(inst));
            if qty > 0 {
                // A purchase opens a lot. `seq` is the journal position, which
                // IS acquisition order — `relief::relieve` sorts by it rather
                // than trusting the vector, but giving it the honest ordinal
                // costs nothing and makes the sort a check rather than a fix.
                let lot = relief::Lot {
                    seq: at as u64,
                    units: qty,
                    cost: p.amount,
                    // ⛔ FROM THE ENTRY, AND `None` WHEN IT HAS NONE. Every
                    // journal written before `trade_date` existed lacks one, and
                    // the holding-period methods refuse such a lot rather than
                    // defaulting — both defaults are wrong in opposite
                    // directions.
                    acquired: trade_day,
                };
                // ⛔ A HUSK IS REFUSED WHERE IT IS OFFERED, so the walk no longer
                // rescans the whole holding on every sale looking for one.
                if let Err(e) = fold.lots.open.entry(key).or_default().push(lot) {
                    fold.lots.breaks.push(format!("{}: {e:#}", entry.id));
                }
                continue;
            }
            // A sale relieves — under the method this entry's configuration
            // named, or not at all.
            //
            // ⛔ A CONFIGURATION THAT COULD NOT BE READ REFUSES THE RELIEF. The
            // tempting fallback is FIFO, and it is the worst available answer:
            // it is a real method that real funds elect, so a book relieved
            // under it by accident is indistinguishable from one relieved under
            // it by agreement. A break says which sale and why.
            let terms = match terms {
                Ok(t) => *t,
                Err(why) => {
                    fold.lots.breaks.push(format!(
                        "{}: selling {} of {} could not be relieved — the lot method is \
                         not known: {why}",
                        entry.id, -qty, inst
                    ));
                    continue;
                }
            };
            let method = terms.method;
            let held = fold.lots.open.entry(key.clone()).or_default();
            let relieving = std::time::Instant::now();
            let relieved = held.relieve(method, -qty);
            cost.relieve += relieving.elapsed();
            cost.reliefs += 1;
            match relieved {
                Ok(r) => {
                    // ⛔ THE POSITION AND THE LOT BOOK ARE TWO INDEPENDENT
                    // PATHS, AND NOTHING FORCES THEM TO AGREE. The aggregate
                    // follows the amount the entry POSTED; the lots follow what
                    // relieving them actually cost. An entry that posts a basis
                    // FIFO does not agree with leaves the two drifting, and both
                    // are internally consistent — the trial balance ties on the
                    // posted figure and the lot book ties on the computed one.
                    //
                    // ⚠ `Ratio.Lots.aggregate_matches_scan` is the theorem that
                    // they must agree. It is about one relief; this is the
                    // system-level obligation, and a derived model cannot
                    // enforce it — the journal is the record. What it can do is
                    // notice, and say which figure it disagrees with.
                    // ⚠ NAMES THE METHOD IT ACTUALLY USED. This message said
                    // "oldest-first" whatever the fund had elected, back when
                    // the fold ignored the configuration — so the one line an
                    // operator would read to investigate a drift asserted the
                    // very thing that was wrong.
                    if -p.amount != r.cost {
                        fold.lots.breaks.push(format!(
                            "{}: selling {} of {} posted {} of basis, and relieving the lots \
                             {} costs {} — the position and the lot book \
                             will disagree by {}",
                            entry.id,
                            -qty,
                            inst,
                            -p.amount,
                            method.describe(),
                            r.cost,
                            -p.amount - r.cost
                        ));
                    }
                    let ccy = p.currency.as_deref().map(|c| names.intern(c));
                    *fold.lots.relieved.entry(ccy.clone()).or_default() += r.cost as i128;
                    // Computed before `r.left` is moved out below, and as a free
                    // function because `held` still borrows the lot book.
                    let split = gain_leg
                        .and_then(|g| classify(&r, g, trade_day, terms));
                    if let Some((short, long)) = split {
                        *fold.lots.short_term.entry(ccy.clone()).or_default() += short;
                        *fold.lots.long_term.entry(ccy).or_default() += long;
                    }
                }
                Err(e) => fold.lots.breaks.push(format!(
                    "{}: selling {} of {} could not be relieved — {e:#}",
                    entry.id, -qty, inst
                )),
            }
        }
    }

    /// What the fund has realized, as of the prefix folded.
    ///
    /// ⛔ THE TOTAL COMES FROM THE CHART, NOT FROM THE LOT BOOK. It is the
    /// realized-gain dimension's balance, which is the figure the trial balance
    /// ties on — so this cannot disagree with the books. Only the SPLIT is the
    /// lot engine's, and it is reported as a part of that total rather than
    /// beside it.
    ///
    /// Returns `None` when the configuration names no chart roles: without one
    /// the engine does not know which dimension a gain lands in, and answering
    /// zero would be a fund that realized nothing.
    /// ⚠ THE GAIN IS TRANSLATED AND THE SPLIT IS NOT. The total is a chart
    /// balance and can hold several currencies; the split is accumulated by the
    /// fold in whatever the disposal was denominated in. On a book with one
    /// currency they agree, and `unclassified` being the remainder means a
    /// multi-currency book shows the discrepancy there rather than hiding it.
    /// Separating an FX gain from a security gain is the modelling question
    /// this deliberately does not answer.
    pub fn realized(
        &self,
        roles: Option<ratio_rules::ChartRoles>,
        rates: &Rates,
    ) -> Result<AsOf<Option<Realized>>> {
        let value = match roles {
            None => None,
            Some(r) => Some(Realized {
                gain: self.translate(&|dim| dim == r.realized_gain, rates)?,
                basis: convert(&self.only().lots.relieved, rates)?,
                short_term: convert(&self.only().lots.short_term, rates)?,
                long_term: convert(&self.only().lots.long_term, rates)?,
            }),
        };
        Ok(AsOf { value, prefix: self.at })
    }

    /// The open lots of one position, oldest first.
    pub fn lots_of(&self, dim: i64, instrument: &str) -> AsOf<Vec<relief::Lot>> {
        AsOf {
            value: self
                .only()
                .lots
                .open
                // ⚠ Allocates, and that is fine HERE: this is a read for one
                // position, not the per-posting key the fold builds. The tuple
                // key cannot be borrowed as `(i64, &str)`, and a linear scan to
                // avoid one malloc would trade nine comparisons for five
                // hundred.
                .get(&(dim, Text::from(instrument)))
                .map(|h| h.lots())
                .unwrap_or_default(),
            prefix: self.at,
        }
    }

    /// How many open lots this fund holds, across every position.
    ///
    /// ⛔ THE NUMBER THE SCALE ARGUMENT IS ABOUT, and it is deliberately NOT in
    /// `nav`. `Ratio.Closure.factored_nav_never_reads_the_lots` is the claim
    /// that this figure does not appear in a NAV's cost, and having it available
    /// here is what lets that be checked rather than asserted.
    pub fn open_lots(&self) -> i64 {
        self.only().lots.open.values().map(|v| v.len() as i64).sum()
    }

    /// Distinct currencies this book's totals are keyed by.
    ///
    /// ⛔ THE FX TERM'S DIAL, AND IT IS NOT THE POSITION COUNT.
    /// `Ratio.Closure.fx_does_not_grow_with_the_chart`: translation applies to
    /// per-currency SUBTOTALS, so a fund holding five hundred names in three
    /// currencies does three translations rather than five hundred. A caller
    /// costing a period end off `positions()` would charge the wrong term.
    ///
    /// ⚠ COUNTS THE UNTYPED BALANCE AS ONE. A posting carrying no currency is a
    /// conservation group of its own — `Rates::factor_of_optional` translates it
    /// through the base — so leaving it out would report a fund with fewer
    /// denominations than it has.
    pub fn currency_count(&self) -> i64 {
        self.only()
            .totals
            .by_dim
            .keys()
            .map(|(_, c)| c.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as i64
    }

    /// Rows the maintained NAV actually walks: one per (dimension, currency).
    ///
    /// ⛔ NOT `Ratio.Closure.markCost`, WHICH IS THE SECURITIES. The model
    /// charges one read per security; `translate` walks this map. Quoting either
    /// as the other is how an estimate stops being checkable against the thing
    /// it estimates — which is the only reason to have both numbers.
    pub fn total_rows(&self) -> i64 {
        self.only().totals.by_dim.len() as i64
    }

    /// Corporate actions announced inside this prefix and not yet rewritten.
    ///
    /// ⛔ OPEN, NOT ANNOUNCED-EVER, and `Ratio.Closure` is explicit about why:
    /// an action already applied by rewriting has been paid for, so a count of
    /// every action a book has ever seen is not a dial at all. This is the
    /// number `actionCost` multiplies, and the one `the_cliff` is about.
    pub fn open_action_count(&self) -> i64 {
        self.actions
            .announced
            .iter()
            .filter(|(_, _, id, _, _)| !self.actions.rewritten.contains(id))
            .count() as i64
    }

    /// Cumulative cost given up by sales.
    pub fn relieved_cost(&self) -> i128 {
        self.only().lots.relieved.values().sum()
    }

    /// Sales that could not be relieved, and why.
    pub fn lot_breaks(&self) -> &[String] {
        &self.only().lots.breaks
    }

    /// Net asset value and the trial-balance difference, off the maintained
    /// totals rather than a walk over the journal.
    ///
    /// ⭐ THIS IS THE POINT OF THE WHOLE CRATE. `ratio_nav::fold_nav` is
    /// O(journal) and this is O(dimensions) — a chart, not a history. The figure
    /// must be IDENTICAL, which `the_projection_strikes_the_same_nav_as_a_full_
    /// fold` checks against the existing path rather than against itself.
    ///
    /// A liability nets negative because it is credit-normal, so summing assets
    /// and liabilities subtracts without a special case — the same fold as
    /// `ratio_nav`, and a sign error here is invisible in a screenshot and wrong
    /// by twice the liability.
    pub fn nav(
        &self,
        is_asset_or_liability: &dyn Fn(i64) -> bool,
        rates: &Rates,
    ) -> Result<AsOf<(i64, i64)>> {
        let nav = self.translate(&|dim| is_asset_or_liability(dim), rates)?;
        // ⛔ A figure that cannot be represented is REFUSED rather than
        // truncated. `Ratio.Bounded`: an operation agrees with the theorem or
        // declines, and there is no third answer.
        Ok(AsOf {
            value: (
                i64::try_from(nav).map_err(|_| {
                    anyhow::anyhow!("this fund's net asset value does not fit in 64 bits")
                })?,
                i64::try_from(self.only().totals.debits - self.only().totals.credits).map_err(|_| {
                    anyhow::anyhow!("this fund's trial-balance difference does not fit in 64 bits")
                })?,
            ),
            prefix: self.at,
        })
    }

    /// Sum the dimensions a predicate picks out, translated into one currency.
    ///
    /// ⛔ REFUSES A CURRENCY IT HAS NO RATE FOR. The tempting alternatives are
    /// both worse than an error: skipping the leg reports a fund that does not
    /// hold what it holds, and translating at par reports yen as though it were
    /// dollars. Neither looks wrong on the page.
    ///
    /// ⚠ TRANSLATION ROUNDS, AND RELIEF DOES NOT. `Ratio.Lots.partial_relief_
    /// is_exactly_pro_rata` refuses a basis that will not divide because the
    /// remainder would be a misstatement of taxable income; a translated figure
    /// has no exact answer at any rate with finitely many digits, so refusing
    /// would refuse every foreign holding. Rounded down, in `i128`, once per
    /// (dimension, currency) rather than per posting.
    fn translate(&self, want: &dyn Fn(i64) -> bool, rates: &Rates) -> Result<i128> {
        let mut total: i128 = 0;
        for ((dim, currency), amount) in &self.only().totals.by_dim {
            if !want(*dim) {
                continue;
            }
            let factor = rates.factor(currency.as_deref()).ok_or_else(|| {
                anyhow::anyhow!(
                    "this fund holds {} and no rate for it was supplied — a figure \
                     mixing denominations is not a figure",
                    currency.as_deref().unwrap_or("an untyped balance")
                )
            })?;
            total += amount * factor as i128 / RATE_SCALE as i128;
        }
        Ok(total)
    }

    /// The splits an instrument's stored units must be read through, on a day.
    ///
    /// Announced on or before the day, inside this prefix, and NOT already
    /// rewritten. In journal order, which is ex-date order for anything
    /// announced before it took effect — and
    /// `Ratio.Actions.actions_do_not_commute` means that order is part of the
    /// answer rather than an implementation detail.
    pub fn steps_for(&self, instrument: &str, day: &str) -> Vec<Step> {
        self.actions
            .announced
            .iter()
            .filter(|(i, ex, id, _, _)| {
                i == instrument && ex.as_str() <= day && !self.actions.rewritten.contains(id)
            })
            .map(|(_, _, _, num, den)| Step { num: *num, den: *den })
            .collect()
    }

    /// Units held in one instrument on a day, read through its open actions.
    ///
    /// ⭐ THIS IS WHAT MAKES AN OUTSTANDING ACTION FREE. Nothing is rewritten,
    /// so `Ratio.Closure.factored_nav_never_reads_the_lots` holds — the cost is
    /// O(splits) on the one instrument rather than O(lots) over all of them.
    ///
    /// ⚠ It can REFUSE, and that is not a bug in the read path. A step that
    /// does not divide means the holder was paid cash in lieu, which realizes a
    /// gain and is a posting the configuration must declare:
    /// `Ratio.Actions.Factor.a_factor_can_succeed_where_the_rewrite_refuses`.
    pub fn units_as_of(&self, dim: i64, instrument: &str, day: &str) -> Result<AsOf<i64>> {
        let stored = self
            .positions
            .held
            .get(&(dim, Text::from(instrument)))
            .map(|(_, q)| *q)
            .unwrap_or(0);
        Ok(AsOf {
            value: ratio_ingest::factor::units_as_of(stored, &self.steps_for(instrument, day))?,
            prefix: self.at,
        })
    }

    /// Whether this has caught up with a journal of the given length.
    ///
    /// ⚠ Lagging is SAFE — `//tla:projection_check` proves it, because a figure
    /// pins what it read. This exists so a caller can wait for freshness when
    /// it wants freshness, not because a stale read would be wrong.
    pub fn is_current_with(&self, journal_len: usize) -> bool {
        self.at == journal_len
    }
}

/// Translate a per-currency total into the base.
///
/// ⚠ THE SAME ARITHMETIC AS `Projection::translate`, over a map the lot book
/// keeps rather than the chart. Both exist because a total over denominations
/// is not a figure, and the split has to be translated the same way as the
/// total it is a part of or the two do not add up.
fn convert(by_currency: &BTreeMap<Option<Text>, i128>, rates: &Rates) -> Result<i128> {
    let mut total = 0i128;
    for (currency, amount) in by_currency {
        let factor = rates.factor(currency.as_deref()).ok_or_else(|| {
            anyhow::anyhow!(
                "this fund realized gains in {} and no rate for it was supplied",
                currency.as_deref().unwrap_or("an untyped currency")
            )
        })?;
        total += amount * factor as i128 / RATE_SCALE as i128;
    }
    Ok(total)
}

/// Split one disposal's gain into (short-term, long-term), or decline.
///
/// ⛔ THE GAIN FOLLOWS THE PROCEEDS, NOT THE COST. Two lots relieved by one sale
/// share its proceeds in proportion to their UNITS — that is what the holder
/// sold — while each keeps its own basis. Apportioning by cost instead gives
/// every lot a gain of zero and puts the whole difference on whichever lot the
/// arithmetic happened to end on.
///
/// ⛔ AND IT REFUSES RATHER THAN ROUNDING, which is
/// `Ratio.Lots.partial_relief_is_exactly_pro_rata`'s discipline in the one place
/// it had not been applied. A disposal whose proceeds do not divide exactly
/// across its lots is left WHOLLY unclassified: rounding here moves minor units
/// between two tax rates, which is not a rounding error — it is a misstatement
/// of taxable income.
///
/// ⚠ A FREE FUNCTION because the caller still holds a mutable borrow of the lot
/// book while it has the `Relieved` in hand, and because nothing here needs the
/// projection: it is arithmetic over one disposal.
///
/// ⚠ Everything it declines stays in the total, and shows up in
/// `Realized::unclassified` — which is the remainder, so nothing goes missing.
fn classify(
    r: &relief::Relief,
    gain: i64,
    disposed_on: Option<relief::Day>,
    terms: Terms,
) -> Option<(i128, i128)> {
    // `sale_postings` posts the gain credit-normal: `relieved − proceeds`.
    let proceeds = ratio_common::checked::sub(r.cost, gain, "proceeds").ok()?;
    let day = disposed_on? as i64;
    let units: i64 = r.taken.iter().map(|t| t.units).sum();
    if units <= 0 {
        return None;
    }

    let mut short = 0i128;
    let mut long = 0i128;
    for t in &r.taken {
        let scaled = ratio_common::checked::mul(proceeds, t.units, "proceeds pro rata").ok()?;
        if scaled.rem_euclid(units) != 0 {
            return None;
        }
        let acquired = t.acquired? as i64;
        let share = ratio_common::checked::sub(t.cost, scaled / units, "a lot's gain").ok()?;
        // `Ratio.Lots.Methods.isLongTerm`: the threshold day IS long-term.
        if day - acquired >= terms.long_term_days {
            long += share as i128;
        } else {
            short += share as i128;
        }
    }
    Some((short, long))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::{Account, AccountTypeRecord as A, ConfigStore, PostingRecord};

    /// An ISO date as the day number a lot now stores.
    fn day(iso: &str) -> relief::Day {
        ratio_common::days_from_iso_date(iso).unwrap() as relief::Day
    }

    /// Where a test's book goes.
    ///
    /// ⛔ `TEST_TMPDIR` WHEN BAZEL SETS IT, which is per test run and cleaned
    /// between them. `std::env::temp_dir()` is the user's one shared directory,
    /// so a book survives the run that made it — and a run interrupted midway
    /// leaves one whose ACTIVE config pointer names a blob that is no longer
    /// there. The next run then fails inside a helper, reporting a stored-config
    /// error about a test that has nothing to do with configs.
    ///
    /// ⚠ Observed, not theorized: a sabotage run left exactly that state behind
    /// and the following clean run failed with `entry "s" names config … which
    /// is not stored`.
    ///
    /// ⛔ AND THE NAME MUST BE UNIQUE ACROSS THE WHOLE FILE. Tests run in
    /// PARALLEL, every book helper begins with `remove_dir_all`, so two tests
    /// naming one book wipe each other's directory mid-flight — leaving ACTIVE
    /// pointing at a config blob that has just been deleted. It fails perhaps
    /// one run in three, in whichever test lost the race, reporting a
    /// stored-config error that has nothing to do with what it is testing.
    fn tmp_root() -> std::path::PathBuf {
        match std::env::var_os("TEST_TMPDIR") {
            Some(d) => std::path::PathBuf::from(d),
            None => std::env::temp_dir(),
        }
    }

    /// The method the slice-folding tests below are written against.
    ///
    /// ⚠ Named rather than defaulted. Every one of these tests was written when
    /// the fold relieved FIFO unconditionally, so FIFO is what preserves their
    /// meaning — but a test that does not say which method it assumes is a test
    /// that cannot notice the method changing under it.
    const FIFO: relief::Method = relief::Method::Fifo;

    fn book(name: &str, trades: &[(&str, i64, i64)]) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        for (n, (inst, cost, qty)) in trades.iter().enumerate() {
            b.append(&JournalEntry {
                id: format!("t{n}"),
                memo: "buy".into(),
                config: c.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: *cost,
                        currency: None,
                        instrument: Some((*inst).into()),
                        quantity: Some(*qty),
                    },
                    PostingRecord::new(2, -*cost),
                ],
            
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    /// One currency's rate, in the shape `Rates::of_facts` reads.
    fn rate_fact(currency: &str, minor: i64) -> ratio_ingest::Fact {
        ratio_ingest::Fact {
            id: format!("rate-{currency}"),
            kind: "rate".into(),
            reference: currency.into(),
            entities: Default::default(),
            values: [
                ("currency".to_string(), ratio_ingest::Value::Text { text: currency.into() }),
                ("rate".to_string(), ratio_ingest::Value::Decimal { minor }),
            ]
            .into_iter()
            .collect(),
            provenance: ratio_ingest::Provenance {
                delivery: "test".into(),
                row: 2,
                template: "test".into(),
                template_id: "test".into(),
                received: 0,
            },
        }
    }

    fn entries(d: &std::path::Path) -> Vec<JournalEntry> {
        FileBook::open(d).unwrap().entries().unwrap()
    }

    fn sell(d: &std::path::Path, id: &str, inst: &str, units: i64, cost: i64) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "sell".into(),
            config: c,
            postings: vec![
                PostingRecord {
                    dim: 1,
                    amount: -cost,
                    currency: None,
                    instrument: Some(inst.into()),
                    quantity: Some(-units),
                },
                PostingRecord::new(2, cost),
            ],
            trade_date: None,
            announcement: None,
        })
        .unwrap();
    }

    fn buy_dated(d: &std::path::Path, id: &str, inst: &str, units: i64, cost: i64, day: &str) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "buy".into(),
            config: c,
            postings: vec![
                PostingRecord {
                    dim: 1,
                    amount: cost,
                    currency: None,
                    instrument: Some(inst.into()),
                    quantity: Some(units),
                },
                PostingRecord::new(2, -cost),
            ],
            trade_date: Some(day.into()),
            announcement: None,
        })
        .unwrap();
    }

    #[test]
    fn a_lot_carries_the_trade_date_of_the_entry_that_opened_it() {
        // ⛔ THE PROPAGATION WAS UNTESTED. Every test here built undated
        // entries, so a fold that dropped the date entirely would have passed
        // the whole file — and the holding-period methods would then refuse
        // every holding, or worse, be given `None` and silently fall back.
        //
        // Found by mutation: replacing `entry.trade_date.clone()` with `None`
        // changed nothing.
        let d = book("dates", &[]);
        buy_dated(&d, "b1", "vti", 10, 100, "2024-03-01");
        buy_dated(&d, "b2", "vti", 10, 400, "2026-01-15");
        let p = Projection::of_book(&d).unwrap();

        let lots = p.lots_of(1, "vti").value;
        assert_eq!(lots.len(), 2);
        assert_eq!(lots[0].acquired, Some(day("2024-03-01")));
        assert_eq!(lots[1].acquired, Some(day("2026-01-15")));

        // And a holding-period method can then be run against them: the older
        // lot is given up first, which is the point of recording the date.
        let r = relief::relieve_by(relief::Method::LongestHeldFirst, &lots, 10).unwrap();
        assert_eq!(r.cost, 100, "the lot held longest, not the cheapest or the first");
        assert_eq!(r.taken[0].acquired, Some(day("2024-03-01")), "and it says when");
    }

    #[test]
    fn an_undated_holding_refuses_a_holding_period_method() {
        // ⚠ Every book written before `trade_date` existed is this. The refusal
        // is the honest outcome — a tax rate guessed from an absence is a claim
        // the records do not support.
        let d = book("undated", &[("vti", 100, 10)]);
        let p = Projection::of_book(&d).unwrap();
        let lots = p.lots_of(1, "vti").value;
        assert!(lots[0].acquired.is_none());
        assert!(relief::relieve_by(relief::Method::LongestHeldFirst, &lots, 5).is_err());
        assert!(relief::relieve_by(relief::Method::Fifo, &lots, 5).is_ok(), "FIFO still works");
    }

    #[test]
    fn buys_open_lots_and_sales_relieve_them_oldest_first() {
        // ⭐ THE ENGINE ON A REAL BOOK. Two buys of one unit — 10 then 40 — and
        // a sale of one. FIFO gives up the CHEAP lot, so 10 of basis leaves and
        // the dear one remains. LIFO would have given up 40 and reported a
        // quarter of the gain on the eventual sale.
        let d = book("lotfold", &[("vti", 10, 1), ("vti", 40, 1)]);
        sell(&d, "s1", "vti", 1, 10); // the cheap lot's basis, which is what FIFO gives up
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.open_lots(), 1, "one lot left");
        let left = p.lots_of(1, "vti");
        assert_eq!(left.value[0].units, 1);
        assert_eq!(left.value[0].cost, 40, "the DEAR lot survives, not the cheap one");
        assert_eq!(p.relieved_cost(), 10, "and 10 of basis was given up");
        assert!(p.lot_breaks().is_empty());
    }

    #[test]
    fn an_entry_posting_a_basis_fifo_disagrees_with_is_a_break() {
        // ⭐ THE FINDING THIS TEST SUITE PRODUCED. The position aggregate and
        // the lot book are two independent paths — one follows the amount the
        // entry POSTED, the other follows what relieving the lots actually
        // costs — and nothing forces them to agree. Both stay internally
        // consistent: the trial balance ties on the posted figure and the lot
        // book ties on the computed one, so the drift is invisible to every
        // check either side has.
        //
        // Two one-unit lots at 10 and 40; a sale posting 40 of basis. FIFO gives
        // up the CHEAP lot, so the true basis is 10 and the books will disagree
        // by 30 — which is also 30 of realized gain that will never be reported.
        let d = book("drift", &[("vti", 10, 1), ("vti", 40, 1)]);
        sell(&d, "s1", "vti", 1, 40);
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.lot_breaks().len(), 1, "{:?}", p.lot_breaks());
        let b = &p.lot_breaks()[0];
        assert!(b.contains("posted 40 of basis"), "{b}");
        assert!(b.contains("costs 10"), "{b}");
        assert!(b.contains("disagree by 30"), "names the gap: {b}");

        // ⚠ A derived model CANNOT enforce this — the journal is the record, and
        // the posted figure is what the trial balance is built from. What it can
        // do is notice, and say which figure it disagrees with.
        assert!(p.nav(&|dim| dim == 1 || dim == 2, &Rates::none()).is_ok(), "and the fund still values");
    }

    #[test]
    fn the_lots_reconcile_with_the_position_they_belong_to() {
        // ⛔ THE CHECK THAT TIES THE TWO HALVES TOGETHER. The position is an
        // aggregate maintained by one path; the lots are a history maintained by
        // another. `Ratio.Lots.aggregate_matches_scan` is the theorem that they
        // must agree, and nothing enforces it structurally — the fold could
        // drift and every other test here would pass.
        let d = book("recon", &[("vti", 100, 10), ("vti", 250, 20), ("voo", 60, 6)]);
        sell(&d, "s1", "vti", 12, 125); // 10 units at 100, then 2 of 20 at 250 → 25
        let p = Projection::of_book(&d).unwrap();

        for (key, held) in &p.positions().value.held {
            let lots = p.lots_of(key.0, &key.1);
            assert_eq!(
                lots.value.iter().map(|l| l.units).sum::<i64>(),
                held.1,
                "units disagree for {key:?}"
            );
            assert_eq!(
                lots.value.iter().map(|l| l.cost).sum::<i64>(),
                held.0,
                "cost disagrees for {key:?}"
            );
        }
    }

    #[test]
    fn a_sale_that_cannot_be_relieved_is_a_break_not_a_failure() {
        // ⛔ A projection that refused to BUILD because one instrument's lots
        // would not divide would take the whole fund down over a line item. The
        // refusal is real — `Ratio.Lots.partial_relief_is_exactly_pro_rata` —
        // and it concerns one position, so it surfaces as a break.
        let d = book("lotbreak", &[("vti", 100, 7)]);
        sell(&d, "s1", "vti", 3, 45);
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.lot_breaks().len(), 1, "{:?}", p.lot_breaks());
        assert!(p.lot_breaks()[0].contains("administration agreement"), "{:?}", p.lot_breaks());
        assert_eq!(p.open_lots(), 1, "and the lot is untouched, not half-relieved");
        assert_eq!(p.lots_of(1, "vti").value[0].units, 7);

        // ⚠ And the NAV still strikes. A break is something an operator looks
        // at, not something that stops the fund being valued.
        assert!(p.nav(&|dim| dim == 1 || dim == 2, &Rates::none()).is_ok());
    }

    #[test]
    fn the_lot_book_advances_with_everything_else() {
        // The incremental property, for lots specifically: catching up in pieces
        // must land where a cold build would.
        let d = book("lotincr", &[("vti", 10, 1), ("vti", 40, 1), ("voo", 20, 2)]);
        sell(&d, "s1", "vti", 1, 50);
        let js = entries(&d);

        let mut piecemeal = Projection::new();
        for n in 1..=js.len() {
            piecemeal.advance(&js[..n], FIFO);
        }
        let cold = Projection::rebuild(&js, FIFO);
        assert_eq!(piecemeal.open_lots(), cold.open_lots());
        assert_eq!(piecemeal.relieved_cost(), cold.relieved_cost());
        assert_eq!(piecemeal.lots_of(1, "vti").value, cold.lots_of(1, "vti").value);
    }

    #[test]
    fn a_projection_holds_the_one_view_every_book_has() {
        // ⛔ ONE, NOT ZERO — AND ZERO IS WHAT `#[derive(Default)]` WOULD GIVE
        // IT, which is why `Default` here is written out. A projection with no
        // book of record is not a state this type has: every book has at least
        // one view, and a book that declares none has exactly one recognising
        // in journal order. `Ratio.Views.nobody_said_is_not_a_settlement_
        // convention`.
        //
        // ⚠ THE ASSERTION THAT `views` HAS ONE ENTRY IS THE ONE THAT MATTERS
        // WHEN THE CUT LANDS. Everything else about this refactor is checked by
        // the three differential tests below going on passing unchanged — the
        // fold moved and the figures did not — and none of them can see how
        // many views there are.
        let p = Projection::new();
        assert_eq!(p.views.len(), 1, "{:?}", p.views.keys().collect::<Vec<_>>());
        assert!(p.views.contains_key(ratio_rules::UNDECLARED_VIEW));
        assert_eq!(p.prefix(), 0);
    }

    #[test]
    fn the_projection_agrees_with_a_full_fold() {
        // ⛔ AGAINST THE SYSTEM OF RECORD, not against itself. A read model
        // that drifts from the journal it derives from is worse than none —
        // it is a second opinion nobody asked for and nobody can adjudicate.
        let d = book("agrees", &[("vti", 25_000, 100), ("voo", 10_000, 40), ("vti", 5_000, 20)]);
        let js = entries(&d);
        let p = Projection::rebuild(&js, FIFO);

        let (held, rest) = FileBook::open(&d).unwrap().positions().unwrap();
        // ⚠ Normalized to compare across the key type. The projection interns
        // its instrument names and `FileBook` — the slow fold this is checked
        // against — does not; the KEYS must still agree, which is the point.
        let projected: BTreeMap<(i64, String), (i64, i64)> = p
            .positions()
            .value
            .held
            .iter()
            .map(|((d, i), v)| ((*d, i.to_string()), *v))
            .collect();
        assert_eq!(projected, held);
        assert_eq!(p.positions().value.rest, rest);
        assert_eq!(p.prefix(), 3);
    }

    #[test]
    fn the_projection_strikes_the_same_nav_as_a_full_fold() {
        // ⭐ AGAINST THE EXISTING PATH, NOT AGAINST ITSELF. `ratio_nav::strike`
        // walks every entry; this reads maintained totals. The whole value of
        // the projection is that the figures are the same number, and a test
        // that compared the projection to another projection would prove only
        // that it is consistent with its own mistake.
        let d = book(
            "navsame",
            &[("vti", 25_000, 100), ("voo", 10_000, 40), ("vti", 5_000, 20)],
        );
        let js = entries(&d);
        let p = Projection::rebuild(&js, FIFO);

        // dims 1 and 2 are assets in `book()`; nothing else is.
        let got = p.nav(&|dim| dim == 1 || dim == 2, &Rates::none()).unwrap();
        let want = ratio_nav::strike(&d, ratio_rules::UNDECLARED_VIEW, 1_782_662_400, "e.marsh").unwrap();

        assert_eq!(got.value.0, want.net_asset_value, "the same NAV");
        assert_eq!(got.value.1, want.trial_balance_difference, "and the same difference");
        assert_eq!(got.prefix, want.journal_position, "over the same prefix");
    }

    #[test]
    fn the_projection_and_the_recorded_strike_agree_across_currencies() {
        // ⛔ THE TEST ABOVE PASSED THROUGH THE ENTIRE DEFECT, and the reason is
        // the shape of its book: one currency, `Rates::none()`. `ratio strike`
        // — the RECORDED nav, the figure a replay re-derives and somebody is
        // paid on — never looked at `PostingRecord::currency` at all. It summed
        // dollars, euros and pounds and labeled the total USD. On a
        // twelve-security generated book it returned the IDENTICAL figure for
        // one currency and for three, and it tied the whole way: trial balance
        // 0, digest reproducible, replay reporting "reproduced".
        //
        // ⚠ SO THE BOOK HERE HOLDS A NON-BASE CURRENCY *OUTSIDE* THE NAV
        // FILTER'S REACH ON ONE SIDE. The first draft of this test bought
        // securities with cash — both legs assets — so every currency netted to
        // zero and the NAV was zero under the fix AND under the bug. It went
        // green on the defect it was written to catch, and only reintroducing
        // the bug on purpose exposed it. Subscriptions are the shape that
        // works: capital is EQUITY, the NAV filter excludes it, and the asset
        // side is left holding 40.00 EUR whose value is 60.00 USD.
        let d = tmp_root().join("ratio-project-navfx");
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
            Account { dim: 3, display_name: "Capital".into(), account_type: A::Equity },
        ])
        .unwrap();
        b.append_record(ratio_store::Plane::Facts, &rate_fact("EUR", 150)).unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        // Each currency conserves on its own: `conserves_every_currency` would
        // refuse anything else.
        for (n, (cur, amount)) in [("USD", 100_00i64), ("EUR", 40_00)].iter().enumerate() {
            b.append(&JournalEntry {
                id: format!("fx{n}"),
                memo: "subscription".into(),
                config: c.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 2,
                        amount: *amount,
                        currency: Some((*cur).into()),
                        instrument: None,
                        quantity: None,
                    },
                    PostingRecord {
                        dim: 3,
                        amount: -*amount,
                        currency: Some((*cur).into()),
                        instrument: None,
                        quantity: None,
                    },
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }

        let rates = Rates::of_facts(
            ratio_store::BASE_CURRENCY,
            &b.records::<ratio_ingest::Fact>(ratio_store::Plane::Facts).unwrap(),
        );
        let p = Projection::rebuild(&entries(&d), FIFO);
        let got = p.nav(&|dim| dim == 1 || dim == 2, &rates).unwrap();
        let want = ratio_nav::strike(&d, ratio_rules::UNDECLARED_VIEW, 1_782_662_400, "e.marsh").unwrap();

        assert_eq!(got.value.0, want.net_asset_value, "the same NAV, translated the same way");
        assert_eq!(
            got.value.1, want.trial_balance_difference,
            "and the same difference"
        );
        // ⭐ AND THE NUMBER ITSELF, stated rather than merely agreed on. Two
        // paths agreeing is worth nothing if they agree on 140.00 — the flat
        // sum — so this pins the translated figure: $100.00 plus €40.00 at
        // 1.50 is $160.00.
        assert_eq!(got.value.0, 160_00, "USD 100.00 + EUR 40.00 at 1.50");
    }

    #[test]
    fn a_strike_refuses_a_currency_it_has_no_rate_for() {
        // ⛔ REFUSES RATHER THAN TREATING IT AS PAR — the same discipline
        // `Rates::none` documents, now on the path that WRITES the number down.
        // A missing rate silently taken as 1.00 reports a fund holding yen at
        // its yen figure: off by two orders of magnitude and shaped like an
        // ordinary number.
        let d = tmp_root().join("ratio-project-navnorate");
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        b.append(&JournalEntry {
            id: "jpy".into(),
            memo: "buy".into(),
            config: c,
            postings: vec![
                PostingRecord {
                    dim: 1,
                    amount: 100_00,
                    currency: Some("JPY".into()),
                    instrument: Some("vti".into()),
                    quantity: Some(1),
                },
                PostingRecord {
                    dim: 2,
                    amount: -100_00,
                    currency: Some("JPY".into()),
                    instrument: None,
                    quantity: None,
                },
            ],
            trade_date: None,
            announcement: None,
        })
        .unwrap();

        let e = ratio_nav::strike(&d, ratio_rules::UNDECLARED_VIEW, 1_782_662_400, "e.marsh").unwrap_err().to_string();
        assert!(e.contains("JPY"), "names the currency it has no rate for: {e}");
    }

    #[test]
    fn the_nav_ignores_dimensions_that_are_not_assets_or_liabilities() {
        // Capital is equity. Including it would net the NAV to zero — the
        // figure would look "balanced" and be worthless, which is the sign
        // error this fold exists to avoid.
        let d = book("navdims", &[("vti", 25_000, 100)]);
        let p = Projection::rebuild(&entries(&d), FIFO);
        assert_eq!(p.nav(&|dim| dim == 1 || dim == 2, &Rates::none()).unwrap().value.0, 0, "buy: asset in, cash out");
        assert_eq!(p.nav(&|dim| dim == 1, &Rates::none()).unwrap().value.0, 25_000, "investments alone");
    }

    #[test]
    fn totals_advance_rather_than_being_recomputed() {
        // The incremental property: catching up in pieces lands where folding
        // the lot from scratch would. `Ratio.Plan.a_stale_total_makes_the_plans
        // _disagree` is what a drifted total would cause.
        let d = book("navincr", &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3)]);
        let js = entries(&d);
        let mut piecemeal = Projection::new();
        for n in 1..=js.len() {
            piecemeal.advance(&js[..n], FIFO);
        }
        let assets = |dim: i64| dim == 1 || dim == 2;
        assert_eq!(piecemeal.nav(&assets, &Rates::none()).unwrap(), Projection::rebuild(&js, FIFO).nav(&assets, &Rates::none()).unwrap());
    }

    #[test]
    fn advancing_twice_folds_each_entry_once() {
        // ⛔ `//tla:rebuild_double_counts_check`. A second advance over the same
        // journal must be a no-op. If it re-folded, the position would stay
        // honest and the contents would double — and nothing about the number
        // would look wrong.
        let d = book("twice", &[("vti", 25_000, 100), ("vti", 5_000, 20)]);
        let js = entries(&d);

        let mut p = Projection::new();
        p.advance(&js, FIFO);
        let once = p.cost_of("vti");
        p.advance(&js, FIFO);
        let twice = p.cost_of("vti");

        assert_eq!(once.value, 30_000);
        assert_eq!(twice, once, "a second advance over the same journal folds nothing");
        assert_eq!(p.prefix(), 2);
    }

    #[test]
    fn advancing_incrementally_equals_rebuilding() {
        // The whole reason `advance` exists: catching up in pieces must land
        // exactly where folding the lot from scratch would.
        let d = book("incr", &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3), ("bnd", 40, 4)]);
        let js = entries(&d);

        let mut piecemeal = Projection::new();
        for n in 1..=js.len() {
            piecemeal.advance(&js[..n], FIFO);
        }
        assert_eq!(piecemeal.positions().value, &Projection::rebuild(&js, FIFO).positions().value.clone());
        assert_eq!(piecemeal.prefix(), js.len());
    }

    #[test]
    fn a_maintained_projection_folds_only_the_delta() {
        // ⭐ STEP 3 OF THE SEAM, AND THE ONE THAT MAKES THE FLAT CURVE REAL IN
        // PRODUCTION. `of_book` rebuilds from zero — O(journal) — which is the
        // cost the benchmark reports as COLD BUILD. A process that keeps a
        // projection pays it once and then folds only what arrived.
        //
        // ⚠ Asserted by COUNT, not by timing. A rebuild fast enough to look
        // incremental would pass a stopwatch and fail this.
        let d = book("delta", &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3)]);
        let js = entries(&d);

        let mut p = Projection::new();
        assert_eq!(p.advance(&js, FIFO), 3, "the first pass folds everything");
        assert_eq!(p.advance(&js, FIFO), 0, "and a second folds nothing at all");
        assert_eq!(p.advance(&js[..2], FIFO), 0, "a SHORTER journal folds nothing either");

        // One more arrives.
        let mut grown = js.clone();
        grown.push(js[0].clone());
        assert_eq!(p.advance(&grown, FIFO), 1, "only the new entry");
        assert_eq!(p.prefix(), 4);
    }

    fn append_one(d: &std::path::Path, id: &str) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "later".into(),
            config: c,
            postings: vec![
                PostingRecord { currency: None, dim: 1, amount: 7, instrument: Some("vti".into()), quantity: Some(1) },
                PostingRecord::new(2, -7),
            ],
            trade_date: None,
            announcement: None,
        })
        .unwrap();
    }

    #[test]
    fn following_a_book_reads_only_what_was_appended() {
        // ⭐ THE PIECE THAT MAKES MAINTENANCE REAL. `entries()` parses the whole
        // journal, so a cached projection built on it pays O(journal) just to
        // learn nothing changed — a rebuild with a cache in front of it.
        // `follow` seeks to where it stopped.
        let d = book("follow", &[("vti", 10, 1), ("voo", 20, 2)]);
        let mut p = Projection::new();
        assert_eq!(p.follow(&d).unwrap(), 2, "first pass folds both");
        assert_eq!(p.follow(&d).unwrap(), 0, "an unchanged book folds nothing");

        append_one(&d, "later-1");
        assert_eq!(p.follow(&d).unwrap(), 1, "only the new entry");
        assert_eq!(p.prefix(), 3);

        // And it lands exactly where a cold build would.
        assert_eq!(p.positions().value, &Projection::of_book(&d).unwrap().positions().value.clone());
        let assets = |dim: i64| dim == 1 || dim == 2;
        assert_eq!(p.nav(&assets, &Rates::none()).unwrap().value, Projection::of_book(&d).unwrap().nav(&assets, &Rates::none()).unwrap().value);
    }

    #[test]
    fn a_journal_that_shrank_is_refused_rather_than_spliced() {
        // ⛔ An append-only log does not shrink, so a shorter file at the same
        // path is a DIFFERENT BOOK. Resuming from the stale offset would splice
        // two histories and fold the result as one — every figure downstream
        // would be built from a mixture nothing could reproduce.
        let d = book("shrank", &[("vti", 10, 1), ("voo", 20, 2), ("bnd", 30, 3)]);
        let mut p = Projection::new();
        p.follow(&d).unwrap();

        // A different, shorter book at the same path.
        let _ = book("shrank", &[("vti", 10, 1)]);
        let err = p.follow(&d).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("does not shrink"), "{msg}");
    }

    #[test]
    fn a_read_carries_the_prefix_it_was_folded_from() {
        // ⭐ `//tla:unpinned_projection_check` as a type rather than a test.
        //
        // The projection lags the journal by two entries. Anything built from
        // this read has ONLY `prefix` to pin — the journal's length is not
        // reachable from an `AsOf`, so pinning the head while reading stale
        // data takes a deliberate act rather than an oversight.
        let d = book("pinned", &[("vti", 10, 1), ("vti", 20, 2), ("vti", 30, 3)]);
        let js = entries(&d);

        let mut p = Projection::new();
        p.advance(&js[..1], FIFO);

        let read = p.cost_of("vti");
        assert_eq!(read.prefix, 1, "what it folded");
        assert_eq!(read.value, 10, "and the value agrees with that prefix, not the journal");
        assert_ne!(read.prefix, js.len(), "the journal has moved on, and the read has not");
        assert!(!p.is_current_with(js.len()));
    }

    #[test]
    fn map_cannot_change_the_prefix() {
        // The prefix is set by the fold and by nothing else. `map` exists so a
        // caller can shape the value without ever getting the chance to restate
        // where it came from.
        let d = book("map", &[("vti", 10, 1)]);
        let p = Projection::rebuild(&entries(&d), FIFO);
        let doubled = p.cost_of("vti").map(|v| v * 2);
        assert_eq!(doubled, AsOf { value: 20, prefix: 1 });
    }

    fn announce(id: &str, inst: &str, num: i64, den: i64, ex: &str, cfg: &ratio_store::Digest) -> JournalEntry {
        JournalEntry {
            id: format!("announce-{id}"),
            memo: String::new(),
            config: cfg.clone(),
            postings: Vec::new(),
            trade_date: None,
            announcement: Some(ratio_store::AnnouncementRecord {
                id: id.into(),
                instrument: inst.into(),
                numerator: num,
                denominator: den,
                ex_date: ex.into(),
                announced: 0,
            }),
        }
    }

    #[test]
    fn an_outstanding_action_is_read_through_and_costs_nothing() {
        // ⭐ THE WHOLE POINT. A 2-for-1 announced and never applied: the stored
        // units are untouched, nothing was rewritten, and the holding reads
        // correctly on any day at or after the ex-date.
        let d = book("open", &[("vti", 25_000, 100)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js, FIFO);

        assert_eq!(p.units_as_of(1, "vti", "2026-01-14").unwrap().value, 100, "before the ex-date");
        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().value, 200, "on and after it");
        assert_eq!(
            p.positions().value.held[&(1, "vti".into())].1,
            100,
            "and the STORED units were never rewritten — that is the saving"
        );
    }

    #[test]
    fn the_ex_date_itself_is_included() {
        // ⛔ THE BOUNDARY, AND MY TESTS DID NOT REACH IT. Two mutation probes
        // survived this file — `<= day` weakened to `< day` passed, because
        // every case used a day strictly before or strictly after. A NAV struck
        // ON the ex-date must include the action: that is what "effective" means
        // and it is the day the market re-priced.
        let d = book("exday", &[("vti", 100, 50)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js, FIFO);

        assert_eq!(p.units_as_of(1, "vti", "2026-01-14").unwrap().value, 50, "the day before");
        assert_eq!(p.units_as_of(1, "vti", "2026-01-15").unwrap().value, 100, "ON the ex-date");
    }

    #[test]
    fn a_split_scales_only_its_own_instrument() {
        // ⛔ ALSO A SURVIVING MUTATION. Dropping the instrument filter entirely
        // left every test green, because no book here held two names — so
        // "steps are per instrument" was asserted nowhere and a split on one
        // holding would have scaled every other.
        let d = book("two", &[("vti", 100, 50), ("voo", 200, 80)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js, FIFO);

        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().value, 100, "split");
        assert_eq!(p.units_as_of(1, "voo", "2026-02-01").unwrap().value, 80, "untouched");
        assert!(p.steps_for("voo", "2026-02-01").is_empty());
    }

    #[test]
    fn an_action_already_rewritten_is_not_read_through_as_well() {
        // ⛔ THE MIGRATION HAZARD. Every book written before the factor path has
        // `action-{id}` entries that already walked the lots. Reading the same
        // split through on top would SQUARE it — 400 units where there are 200 —
        // while the cost stayed put and the trial balance went on tying.
        // `Ratio.Actions.applying_twice_is_not_applying_once`, in a new costume.
        let d = book("rewritten", &[("vti", 25_000, 100)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        // the rewrite: units doubled in the stored figures
        js.push(JournalEntry {
            id: "action-ca-1".into(),
            memo: "applied".into(),
            config: cfg,
            postings: vec![ratio_store::PostingRecord {
                dim: 1,
                amount: 0,
                currency: None,
                instrument: Some("vti".into()),
                quantity: Some(100),
            }],
            trade_date: None,
            announcement: None,
        });
        let p = Projection::rebuild(&js, FIFO);

        assert_eq!(
            p.positions().value.held[&(1, "vti".into())].1,
            200,
            "the rewrite is in the stored units"
        );
        assert!(p.steps_for("vti", "2026-02-01").is_empty(), "so it is NOT read through");
        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().value, 200, "not 400");
    }

    #[test]
    fn a_read_that_would_owe_cash_in_lieu_refuses() {
        // ⚠ Not a bug in the read path. A step that does not divide means the
        // holder was paid cash for a fraction, which realizes a gain and is a
        // posting the configuration has to declare.
        let d = book("lieu", &[("vti", 100, 5)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 3, 2, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js, FIFO);

        assert_eq!(p.units_as_of(1, "vti", "2026-01-14").unwrap().value, 5, "before: fine");
        let err = p.units_as_of(1, "vti", "2026-02-01").unwrap_err();
        assert!(format!("{err:#}").contains("cash in lieu"), "{err:#}");
    }

    #[test]
    fn a_factor_read_still_carries_its_prefix() {
        // The safety property survives the new read path — `AsOf`, not a bare
        // number, because a figure built from this must pin what it folded.
        let d = book("prefixed", &[("vti", 10, 1)]);
        let mut js = entries(&d);
        let cfg = js[0].config.clone();
        js.push(announce("ca-1", "vti", 2, 1, "2026-01-15", &cfg));
        let p = Projection::rebuild(&js, FIFO);
        assert_eq!(p.units_as_of(1, "vti", "2026-02-01").unwrap().prefix, 2);
    }

    #[test]
    fn an_empty_journal_projects_to_nothing_at_position_zero() {
        let p = Projection::rebuild(&[], FIFO);
        assert_eq!(p.prefix(), 0);
        assert_eq!(p.cost_of("vti"), AsOf { value: 0, prefix: 0 });
        assert!(p.is_current_with(0), "current with an empty journal, not stale");
    }

    /// A book whose active configuration declares `method`, holding one cheap
    /// lot and one dear one.
    ///
    /// ⚠ The two costs are 10 and 40 on one unit each, which is
    /// `Ratio.Lots.Methods.the_method_decides_the_taxable_gain`'s own holding.
    /// Lots far apart in cost are what makes the methods DISTINGUISHABLE — a
    /// holding whose lots cost the same is relieved identically by all six, and
    /// a test built on one would pass against any wiring at all.
    fn book_electing(name: &str, method: &str) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let c = b.put(format!("lot_method = \"{method}\"\nrules = []\n").as_bytes()).unwrap();
        b.set_active(&c).unwrap();
        for (n, (cost, qty)) in [(10i64, 1i64), (40, 1)].iter().enumerate() {
            b.append(&JournalEntry {
                id: format!("b{n}"),
                memo: "buy".into(),
                config: c.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: *cost,
                        currency: None,
                        instrument: Some("vti".into()),
                        quantity: Some(*qty),
                    },
                    PostingRecord::new(2, -*cost),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    #[test]
    fn the_declared_method_is_the_method_the_fold_relieves_under() {
        // ⛔ THE TEST THIS ENGINE DID NOT HAVE. `the_configured_method_reaches_
        // the_engine` in relief.rs proves the ENUM MAPPING is faithful; nothing
        // proved the mapping was ever consulted. It was not: `fold_lots` called
        // `relief::relieve`, which is FIFO whatever the fund elected, so a book
        // declaring HIFO was relieved FIFO with every other figure agreeing —
        // the units left were right, the proceeds were right, the trial balance
        // tied, and only the realized gain moved.
        let d = book_electing("elects-hifo", "hifo");
        sell(&d, "s1", "vti", 1, 40);
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.relieved_cost(), 40, "HIFO gives up the DEAR lot, not the old one");
        let left = p.lots_of(1, "vti").value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 10, "the cheap lot is what remains");
        assert!(p.lot_breaks().is_empty(), "{:?}", p.lot_breaks());
    }

    #[test]
    fn electing_a_different_method_relieves_a_different_lot() {
        // The other half of the same claim. One of these passing alone proves
        // nothing — a fold hardcoded to either method passes one of them.
        let d = book_electing("elects-fifo", "fifo");
        sell(&d, "s1", "vti", 1, 10);
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.relieved_cost(), 10, "FIFO gives up the OLD lot");
        assert_eq!(p.lots_of(1, "vti").value[0].cost, 40);
    }

    #[test]
    fn a_method_change_applies_to_the_entries_posted_after_it() {
        // ⛔ THE METHOD IS RESOLVED PER ENTRY, from the config that entry
        // pinned — `//tla:stale_method_relief_check`. A projection holding ONE
        // method would relieve the whole journal under whichever it happened to
        // be handed, and a fund that changed method mid-year would have its
        // earlier sales silently restated.
        let d = book_electing("changes-method", "fifo");
        sell(&d, "s-under-fifo", "vti", 1, 10);

        // A new rule set is promoted. Nothing already posted moves.
        let mut b = FileBook::open(&d).unwrap();
        let hifo = b.put(b"lot_method = \"hifo\"\nrules = []\n").unwrap();
        b.set_active(&hifo).unwrap();
        drop(b);

        // Two more lots, then a sale under the new method.
        for (n, cost) in [(0usize, 30i64), (1, 60)] {
            let mut b = FileBook::open(&d).unwrap();
            b.append(&JournalEntry {
                id: format!("b-after-{n}"),
                memo: "buy".into(),
                config: hifo.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: cost,
                        currency: None,
                        instrument: Some("vti".into()),
                        quantity: Some(1),
                    },
                    PostingRecord::new(2, -cost),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        sell(&d, "s-under-hifo", "vti", 1, 60);

        let p = Projection::of_book(&d).unwrap();
        // 10 relieved under FIFO, then 60 — the dearest — under HIFO.
        assert_eq!(p.relieved_cost(), 70);
        let left: Vec<i64> = p.lots_of(1, "vti").value.iter().map(|l| l.cost).collect();
        assert_eq!(left, vec![40, 30], "the dear lot went, the cheap ones stayed");
        assert!(p.lot_breaks().is_empty(), "{:?}", p.lot_breaks());
    }

    #[test]
    fn a_configuration_that_is_not_a_rule_set_refuses_the_relief() {
        // ⛔ NO FALLBACK TO FIFO. FIFO is a method real funds elect, so a book
        // relieved under it by accident is indistinguishable from one relieved
        // under it by agreement. The sale becomes a break instead.
        let d = tmp_root().join("ratio-project-unreadable-config");
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[Account {
            dim: 1,
            display_name: "Investments".into(),
            account_type: A::Asset,
        }])
        .unwrap();
        let c = b.put(b"lot_method = \"not-a-method\"\n").unwrap();
        b.set_active(&c).unwrap();
        for (id, amount, qty) in [("b0", 10i64, 1i64), ("s0", -10, -1)] {
            b.append(&JournalEntry {
                id: id.into(),
                memo: "trade".into(),
                config: c.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount,
                        currency: None,
                        instrument: Some("vti".into()),
                        quantity: Some(qty),
                    },
                    PostingRecord::new(2, -amount),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        drop(b);

        let p = Projection::of_book(&d).unwrap();
        let breaks = p.lot_breaks();
        assert_eq!(breaks.len(), 1, "{breaks:?}");
        assert!(breaks[0].contains("lot method is not known"), "{}", breaks[0]);
        assert_eq!(p.relieved_cost(), 0, "nothing was relieved under a guess");
        assert_eq!(p.lots_of(1, "vti").value.len(), 1, "the lot is still open");
    }

    #[test]
    fn the_drift_break_names_the_method_it_actually_used() {
        // ⚠ This message said "oldest-first" whatever ran, back when the fold
        // ignored the configuration — so the one line an operator reads to
        // investigate a disagreement asserted the thing that was wrong.
        let d = book_electing("break-names-method", "hifo");
        sell(&d, "s1", "vti", 1, 25); // posts 25 of basis; HIFO relieves 40
        let p = Projection::of_book(&d).unwrap();

        let breaks = p.lot_breaks();
        assert_eq!(breaks.len(), 1, "{breaks:?}");
        assert!(breaks[0].contains("dearest-per-unit-first"), "{}", breaks[0]);
        assert!(!breaks[0].contains("oldest-first"), "{}", breaks[0]);
    }

    // ── what the fund realized, and how much of it is classified ───────────

    const ROLES: ratio_rules::ChartRoles =
        ratio_rules::ChartRoles {
        investments: 1,
        cash: 2,
        realized_gain: 30,
        currency_conversion: None,
    };

    /// A book that posts sales through `relief::sale_postings` — three legs,
    /// with the gain derived rather than supplied.
    fn book_with_gains(name: &str, long_term_days: i64) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
            Account { dim: 30, display_name: "Realized gain".into(), account_type: A::Income },
        ])
        .unwrap();
        let c = b
            .put(
                format!(
                    "lot_method = \"fifo\"\nrules = []\nlong_term_days = {long_term_days}\n\n\
                     [chart_roles]\ninvestments = 1\ncash = 2\nrealized_gain = 30\n"
                )
                .as_bytes(),
            )
            .unwrap();
        b.set_active(&c).unwrap();
        d
    }

    fn buy_on(d: &std::path::Path, id: &str, units: i64, cost: i64, day: &str) {
        buy_dated(d, id, "vti", units, cost, day);
    }

    /// A disposal posted the way the engine posts one: investments out at
    /// basis, cash in at proceeds, the difference to realized gain.
    fn dispose(d: &std::path::Path, id: &str, units: i64, proceeds: i64, day: &str) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        let p = Projection::of_book(d).unwrap();
        let held = p.lots_of(1, "vti").value;
        let r = relief::relieve_by(relief::Method::Fifo, &held, units).unwrap();
        let postings =
            relief::sale_postings(ROLES, None, "vti", units, r.cost, proceeds).unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "sell".into(),
            config: c,
            postings,
            trade_date: Some(day.into()),
            announcement: None,
        })
        .unwrap();
    }

    #[test]
    fn a_gain_is_split_by_how_long_the_lot_was_actually_held() {
        // ⛔ THE REASON THE HOLDING-PERIOD METHODS EXIST. One holding, one
        // trade, and the two halves are taxed at different rates — a report
        // that gives only a total is missing the figure the fund is asked for.
        let d = book_with_gains("split", 365);
        buy_on(&d, "b-old", 1, 100, "2024-01-10"); // held well over a year
        buy_on(&d, "b-new", 1, 100, "2026-06-01"); // held weeks
        dispose(&d, "s-old", 1, 300, "2026-06-30"); // FIFO takes the old lot
        dispose(&d, "s-new", 1, 150, "2026-06-30"); // then the new one

        let p = Projection::of_book(&d).unwrap();
        let r = p.realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();

        // Credit-normal: a gain reads negative. 200 long, 50 short.
        assert_eq!(r.gain, -250);
        assert_eq!(r.long_term, -200, "the lot held since 2024");
        assert_eq!(r.short_term, -50, "the one bought this month");
        assert_eq!(r.unclassified(), 0);
        assert_eq!(r.basis, 200);
    }

    #[test]
    fn the_threshold_is_the_configured_number_and_the_boundary_day_is_long() {
        // ⛔ `365` IS A JURISDICTION'S NUMBER. A fund administered under other
        // rules says so, and the engine must use what it said —
        // `Ratio.Lots.Methods.isLongTerm` takes the threshold as a parameter
        // for exactly this reason.
        //
        // ⚠ AND THE BOUNDARY IS ON THE DAY: held exactly the threshold is LONG.
        // `the_threshold_day_is_long_term`. Off by one moves a disposal between
        // tax rates and nothing about the figure looks unusual.
        let d = book_with_gains("threshold-exact", 365);
        buy_on(&d, "b", 1, 100, "2025-06-30");
        dispose(&d, "s", 1, 300, "2026-06-30"); // exactly 365 days
        let r = Projection::of_book(&d).unwrap().realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.long_term, -200, "the threshold day is long-term");
        assert_eq!(r.short_term, 0);

        // One day short of it is not.
        let d = book_with_gains("threshold-short", 365);
        buy_on(&d, "b", 1, 100, "2025-07-01");
        dispose(&d, "s", 1, 300, "2026-06-30"); // 364 days
        let r = Projection::of_book(&d).unwrap().realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.short_term, -200);
        assert_eq!(r.long_term, 0);

        // And a fund whose agreement says two years gets two years.
        let d = book_with_gains("threshold-730", 730);
        buy_on(&d, "b", 1, 100, "2025-06-30");
        dispose(&d, "s", 1, 300, "2026-06-30"); // 365 days — short, under 730
        let r = Projection::of_book(&d).unwrap().realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.short_term, -200, "365 days is short-term at a 730-day threshold");
        assert_eq!(r.long_term, 0);
    }

    #[test]
    fn a_disposal_whose_proceeds_do_not_divide_is_left_wholly_unclassified() {
        // ⛔ REFUSES RATHER THAN ROUNDING. Two lots of one unit each sharing
        // proceeds of 101 cannot be apportioned exactly, and rounding would move
        // a minor unit between two tax rates — a misstatement of taxable income
        // rather than a rounding error. The TOTAL is untouched.
        let d = book_with_gains("indivisible", 365);
        buy_on(&d, "b1", 1, 40, "2024-01-10");
        buy_on(&d, "b2", 1, 40, "2026-06-01");
        dispose(&d, "s", 2, 101, "2026-06-30");

        let r = Projection::of_book(&d).unwrap().realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.gain, -21, "the total is exact and ties to the chart");
        assert_eq!(r.short_term, 0);
        assert_eq!(r.long_term, 0);
        assert_eq!(r.unclassified(), -21, "all of it, named rather than rounded");
    }

    #[test]
    fn a_lot_with_no_acquisition_date_leaves_its_gain_unclassified() {
        // ⚠ Every journal written before `trade_date` existed is in this state.
        // The honest answer about such a holding's period is that there is not
        // one — assuming the epoch makes it long-term at the favorable rate on
        // records that do not support the claim.
        let d = book_with_gains("undated-gain", 365);
        let mut b = FileBook::open(&d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: "b-undated".into(),
            memo: "buy".into(),
            config: c,
            postings: vec![
                PostingRecord {
                    dim: 1,
                    amount: 100,
                    currency: None,
                    instrument: Some("vti".into()),
                    quantity: Some(1),
                },
                PostingRecord::new(2, -100),
            ],
            trade_date: None,
            announcement: None,
        })
        .unwrap();
        drop(b);
        dispose(&d, "s", 1, 300, "2026-06-30");

        let r = Projection::of_book(&d).unwrap().realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.gain, -200);
        assert_eq!(r.unclassified(), -200);
        assert_eq!(r.short_term + r.long_term, 0);
    }

    #[test]
    fn the_split_always_sums_to_the_total_the_chart_reports() {
        // ⛔ THE INVARIANT THAT MAKES THE THREE FIGURES SAFE TO PRINT TOGETHER.
        // `unclassified` is DERIVED as the remainder, so a classification that
        // silently dropped a disposal shows up as a larger unclassified figure
        // rather than as three numbers that do not add up.
        let d = book_with_gains("sums", 365);
        buy_on(&d, "b1", 2, 200, "2024-01-10");
        buy_on(&d, "b2", 1, 90, "2026-05-01");
        dispose(&d, "s1", 1, 250, "2026-06-30");
        dispose(&d, "s2", 2, 401, "2026-06-30"); // will not divide

        let p = Projection::of_book(&d).unwrap();
        let r = p.realized(Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.short_term + r.long_term + r.unclassified(), r.gain);
        assert!(r.unclassified() != 0, "the indivisible disposal is in there");
    }

    // ── currencies ─────────────────────────────────────────────────────────

    /// A book holding one hundred of the base and ninety of a foreign currency,
    /// in the same asset dimension.
    fn two_currency_book(name: &str) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Cash".into(), account_type: A::Asset },
            Account { dim: 20, display_name: "Capital".into(), account_type: A::Equity },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        for (id, ccy, amount) in [("usd", "USD", 10_000i64), ("eur", "EUR", 9_000)] {
            b.append(&JournalEntry {
                id: id.into(),
                memo: "subscription".into(),
                config: c.clone(),
                postings: vec![
                    PostingRecord::of_currency(1, amount, ccy),
                    PostingRecord::of_currency(20, -amount, ccy),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap();
        }
        d
    }

    #[test]
    fn a_nav_over_several_currencies_is_refused_without_a_rate_for_each() {
        // ⛔ THE FAILURE THIS EXISTS TO PREVENT. `totals.by_dim` used to key on
        // the dimension alone, so this book's NAV was 19,000 — a hundred dollars
        // and ninety euros added together, which is
        // `Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch` in the
        // other direction. Every leg conserved, the trial balance tied, and the
        // figure was in no currency at all.
        let d = two_currency_book("two-ccy-refused");
        let p = Projection::of_book(&d).unwrap();

        let err = p.nav(&|dim| dim == 1, &Rates::of("USD", [])).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("EUR"), "{msg}");
        assert!(msg.contains("mixing denominations"), "{msg}");
    }

    #[test]
    fn given_a_rate_the_nav_is_translated_rather_than_added_up() {
        let d = two_currency_book("two-ccy-translated");
        let p = Projection::of_book(&d).unwrap();

        // EUR at 1.20: 9,000 becomes 10,800, plus 10,000 of base.
        let rates = Rates::of("USD", [("EUR".to_string(), 120)]);
        assert_eq!(p.nav(&|dim| dim == 1, &rates).unwrap().value.0, 20_800);

        // ⛔ AND THE RATE CHANGES THE ANSWER, which is the whole reason it must
        // be stated. At par the same book is 19,000 — the number the old flat
        // sum produced, and it is only right if a euro is worth a dollar.
        let par = Rates::of("USD", [("EUR".to_string(), RATE_SCALE)]);
        assert_eq!(p.nav(&|dim| dim == 1, &par).unwrap().value.0, 19_000);
    }

    #[test]
    fn the_base_currency_needs_no_rate_and_an_untyped_book_still_values() {
        // ⛔ A FUND DOES NOT RECORD WHAT A DOLLAR IS WORTH IN DOLLARS, so there
        // is no rate fact for the base and looking it up would refuse every
        // book that holds any of its own currency.
        let d = two_currency_book("base-at-par");
        let p = Projection::of_book(&d).unwrap();
        let rates = Rates::of("USD", [("EUR".to_string(), 100)]);
        assert!(p.nav(&|dim| dim == 20, &rates).is_ok());

        // And a book written before any of this, whose legs name no currency,
        // values with no rates at all — one group, translated at par.
        let d = book("untyped-values", &[("vti", 10, 1)]);
        let p = Projection::of_book(&d).unwrap();
        assert!(p.nav(&|dim| dim == 1, &Rates::none()).is_ok());
    }

    #[test]
    fn one_copy_of_each_name_however_many_postings_mention_it() {
        // ⛔ THE PROPERTY, NOT THE MECHANISM. Asserting "we call intern()" would
        // pass against an interner that returned a fresh allocation every time.
        // What matters is that a hundred postings naming three instruments
        // leave THREE strings behind, and that is what the fold was not doing:
        // every posting cloned its instrument to build a map key, fourteen
        // million times on the book `ratio bench` measures.
        let d = book(
            "interned",
            &[("vti", 10, 1), ("voo", 20, 2), ("vti", 30, 3), ("bnd", 40, 4), ("vti", 50, 5)],
        );
        let p = Projection::of_book(&d).unwrap();

        // Three instruments. The book's postings carry no currency, so the
        // table holds instruments alone.
        assert_eq!(p.names.len(), 3, "one entry per distinct instrument");

        // ⭐ And the shared copy really is shared: the key in the position map
        // and the key in the lot book are the same allocation, not two equal
        // ones.
        let pos_key = p.positions().value.held.keys().find(|(_, i)| &**i == "vti").unwrap();
        let lot_key = p.lots.open.keys().find(|(_, i)| &**i == "vti").unwrap();
        assert!(std::sync::Arc::ptr_eq(&pos_key.1, &lot_key.1));
    }

    #[test]
    fn a_trade_date_that_is_not_a_date_is_a_break_rather_than_an_absence() {
        // ⚠ LEFT AS `None` IT IS INDISTINGUISHABLE FROM AN ENTRY THAT NEVER
        // CARRIED ONE. The holding-period methods would refuse the holding and
        // nobody would learn why — a data defect wearing the costume of a
        // book written before trade dates existed.
        let d = tmp_root().join("ratio-project-bad-date");
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[Account {
            dim: 1,
            display_name: "Investments".into(),
            account_type: A::Asset,
        }])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        b.append(&JournalEntry {
            id: "b0".into(),
            memo: "buy".into(),
            config: c,
            postings: vec![
                PostingRecord {
                    dim: 1,
                    amount: 100,
                    currency: None,
                    instrument: Some("vti".into()),
                    quantity: Some(1),
                },
                PostingRecord::new(2, -100),
            ],
            trade_date: Some("the fifth of March".into()),
            announcement: None,
        })
        .unwrap();
        drop(b);

        let p = Projection::of_book(&d).unwrap();
        let breaks = p.lot_breaks();
        assert_eq!(breaks.len(), 1, "{breaks:?}");
        assert!(breaks[0].contains("is not a date"), "{}", breaks[0]);
        // And the lot is open with no acquisition date, which the
        // holding-period methods refuse rather than guess about.
        assert_eq!(p.lots_of(1, "vti").value[0].acquired, None);
    }

    #[test]
    fn realized_carries_the_prefix_and_says_nothing_without_a_chart() {
        let d = book_with_gains("no-roles", 365);
        buy_on(&d, "b", 1, 100, "2025-01-10");
        dispose(&d, "s", 1, 300, "2026-06-30");
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.realized(Some(ROLES), &Rates::none()).unwrap().prefix, p.prefix());
        // ⛔ `None`, NOT ZERO. Without a chart the engine does not know which
        // dimension a gain lands in, and zero is a fund that realized nothing.
        assert!(p.realized(None, &Rates::none()).unwrap().value.is_none());
    }

    /// A book of `n` balanced entries, written in ONE `append_all`.
    ///
    /// ⛔ NOT `append` IN A LOOP. That opens and closes the journal per entry —
    /// ~4 ms each, which `ratio_gen` measured and is why `append_all` exists.
    /// Seventy thousand of them would be a five-minute unit test.
    fn wide_book(name: &str, n: usize) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let c = b.put(b"rules = []\n").unwrap();
        b.set_active(&c).unwrap();
        let entries: Vec<JournalEntry> = (0..n)
            .map(|i| JournalEntry {
                id: format!("e{i}"),
                memo: String::new(),
                config: c.clone(),
                postings: vec![PostingRecord::new(1, 100), PostingRecord::new(2, -100)],
                trade_date: None,
                announcement: None,
            })
            .collect();
        b.append_all(&entries).unwrap();
        drop(b);
        d
    }

    #[test]
    fn a_cold_build_reports_the_last_entry_it_folded() {
        let d = wide_book("progress-tail", 3);
        let mut seen = Vec::new();
        let p = Projection::of_book_with_progress(&d, &mut |n| seen.push(n)).unwrap();

        // ⛔ THE TAIL IS THE HALF THAT IS EASY TO LOSE. A fold shorter than one
        // chunk never reaches the in-loop call, so without the report after the
        // walk a small book would fold in complete silence — which is the state
        // this whole callback exists to end.
        assert_eq!(seen.last().copied(), Some(3), "{seen:?}");
        assert_eq!(p.prefix(), 3);
    }

    #[test]
    fn a_long_fold_reports_before_it_finishes_and_the_reports_only_grow() {
        // ⛔ MORE THAN ONE CHUNK, deliberately. At 65,536 the in-loop report
        // fires once and the tail fires once, so this covers the branch that
        // `a_cold_build_reports_the_last_entry_it_folded` cannot reach — a
        // caller watching a 995-second fold is watching exactly this branch.
        const N: usize = 65_536 + 17;
        let d = wide_book("progress-chunked", N);
        let mut seen = Vec::new();
        let p = Projection::of_book_with_progress(&d, &mut |n| seen.push(n)).unwrap();

        assert!(seen.len() >= 2, "one report over {N} entries is not progress: {seen:?}");
        assert_eq!(seen[0], 65_536, "the first report is a whole chunk: {seen:?}");
        assert!(seen.windows(2).all(|w| w[0] < w[1]), "reports went backwards: {seen:?}");
        assert_eq!(seen.last().copied(), Some(N), "{seen:?}");
        assert_eq!(p.prefix(), N);
    }

    #[test]
    fn watching_a_cold_build_does_not_change_what_it_builds() {
        // ⛔ THE PROPERTY THAT MAKES THE HOOK SAFE TO ADD. `of_book` now routes
        // through `of_book_with_progress`, so the two must fold to the same
        // projection over the same book — otherwise instrumenting a fold would
        // have changed the figures it produced, which is the one thing a
        // measurement is not allowed to do.
        let d = wide_book("progress-agrees", 1_000);
        let quiet = Projection::of_book(&d).unwrap();
        let watched = Projection::of_book_with_progress(&d, &mut |_| {}).unwrap();

        assert_eq!(quiet.prefix(), watched.prefix());
        assert_eq!(quiet.positions().value, watched.positions().value);
        assert_eq!(
            quiet.nav(&|d| d == 1 || d == 2, &Rates::none()).unwrap().value,
            watched.nav(&|d| d == 1 || d == 2, &Rates::none()).unwrap().value
        );
    }
}
