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
    /// The book of record this figure is read under.
    ///
    /// ⛔ ON THE VALUE, NOT LEFT TO THE CALLER'S MEMORY. With two views,
    /// `nav("abor")` and `nav("ibor")` are structurally interchangeable — same
    /// types, same shapes, figures close enough to pass a glance. A figure that
    /// carries its view cannot be quoted under the other one without the
    /// substitution being visible in the record.
    pub view: String,
    /// The day this view had recognised through when the figure was read.
    ///
    /// `None` on a view that consults no date (journal order), or on one that
    /// has not yet seen a dated entry. A settlement figure is determined by the
    /// prefix, the view, AND the day — this is the third coordinate, and it is
    /// the fold's own frontier rather than a clock nobody gave it.
    pub through: Option<views::Day>,
}

impl<T> AsOf<T> {
    /// Transform the value, keeping the prefix, the view and the cut. None of
    /// them can be changed by this or any other method — they are set once, by
    /// the fold that produced the figure.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AsOf<U> {
        AsOf { value: f(self.value), prefix: self.prefix, view: self.view, through: self.through }
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
    ///
    /// ⚠ THE SIDES ARE KEPT, NOT JUST THE NET, because the trial-balance screen
    /// shows debit and credit columns PER VIEW — and a net cannot be split back
    /// into its sides. `FileBook::balances_by_dim` answers the same question
    /// only for the whole journal, which is exactly one view's answer.
    pub by_dim: BTreeMap<(i64, Option<Text>), DimTotal>,
    pub debits: i128,
    pub credits: i128,
}

/// One (dimension, currency) row of a view's fold: both sides, and how many
/// postings landed there.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DimTotal {
    pub debit: i128,
    pub credit: i128,
    /// Postings, for the console's per-account counts — a count per VIEW,
    /// since a view recognises a subset of the journal.
    pub postings: i64,
}

impl DimTotal {
    pub fn net(&self) -> i128 {
        self.debit - self.credit
    }
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
        // ⛔ SAME FOLD AS `current_rates`. A NAV that translated at one rate
        // and a screen that cited another would be the books tying on the
        // wrong number, with provenance pointing at the right one.
        Self::of(
            base,
            ratio_ingest::value::current_rates(facts)
                .into_iter()
                .filter_map(|(ccy, f)| Some((ccy, f.values.get("rate")?.as_minor()?))),
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
/// now. The method, the chart roles, the holding-period threshold, the
/// wash window, the min-tax weight and the wash holding-period election
/// are terms of an administration agreement rather than implementation
/// choices, and each decides a REALIZED GAIN — the figure with no
/// counterparty, which no reconciliation reaches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terms {
    /// Which lots a sale gives up.
    pub method: relief::Method,
    /// Which dimensions play which part. `None` when the chart names no roles,
    /// in which case a gain cannot be attributed to a disposal at all.
    pub roles: Option<ratio_rules::ChartRoles>,
    /// Days held for a gain to be long-term. `Ratio.Lots.Methods.isLongTerm`.
    pub long_term_days: i64,
    /// Days either side of a sale a repurchase washes the loss.
    /// `None` means the fund did not elect the rule.
    /// `Ratio.Lots.Wash.inWashWindow`.
    pub wash_window_days: Option<i64>,
    /// Short-term tax weight for min-tax relief.
    /// `None` means the fund did not elect the ranking.
    /// ⛔ NOT A `Method`. `Ratio.Lots.MinTax`.
    pub min_tax_short_weight: Option<i64>,
    /// Whether this book pools the holding.
    /// `None` means the fund did not elect the pool.
    /// ⛔ NOT A `Method`. `Ratio.Lots.AverageCost`.
    pub average_cost: Option<bool>,
    /// Whether a wash replacement keeps its own acquisition date.
    /// `None` means nobody elected the non-US variant — the US
    /// transfer already named in `Ratio.Lots.Wash` stays in force.
    /// ⛔ NOT A `Method`. `Ratio.Lots.WashHolding`.
    pub wash_keep_holding_period: Option<bool>,
}

impl Terms {
    /// The terms a rule set sets.
    pub fn of(set: &ratio_rules::RuleSet) -> Self {
        Self {
            method: set.effective_lot_method().into(),
            roles: set.chart_roles,
            long_term_days: set.long_term_days,
            wash_window_days: set.wash_window_days,
            min_tax_short_weight: set.min_tax_short_weight,
            average_cost: set.average_cost,
            wash_keep_holding_period: set.wash_keep_holding_period,
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

/// One entry in flight between two books of record.
///
/// `Ratio.Views.two_views_differ_by_exactly_what_is_in_flight`: the difference
/// between two views over one journal prefix is a LIST OF ENTRIES, not a
/// number — the number is what the list sums to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InFlightEntry {
    pub id: String,
    pub memo: String,
    /// The day the trade was struck.
    pub trade_day: views::Day,
    /// When `here` recognises it. `None` on a basis that consults no date.
    pub recognised_here: Option<views::Day>,
    /// When `there` recognises it.
    pub recognised_there: Option<views::Day>,
    /// What recognising this entry moves a NAV by, in minor units of the base.
    ///
    /// ⛔ SIGNED AS A CONTRIBUTION TO `difference`, not to either NAV: an entry
    /// `here` counts and `there` does not carries its own effect; the reverse
    /// carries the negation. The two directions sum to the difference exactly.
    ///
    /// ⚠ ZERO FOR A PURCHASE, AND THAT IS THE ENTRY BEHAVING, NOT A BUG: a
    /// purchase moves cash into investments — both assets — so a NAV does not
    /// care when it is recognised. Subscriptions and redemptions are what make
    /// two views actually disagree.
    pub effect: i64,
    /// Which side counts it: `true` when `here` has recognised it and `there`
    /// has not. ⛔ NOT DERIVABLE FROM THE SIGN — a trade's effect is zero in
    /// both directions, and it still belongs to exactly one list.
    pub in_here: bool,
}

/// What accounts for two views' NAVs differing, entry by entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reconciliation {
    pub here: String,
    pub there: String,
    /// Entries one view holds and the other does not, in journal order.
    pub entries: Vec<InFlightEntry>,
    /// Entries NEITHER view can place. ⛔ SHOWN, NOT OMITTED: they contribute
    /// to neither figure, so leaving them off makes a difference look fully
    /// explained when it is not. An entry unplaceable in only ONE view refuses
    /// the whole reconciliation instead — the other view counts it, so the
    /// lists could not account for the difference.
    pub unplaceable: Vec<Unplaced>,
    /// `nav(here) − nav(there)`, and ⛔ EXACTLY the sum of the entries'
    /// signed effects — positive where `here` has recognised and `there` has
    /// not. The test that says so is
    /// `two_views_disagree_about_the_nav_and_the_difference_is_a_list_of_entries`.
    pub difference: i64,
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
    /// A loss that has been realized and not yet matched to a replacement.
    ///
    /// ⛔ THE FORWARD HALF OF THE WINDOW. A repurchase can land after the
    /// sale; at sale time there is nothing to write. Remembering the leftover
    /// is how the write still happens, onto a lot the sale did not take.
    /// `//tla:wash_engine_check`.
    pending_wash: Vec<PendingWash>,
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

/// A sale whose wash window is still open at a named day.
///
/// ⭐ THIS IS WHAT A STRIKE READS TO QUALIFY. The fold remembers leftovers
/// so the write can still land; a strike taken while any of these exist
/// reports a figure that can still move.
/// `Ratio.Lots.WashRestatement.an_open_window_is_qualified`.
///
/// ⚠ A LEFTOVER AFTER THE WINDOW CLOSED IS NOT THIS. `pending_wash` stays
/// until a replacement matches; qualification asks whether the window is
/// still open on the day being struck, not whether a leftover exists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenWashWindow {
    pub sold_on: relief::Day,
    pub window: i64,
    pub remaining_units: i64,
    pub remaining_loss: i64,
}

/// A sale's leftover wash, waiting for a replacement to open.
///
/// Window and terms are the SALE's — pinned by the configuration that entry
/// named, not whatever is in force when the repurchase lands.
#[derive(Clone, Debug)]
struct PendingWash {
    key: (i64, Text),
    window: i64,
    sold_on: relief::Day,
    remaining_units: i64,
    remaining_loss: i64,
    original_acquired: Option<relief::Day>,
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
#[derive(Clone, Debug)]
struct ViewFold {
    positions: Positions,
    totals: Totals,
    lots: LotBook,
    /// How this view dates an entry, as the ACTIVE configuration declares it.
    ///
    /// ⚠ FOR THE READ SIDE ONLY — `AsOf.through` is `None` on a basis that
    /// consults no date. How each ENTRY is recognised still comes from the
    /// digest that entry pinned, never from here.
    basis: ratio_rules::Basis,
    /// The cut: the highest trade day the fold has seen.
    ///
    /// ⛔ THE JOURNAL'S OWN CLOCK, NOT THE HIGHEST RECOGNITION DAY. Advancing
    /// this to the highest day an entry was PLACED on would drain the band the
    /// moment anything entered it — the latest-settling entry always settles
    /// last, so everything would fold and the views would agree at every
    /// prefix, which is `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_
    /// gap` rebuilt one layer up. The trade days are the evidence of what day
    /// it is; the band holds what is placed beyond them.
    ///
    /// ⛔ MONOTONIC. A late entry carrying an old trade date folds immediately
    /// (its day is inside the cut) but never moves the cut backward —
    /// `advance` using `max`, one layer out.
    recognised_through: Option<views::Day>,
    /// The band: entries placed beyond the cut, keyed by recognition day.
    ///
    /// ⛔ BOUNDED BY THE SETTLEMENT LAG ONLY BECAUSE THE CUT MOVES AS THE FOLD
    /// READS. A cold build over twenty years against a cut that never advanced
    /// would hold the whole journal here — the failure design 2 was rejected
    /// for, reached by a different road. A journal is roughly chronological,
    /// so with the cut tracking the trade days this holds days of entries
    /// rather than years: `the_pending_queue_is_bounded_by_the_settlement_lag_
    /// not_by_the_journal`.
    pending: BTreeMap<views::Day, Vec<Pending>>,
    /// Entries this view can never date, each with the reason.
    ///
    /// ⛔ REPORTED, NEVER SILENTLY DROPPED — and never folded as `recorded`
    /// either, for the reason a bad config refuses the relief rather than
    /// falling back to FIFO. `Placement::Unplaceable` is not "not yet":
    /// an entry the view recognises on Tuesday is in the band; an entry it
    /// cannot date at all is here.
    ///
    /// ⚠ THE ENTRY, NOT JUST THE SENTENCE. A reconciliation shows what neither
    /// view can place as a third list — omitted, a difference looks fully
    /// explained when it is not — and a list needs the id and the memo, which
    /// a prose reason has already thrown away.
    unplaceable: Vec<Unplaced>,
}

impl Default for ViewFold {
    fn default() -> Self {
        Self {
            positions: Positions::default(),
            totals: Totals::default(),
            lots: LotBook::default(),
            basis: ratio_rules::Basis::Recorded,
            recognised_through: None,
            pending: BTreeMap::new(),
            unplaceable: Vec::new(),
        }
    }
}

/// One entry the fold has read but a view has not yet recognised.
///
/// ⚠ A CLONE OF THE ENTRY, AND THE BAND IS WHY THAT IS AFFORDABLE: it holds
/// the last few days' trades, not the journal — retaining every entry to
/// filter on read is exactly the design the band replaced.
#[derive(Clone, Debug)]
struct Pending {
    at: usize,
    entry: JournalEntry,
    terms: Result<Terms, String>,
    trade_day: views::Day,
}

/// One entry a view can never recognise, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unplaced {
    /// Empty on the one refusal that is about the VIEW rather than an entry —
    /// a view declared after this projection had already folded a prefix.
    pub id: String,
    pub memo: String,
    /// `None` when the record carries none, which is usually why it is here.
    pub trade_day: Option<views::Day>,
    pub why: String,
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
    /// The views each configuration declares, resolved once per digest.
    ///
    /// ⛔ FROM THE DIGEST AN ENTRY PINNED, NEVER FROM ACTIVE, cached exactly as
    /// `terms` is and for the same two reasons — the fold is per entry, and how
    /// an entry is recognised is a term of the agreement in force when it was
    /// posted. A settlement date re-derived over a calendar amended since is
    /// `//tla:calendar_in_side_file_check`. Which views EXIST is the active
    /// configuration's question; this map answers the other one.
    view_defs: BTreeMap<ratio_store::Digest, Result<Vec<views::ViewDef>, String>>,
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
            view_defs: BTreeMap::new(),
        }
    }
}

impl Projection {
    /// One view's fold, or a refusal that names the views this book keeps.
    ///
    /// ⛔ A REFUSAL, NOT A FALLBACK. A caller asking for a view this book does
    /// not keep is holding a URL, a flag, or a guess — and answering with the
    /// default view's figures would be the substitution this whole feature
    /// exists to prevent. The error lists what the book actually keeps, which
    /// is what the caller needs to correct with.
    fn fold_of(&self, view: &str) -> Result<&ViewFold> {
        self.views.get(view).ok_or_else(|| {
            anyhow::anyhow!(
                "no view {view:?} on this book. It keeps: {}",
                self.views.keys().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
            )
        })
    }

    /// The cut a figure read from this view carries: `None` on a basis that
    /// consults no date, the frontier otherwise.
    fn through_of(fold: &ViewFold) -> Option<views::Day> {
        match fold.basis {
            ratio_rules::Basis::Recorded => None,
            _ => fold.recognised_through,
        }
    }

    /// The views this projection folds, in id order.
    pub fn views(&self) -> Vec<&str> {
        self.views.keys().map(|k| k.as_str()).collect()
    }

    /// Entries a view has read but not yet recognised — the band, flattened.
    pub fn in_flight(&self, view: &str) -> Result<usize> {
        Ok(self.fold_of(view)?.pending.values().map(Vec::len).sum())
    }

    /// Entries this view can never date, each with the reason.
    pub fn unplaceable(&self, view: &str) -> Result<&[Unplaced]> {
        Ok(&self.fold_of(view)?.unplaceable)
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
        // Shared, once per entry: an announcement is a fact about an
        // instrument, not a posting in scope of a recognition convention, which
        // is why `actions` lives on `Projection` rather than on a view.
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
        // ⛔ PARSED ONCE PER ENTRY, NOT ONCE PER VIEW. Two views over seven
        // million entries must not mean fourteen million date parses — `follow`
        // pays `parse` once and runs N per-view bodies, which is
        // `//tla:views_check`'s `EveryViewFoldsTheSamePrefix` by construction.
        //
        // ⚠ A DATE THAT WILL NOT PARSE IS A BREAK, NOT A SILENT ABSENCE. Left
        // as `None` it is indistinguishable from an entry that never carried
        // one. The break lands in the lot book of every view that folds the
        // entry; a view that refuses the entry outright reports it in
        // `unplaceable` instead, which is the sharper message.
        let (trade_day, date_break): (Option<relief::Day>, Option<String>) =
            match entry.trade_date.as_deref() {
                None => (None, None),
                Some(d) => match ratio_common::days_from_iso_date(d) {
                    Ok(n) => (Some(n as relief::Day), None),
                    Err(e) => (
                        None,
                        Some(format!(
                            "{}: trade date {d:?} is not a date — {e:#}. Lots it opens carry \
                             no acquisition date, and the holding-period methods refuse them",
                            entry.id
                        )),
                    ),
                },
            };
        // ⛔ THE INTERNER AND THE FOLDS ARE BORROWED SEPARATELY, ONCE, OUTSIDE
        // THE LOOP. Reaching through `self` for each in turn would borrow the
        // whole thing twice per posting; destructuring splits the borrow at no
        // cost, which is the same reason `names` exists at all.
        let Self { names, views, cost, view_defs, .. } = self;
        for (vid, vf) in views.iter_mut() {
            let placement = Self::placement_of(view_defs, vid, entry, trade_day);
            match placement {
                views::Placement::Always => {
                    Self::apply(vf, names, cost, at, entry, terms, trade_day, date_break.as_deref())
                }
                views::Placement::On(d) => {
                    // ⚠ `<=`, AND THE BOUNDARY IS ON THE DAY — an entry
                    // recognised exactly at the cut is IN, matching
                    // `ViewDef::recognises` and `the_settlement_day_itself_is_
                    // recognised`.
                    if vf.recognised_through.is_some_and(|c| d <= c) {
                        Self::apply(vf, names, cost, at, entry, terms, trade_day, date_break.as_deref())
                    } else {
                        vf.pending.entry(d).or_default().push(Pending {
                            at,
                            entry: entry.clone(),
                            terms: terms.clone(),
                            trade_day: trade_day
                                .expect("an entry placed on a day carries the date that placed it"),
                        });
                    }
                }
                views::Placement::Unplaceable(why) => vf.unplaceable.push(Unplaced {
                    id: entry.id.clone(),
                    memo: entry.memo.clone(),
                    trade_day,
                    why,
                }),
            }
            // ⛔ THE CUT ADVANCES ON THE TRADE DAY — THE JOURNAL'S OWN CLOCK —
            // AND THE BAND DRAINS UP TO IT. Advancing on the day entries are
            // PLACED would drain the band the moment anything entered it, and
            // every view would agree at every prefix. The trade days are the
            // fold's only evidence of what day it is; an entry that settles
            // beyond the latest of them is exactly what "in flight" means.
            if let Some(t) = trade_day {
                if vf.recognised_through.is_none_or(|c| t > c) {
                    vf.recognised_through = Some(t);
                    loop {
                        match vf.pending.first_key_value() {
                            Some((&d, _)) if d <= t => {
                                let (_, batch) =
                                    vf.pending.pop_first().expect("the key was just seen");
                                // ⚠ RECOGNITION ORDER, WHICH IS THE VIEW'S OWN
                                // ORDER: by day, journal order within a day.
                                // The lot ordinals stay honest because each
                                // pending entry kept its journal position.
                                for pe in batch {
                                    Self::apply(
                                        vf,
                                        names,
                                        cost,
                                        pe.at,
                                        &pe.entry,
                                        &pe.terms,
                                        Some(pe.trade_day),
                                        None,
                                    );
                                }
                            }
                            _ => break,
                        }
                    }
                }
            }
        }
    }

    /// Where one view puts one entry, resolved from the digest THAT ENTRY
    /// pinned — never from the active configuration.
    ///
    /// ⛔ THE ONE SPELLING OF THE DECISION. `fold` asks it to decide what lands
    /// in a view's figures; `recognised` asks it so a walk over the journal
    /// can filter to exactly those entries. Two spellings would be two doors
    /// with one law, and HANDOFF.md records what that costs here.
    ///
    /// ⛔ AN ENTRY WHOSE PINNED CONFIGURATION DOES NOT DECLARE THE VIEW IS
    /// REFUSED, never folded as `recorded`: that would put an entry in a
    /// settlement NAV no settlement convention admitted, and the figure would
    /// tie the whole way — the same argument that stops an unreadable config
    /// falling back to FIFO.
    fn placement_of(
        view_defs: &BTreeMap<ratio_store::Digest, Result<Vec<views::ViewDef>, String>>,
        vid: &str,
        entry: &JournalEntry,
        trade_day: Option<relief::Day>,
    ) -> views::Placement {
        match view_defs.get(&entry.config) {
            Some(Ok(defs)) => match defs.iter().find(|v| v.id == vid) {
                Some(def) => def.placement(&entry.id, trade_day),
                None => views::Placement::Unplaceable(format!(
                    "{}: the configuration this entry pinned ({}) declares no view \
                     {:?}, so this view cannot say when it is recognised",
                    entry.id,
                    entry.config.as_str(),
                    vid
                )),
            },
            // ⚠ AN UNREADABLE CONFIG REFUSES A DECLARED VIEW AND NOT THE
            // UNDECLARED ONE, and the asymmetry is the point. A settlement
            // date is a term of the pinned agreement — no readable config, no
            // calendar, no date. Journal order is a term of NOTHING: it is the
            // custom every book has, `ViewDef::undeclared` answers over every
            // book ever written, and an entry with a broken config still
            // FOLDED before views existed — only its RELIEF refused, per sale,
            // with a break naming why. Refusing the whole entry here turned
            // `a_configuration_that_is_not_a_rule_set_refuses_the_relief`'s
            // one lot break into an empty book.
            Some(Err(_)) if vid == ratio_rules::UNDECLARED_VIEW => views::Placement::Always,
            Some(Err(why)) => views::Placement::Unplaceable(format!(
                "{}: the configuration this entry pinned ({}) could not be read — \
                 {why}",
                entry.id,
                entry.config.as_str()
            )),
            // A slice fold has no book to resolve digests from. The one view
            // every book has consults no date, so its placement needs none;
            // any other view got into this map from a configuration a slice
            // cannot see, and guessing would be folding under terms nobody
            // read.
            None if vid == ratio_rules::UNDECLARED_VIEW => views::Placement::Always,
            None => views::Placement::Unplaceable(format!(
                "{}: this fold was advanced from a slice, which carries no \
                 configurations to say when view {:?} recognises it",
                entry.id, vid
            )),
        }
    }

    /// Whether one view's figures hold this entry, at the fold's own cut.
    ///
    /// ⛔ FOR WALKS THE PROJECTION DOES NOT RETAIN. `list_postings` reads the
    /// journal directly — the fold keeps totals, not history — so a per-view
    /// posting list must skip exactly what the view has not recognised.
    /// Filtering with anything but the fold's own decision would let the list
    /// and the totals disagree about which entries a figure holds.
    pub fn recognised(&self, view: &str, entry: &JournalEntry) -> Result<bool> {
        let fold = self.fold_of(view)?;
        let trade_day = entry
            .trade_date
            .as_deref()
            .and_then(|d| ratio_common::days_from_iso_date(d).ok())
            .map(|n| n as relief::Day);
        Ok(match Self::placement_of(&self.view_defs, view, entry, trade_day) {
            views::Placement::Always => true,
            views::Placement::On(d) => fold.recognised_through.is_some_and(|c| d <= c),
            views::Placement::Unplaceable(_) => false,
        })
    }

    /// One view's trial-balance rows: `(dimension, currency)` → both sides and
    /// the posting count, off the maintained fold.
    ///
    /// ⛔ PER VIEW, WHICH `FileBook::balances_by_dim` IS NOT: the file's answer
    /// sums the whole journal, and that is exactly one view's answer wearing
    /// no label. The console's trial balance reads this.
    pub fn balances(
        &self,
        view: &str,
    ) -> Result<AsOf<&BTreeMap<(i64, Option<Text>), DimTotal>>> {
        let fold = self.fold_of(view)?;
        Ok(AsOf {
            value: &fold.totals.by_dim,
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
        })
    }

    /// Apply one recognised entry to one view's fold: the totals, the
    /// positions, and the lot book.
    ///
    /// ⛔ THE ONLY PLACE AN ENTRY LANDS IN A FOLD. `fold` reaches here directly
    /// for an entry inside the cut and through the band for one beyond it —
    /// two application paths would be two chances to disagree about what an
    /// entry means, and the disagreement would be a NAV that depended on
    /// whether an entry ever sat in the band.
    #[allow(clippy::too_many_arguments)]
    fn apply(
        fold: &mut ViewFold,
        names: &mut ratio_common::intern::Interner,
        cost: &mut FoldCost,
        at: usize,
        entry: &JournalEntry,
        terms: &Result<Terms, String>,
        trade_day: Option<relief::Day>,
        date_break: Option<&str>,
    ) {
        if let Some(b) = date_break {
            fold.lots.breaks.push(b.to_string());
        }
        Self::apply_lots(fold, names, cost, at, entry, terms, trade_day);
        for p in &entry.postings {
            // ⚠ `-p.amount` is not the magnitude at the floor: `-i64::MIN`
            // overflows. Widened first, it does not.
            let amount = p.amount as i128;
            // ⛔ THE LAST PER-POSTING ALLOCATION. Five distinct currency codes
            // across seven million entries; cloning the code to look up a row
            // that already exists is the same waste the instrument key was.
            let ccy = p.currency.as_deref().map(|c| names.intern(c));
            let row = fold.totals.by_dim.entry((p.dim, ccy)).or_default();
            row.postings += 1;
            if amount >= 0 {
                row.debit += amount;
                fold.totals.debits += amount;
            } else {
                row.credit += -amount;
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

        // ⛔ TWO DIFFERENT QUESTIONS, TWO DIFFERENT SOURCES. Which views EXIST
        // comes from the ACTIVE configuration, read here, once per follow. How
        // each ENTRY is recognised comes from the digest that entry pinned,
        // resolved per digest inside the walk. Conflating them re-derives
        // settlement dates over whatever calendar is in force now, which is
        // `//tla:calendar_in_side_file_check`.
        {
            use ratio_store::ConfigStore;
            let active = match book.active()? {
                Some(d) => ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(
                    &book.get(&d)?,
                ))
                .unwrap_or_default(),
                None => ratio_rules::RuleSet::default(),
            };
            self.seed_views(&active);
        }

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
            if !self.view_defs.contains_key(&entry.config) {
                let resolved = Self::views_of(&book, &entry.config);
                self.view_defs.insert(entry.config.clone(), resolved);
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

    /// The views a stored configuration declares, for recognising the entries
    /// that pinned it. Cached by digest in `view_defs`, exactly as `terms` is.
    fn views_of(
        book: &FileBook,
        config: &ratio_store::Digest,
    ) -> Result<Vec<views::ViewDef>, String> {
        use ratio_store::ConfigStore;
        let bytes = book
            .get(config)
            .map_err(|e| format!("config {} could not be read — {e:#}", config.as_str()))?;
        let set = ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&bytes))
            .map_err(|e| format!("config {} is not a rule set — {e:#}", config.as_str()))?;
        Ok(views::ViewDef::of(&set))
    }

    /// Make the fold map answer for exactly the views the ACTIVE configuration
    /// declares.
    ///
    /// ⛔ A VIEW DECLARED AFTER ENTRIES WERE FOLDED CANNOT BE CONJURED. A
    /// maintained fold has read past everything it would need — so the view
    /// appears, but refusing, until a rebuild folds it from the start. Folding
    /// only from here would be a book of record missing its history, and the
    /// trial balance would tie on the fragment.
    ///
    /// ⚠ A view REMOVED from the active configuration keeps its fold: the
    /// entries were recognised under terms they pinned, and the figures remain
    /// answerable. Listing what a fund offers is the console's question and it
    /// reads the active configuration, not this map.
    fn seed_views(&mut self, active: &ratio_rules::RuleSet) {
        let declared = views::ViewDef::of(active);
        if self.at == 0 {
            self.views = declared
                .iter()
                .map(|d| (d.id.clone(), ViewFold { basis: d.basis, ..ViewFold::default() }))
                .collect();
            return;
        }
        let folded = self.at;
        for d in declared {
            self.views.entry(d.id.clone()).or_insert_with(|| {
                let mut vf = ViewFold { basis: d.basis, ..ViewFold::default() };
                vf.unplaceable.push(Unplaced {
                    id: String::new(),
                    memo: String::new(),
                    trade_day: None,
                    why: format!(
                        "view {:?} was declared after this projection had folded {folded} \
                         entries, and a maintained fold cannot recognise what it has \
                         already read past — rebuild to fold this view from the start",
                        d.id
                    ),
                });
                vf
            });
        }
    }

    /// The positions, as of the prefix folded, under one book of record.
    ///
    /// ⛔ THE ONLY WAY OUT OF THIS TYPE, and it hands back the prefix, the view
    /// and the cut with the value. There is deliberately no
    /// `fn positions(&self) -> &Positions` — and deliberately no default view
    /// parameter: with two views the figures are structurally interchangeable,
    /// so the caller SAYS which book of record it is asking about, every time.
    pub fn positions(&self, view: &str) -> Result<AsOf<&Positions>> {
        let fold = self.fold_of(view)?;
        Ok(AsOf {
            value: &fold.positions,
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
        })
    }

    /// Total cost held in one instrument, across every account.
    pub fn cost_of(&self, view: &str, instrument: &str) -> Result<AsOf<i64>> {
        let fold = self.fold_of(view)?;
        Ok(AsOf {
            value: fold
                .positions
                .held
                .iter()
                .filter(|((_, i), _)| &**i == instrument)
                .map(|(_, (cost, _))| *cost)
                .sum(),
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
        })
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
    fn apply_lots(
        fold: &mut ViewFold,
        names: &mut ratio_common::intern::Interner,
        cost: &mut FoldCost,
        at: usize,
        entry: &JournalEntry,
        terms: &Result<Terms, String>,
        trade_day: Option<relief::Day>,
    ) {
        // ⛔ THE GAIN IS ATTRIBUTED ONLY WHEN THE ENTRY IS UNAMBIGUOUS: exactly
        // one sale, and a chart that names where a gain goes. An entry disposing
        // of two instruments carries ONE gain leg between them, and splitting it
        // would be inventing an attribution the journal does not record.
        //
        // ⚠ Ambiguous entries are not dropped — they land in
        // `Realized::unclassified`, which is the total MINUS what was
        // classified, so nothing can go missing without showing up there.
        //
        // ⚠ `trade_day` WAS PARSED ONCE, IN `fold`, so N views cost one parse —
        // and a date that would not parse arrives as `None` here with the break
        // already recorded by `apply`.
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
                // ⛔ PENDING IS TAKEN OUT so the holding and the leftover
                // list are not borrowed together. A repurchase that matches
                // writes the deferral onto this new lot — the forward half
                // of `Ratio.Lots.Wash.the_window_reaches_backwards_too`.
                let mut pending = std::mem::take(&mut fold.lots.pending_wash);
                let held = fold.lots.open.entry(key.clone()).or_default();
                // ⛔ A HUSK IS REFUSED WHERE IT IS OFFERED, so the walk no longer
                // rescans the whole holding on every sale looking for one.
                if let Err(e) = held.push(lot.clone()) {
                    fold.lots.breaks.push(format!("{}: {e:#}", entry.id));
                    fold.lots.pending_wash = pending;
                    continue;
                }
                if let Err(e) = match_pending_washes(
                    held,
                    &mut pending,
                    &key,
                    lot.seq,
                    qty,
                    trade_day,
                ) {
                    fold.lots.breaks.push(format!("{}: {e:#}", entry.id));
                }
                fold.lots.pending_wash = pending;
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
            let leftover = {
                let held = fold.lots.open.entry(key.clone()).or_default();
                let relieving = std::time::Instant::now();
                // ⛔ SPECID IS NOT A METHOD, AND NEITHER IS MINTAX. A named
                // selection is an attested per-sale choice; a ranking takes
                // a PRICE. Treating either as `held.relieve(method, …)` is
                // the TLA probe that goes red.
                let relieved = if let Some(named) = &entry.identified_lots {
                    if terms.min_tax_short_weight.is_some() {
                        Err(anyhow::anyhow!(
                            "this sale names lots for specific identification and its \
                             configuration elects min-tax. Two answers for one sale. \
                             Drop identified_lots, or drop min_tax_short_weight. \
                             See Ratio.Lots.SpecId"
                        ))
                    } else if terms.average_cost == Some(true) {
                        Err(anyhow::anyhow!(
                            "this sale names lots for specific identification and its \
                             configuration elects average cost. Two answers for one sale. \
                             Drop identified_lots, or drop average_cost. \
                             See Ratio.Lots.AverageCost"
                        ))
                    } else {
                        held.relieve_spec_id(-qty, named)
                    }
                } else if let Some(weight) = terms.min_tax_short_weight {
                    min_tax_sale(held, entry, &terms, trade_day, -qty, weight)
                } else if terms.average_cost == Some(true) {
                    // ⛔ NOT `held.relieve(method, …)`. Pooling is not a
                    // sort-and-walk. `//tla:sort_and_walk_average_cost_check`
                    // is the engine that pretends otherwise.
                    held.relieve_average_cost(-qty)
                } else {
                    held.relieve(method, -qty)
                };
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
                            let how = if entry.identified_lots.is_some() {
                                "specific-identification"
                            } else if terms.min_tax_short_weight.is_some() {
                                "min-tax-at-the-sale-price"
                            } else if terms.average_cost == Some(true) {
                                "average-cost-pooled-basis"
                            } else {
                                method.describe()
                            };
                            fold.lots.breaks.push(format!(
                                "{}: selling {} of {} posted {} of basis, and relieving the lots \
                                 {} costs {} — the position and the lot book \
                                 will disagree by {}",
                                entry.id,
                                -qty,
                                inst,
                                -p.amount,
                                how,
                                r.cost,
                                -p.amount - r.cost
                            ));
                        }
                        let ccy = p.currency.as_deref().map(|c| names.intern(c));
                        *fold.lots.relieved.entry(ccy.clone()).or_default() += r.cost as i128;
                        // The wash write, if any. ⛔ BOTH HALVES: disallow the
                        // loss AND attach it to an open replacement. A first-half
                        // engine conserves and permanently overtaxes.
                        // `Ratio.Lots.Wash.disallowing_without_attaching_
                        // destroys_the_loss`.
                        let attached = match try_wash_sale(
                            held,
                            &r,
                            -qty,
                            gain_leg,
                            trade_day,
                            terms,
                        ) {
                            Ok(w) => w,
                            Err(e) => {
                                fold.lots.breaks.push(format!("{}: {e:#}", entry.id));
                                None
                            }
                        };
                        // ⚠ CLASSIFY THE POSTED GAIN, NOT THE WASHED ONE.
                        // The chart total is the journal's realized-gain
                        // dimension. Restating the split against a figure
                        // the journal did not post makes `unclassified` absorb
                        // every deferral — which is how a generated book
                        // that never elected wash stopped partitioning.
                        // Qualification / restatement of the posted figure
                        // is `Ratio.Lots.WashRestatement`, and it is not
                        // this fold — a restatement cites the strike, it
                        // does not rewrite the journal's posted gain.
                        let split = gain_leg.and_then(|g| classify(&r, g, trade_day, terms));
                        if let Some((short, long)) = split {
                            *fold.lots.short_term.entry(ccy.clone()).or_default() += short;
                            *fold.lots.long_term.entry(ccy).or_default() += long;
                        }
                        match (attached, trade_day, terms.wash_window_days) {
                            (Some(w), Some(sold_on), Some(window))
                                if w.remaining_units > 0 && w.remaining_loss < 0 =>
                            {
                                Some(PendingWash {
                                    key: key.clone(),
                                    window,
                                    sold_on,
                                    remaining_units: w.remaining_units,
                                    remaining_loss: w.remaining_loss,
                                    original_acquired: relief::acquired_write(
                                        terms.wash_keep_holding_period == Some(true),
                                        r.taken.iter().filter_map(|t| t.acquired).min(),
                                    ),
                                })
                            }
                            _ => None,
                        }
                    }
                    Err(e) => {
                        fold.lots.breaks.push(format!(
                            "{}: selling {} of {} could not be relieved — {e:#}",
                            entry.id, -qty, inst
                        ));
                        None
                    }
                }
            };
            if let Some(w) = leftover {
                fold.lots.pending_wash.push(w);
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
        view: &str,
        roles: Option<ratio_rules::ChartRoles>,
        rates: &Rates,
    ) -> Result<AsOf<Option<Realized>>> {
        let fold = self.fold_of(view)?;
        let value = match roles {
            None => None,
            Some(r) => Some(Realized {
                gain: Self::translate(fold, &|dim| dim == r.realized_gain, rates)?,
                basis: convert(&fold.lots.relieved, rates)?,
                short_term: convert(&fold.lots.short_term, rates)?,
                long_term: convert(&fold.lots.long_term, rates)?,
            }),
        };
        Ok(AsOf {
            value,
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
        })
    }

    /// Sales whose wash window is still open on `as_of`.
    ///
    /// ⭐ THE VALUE A STRIKE READS TO QUALIFY. Empty means nothing can still
    /// move this prefix's realized gain; nonempty means the strike must say
    /// so. `Ratio.Lots.WashRestatement`.
    ///
    /// ⛔ `as_of` IS REQUIRED. Guessing the day from the view's frontier
    /// would qualify (or not) a strike against a clock nobody named.
    pub fn open_wash_windows(
        &self,
        view: &str,
        as_of: relief::Day,
    ) -> Result<AsOf<Vec<OpenWashWindow>>> {
        let fold = self.fold_of(view)?;
        let mut out = Vec::new();
        for w in &fold.lots.pending_wash {
            if relief::window_still_open(w.window, w.sold_on, as_of)? {
                out.push(OpenWashWindow {
                    sold_on: w.sold_on,
                    window: w.window,
                    remaining_units: w.remaining_units,
                    remaining_loss: w.remaining_loss,
                });
            }
        }
        Ok(AsOf {
            value: out,
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
        })
    }

    /// The open lots of one position, oldest first.
    pub fn lots_of(&self, view: &str, dim: i64, instrument: &str) -> Result<AsOf<Vec<relief::Lot>>> {
        let fold = self.fold_of(view)?;
        Ok(AsOf {
            value: fold
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
            view: view.to_string(),
            through: Self::through_of(fold),
        })
    }

    /// How many open lots this fund holds, across every position.
    ///
    /// ⛔ THE NUMBER THE SCALE ARGUMENT IS ABOUT, and it is deliberately NOT in
    /// `nav`. `Ratio.Closure.factored_nav_never_reads_the_lots` is the claim
    /// that this figure does not appear in a NAV's cost, and having it available
    /// here is what lets that be checked rather than asserted.
    pub fn open_lots(&self, view: &str) -> Result<i64> {
        Ok(self.fold_of(view)?.lots.open.values().map(|v| v.len() as i64).sum())
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
    pub fn currency_count(&self, view: &str) -> Result<i64> {
        Ok(self
            .fold_of(view)?
            .totals
            .by_dim
            .keys()
            .map(|(_, c)| c.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as i64)
    }

    /// Rows the maintained NAV actually walks: one per (dimension, currency).
    ///
    /// ⛔ NOT `Ratio.Closure.markCost`, WHICH IS THE SECURITIES. The model
    /// charges one read per security; `translate` walks this map. Quoting either
    /// as the other is how an estimate stops being checkable against the thing
    /// it estimates — which is the only reason to have both numbers.
    pub fn total_rows(&self, view: &str) -> Result<i64> {
        Ok(self.fold_of(view)?.totals.by_dim.len() as i64)
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
    pub fn relieved_cost(&self, view: &str) -> Result<i128> {
        Ok(self.fold_of(view)?.lots.relieved.values().sum())
    }

    /// Sales that could not be relieved, and why.
    pub fn lot_breaks(&self, view: &str) -> Result<&[String]> {
        Ok(&self.fold_of(view)?.lots.breaks)
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
        view: &str,
        is_asset_or_liability: &dyn Fn(i64) -> bool,
        rates: &Rates,
    ) -> Result<AsOf<(i64, i64)>> {
        let fold = self.fold_of(view)?;
        let nav = Self::translate(fold, &|dim| is_asset_or_liability(dim), rates)?;
        // ⛔ A figure that cannot be represented is REFUSED rather than
        // truncated. `Ratio.Bounded`: an operation agrees with the theorem or
        // declines, and there is no third answer.
        Ok(AsOf {
            value: (
                i64::try_from(nav).map_err(|_| {
                    anyhow::anyhow!("this fund's net asset value does not fit in 64 bits")
                })?,
                i64::try_from(fold.totals.debits - fold.totals.credits).map_err(|_| {
                    anyhow::anyhow!("this fund's trial-balance difference does not fit in 64 bits")
                })?,
            ),
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
        })
    }

    /// What accounts for two views' NAVs differing, entry by entry.
    ///
    /// ⛔ BOUNDED BY THE SETTLEMENT LAG, NOT BY THE JOURNAL: this walks the two
    /// bands, which hold the last few days' trades, never the history. It is
    /// `Ratio.Views.two_views_differ_by_exactly_what_is_in_flight` as a read —
    /// the difference IS a list of entries, and the number is what the list
    /// sums to.
    ///
    /// ⛔ THREE REFUSALS, EACH THE HONEST ANSWER:
    /// - a view this book does not keep — `fold_of`'s refusal, naming what it
    ///   keeps;
    /// - a view with unplaceable entries — those contribute to NEITHER figure,
    ///   so the difference cannot be fully accounted for, and a list that looks
    ///   complete while missing them is worse than no list;
    /// - a translation residue — integer translation does not distribute over
    ///   a sum, so on a multi-currency book the per-entry effects can differ
    ///   from the NAV difference by a minor unit per bucket. Refused with the
    ///   residue named, rather than published as an account that does not add
    ///   up or silently adjusted so it does.
    pub fn reconcile(
        &self,
        here: &str,
        there: &str,
        is_asset_or_liability: &dyn Fn(i64) -> bool,
        rates: &Rates,
    ) -> Result<AsOf<Reconciliation>> {
        let a = self.fold_of(here)?;
        let b = self.fold_of(there)?;
        // ⛔ UNPLACEABLE-IN-ONE REFUSES; UNPLACEABLE-IN-BOTH IS THE THIRD LIST.
        // An entry only one view can place is counted by that view and missing
        // from the other's band, so no list of in-flight entries can account
        // for the difference — refusing is the only answer that does not
        // publish an account that does not add up. An entry NEITHER view can
        // place contributes to neither figure; it is reported, not refused.
        fn ids(f: &ViewFold) -> std::collections::BTreeSet<&str> {
            f.unplaceable.iter().map(|u| u.id.as_str()).collect()
        }
        let (un_a, un_b) = (ids(a), ids(b));
        for (name, mine, theirs) in [(here, &a.unplaceable, &un_b), (there, &b.unplaceable, &un_a)]
        {
            if let Some(u) = mine.iter().find(|u| !theirs.contains(u.id.as_str())) {
                anyhow::bail!(
                    "view {name:?} cannot place {:?} and the other view counts it, so the \
                     difference cannot be accounted for entry by entry: {}",
                    u.id,
                    u.why
                );
            }
        }
        let unplaceable: Vec<Unplaced> = a
            .unplaceable
            .iter()
            .filter(|u| un_b.contains(u.id.as_str()))
            .cloned()
            .collect();
        // The bands, keyed by journal position — which both views share,
        // because one pass feeds every view.
        fn band(f: &ViewFold) -> BTreeMap<usize, (&Pending, views::Day)> {
            f.pending
                .iter()
                .flat_map(|(d, batch)| batch.iter().map(move |p| (p.at, (p, *d))))
                .collect()
        }
        let pending_here = band(a);
        let pending_there = band(b);
        // Journal order, one merged walk: an entry pending in BOTH bands is in
        // NEITHER fold, so it accounts for nothing and is skipped.
        let mut entries = Vec::new();
        let mut summed: i128 = 0;
        for (at, (p, day)) in &pending_there {
            if pending_here.contains_key(at) {
                continue;
            }
            // `here` has recognised it and `there` has not: it adds its effect.
            let effect = Self::effect_of(&p.entry, is_asset_or_liability, rates)?;
            summed += i128::from(effect);
            entries.push((
                *at,
                InFlightEntry {
                    id: p.entry.id.clone(),
                    memo: p.entry.memo.clone(),
                    trade_day: p.trade_day,
                    recognised_here: self.recognition_of(here, &p.entry, p.trade_day),
                    recognised_there: Some(*day),
                    effect,
                    in_here: true,
                },
            ));
        }
        for (at, (p, day)) in &pending_here {
            if pending_there.contains_key(at) {
                continue;
            }
            let effect = Self::effect_of(&p.entry, is_asset_or_liability, rates)?;
            summed -= i128::from(effect);
            entries.push((
                *at,
                InFlightEntry {
                    id: p.entry.id.clone(),
                    memo: p.entry.memo.clone(),
                    trade_day: p.trade_day,
                    recognised_here: Some(*day),
                    recognised_there: self.recognition_of(there, &p.entry, p.trade_day),
                    effect: -effect,
                    in_here: false,
                },
            ));
        }
        entries.sort_by_key(|(at, _)| *at);
        let nav_here = Self::translate(a, &|d| is_asset_or_liability(d), rates)?;
        let nav_there = Self::translate(b, &|d| is_asset_or_liability(d), rates)?;
        let difference = nav_here - nav_there;
        if difference != summed {
            anyhow::bail!(
                "the NAVs differ by {difference} and the in-flight entries sum to {summed} — \
                 a translation residue of {} minor units, because integer translation does \
                 not distribute over a sum. The difference cannot be accounted for entry by \
                 entry at these rates",
                difference - summed
            );
        }
        Ok(AsOf {
            value: Reconciliation {
                here: here.to_string(),
                there: there.to_string(),
                entries: entries.into_iter().map(|(_, e)| e).collect(),
                unplaceable,
                difference: i64::try_from(difference).map_err(|_| {
                    anyhow::anyhow!("the difference between these views does not fit in 64 bits")
                })?,
            },
            prefix: self.at,
            view: here.to_string(),
            through: Self::through_of(a),
        })
    }

    /// When one view recognises an entry, from the configuration THAT ENTRY
    /// pinned. `None` is journal order — the basis that consults no date.
    fn recognition_of(
        &self,
        view: &str,
        entry: &JournalEntry,
        trade_day: views::Day,
    ) -> Option<views::Day> {
        match self.view_defs.get(&entry.config) {
            Some(Ok(defs)) => defs.iter().find(|v| v.id == view).and_then(|d| {
                match d.placement(&entry.id, Some(trade_day)) {
                    views::Placement::On(day) => Some(day),
                    _ => None,
                }
            }),
            _ => None,
        }
    }

    /// What recognising one entry moves a NAV by, in minor units of the base.
    fn effect_of(
        entry: &JournalEntry,
        want: &dyn Fn(i64) -> bool,
        rates: &Rates,
    ) -> Result<i64> {
        let mut total: i128 = 0;
        for p in &entry.postings {
            if !want(p.dim) {
                continue;
            }
            let factor = rates.factor(p.currency.as_deref()).ok_or_else(|| {
                anyhow::anyhow!(
                    "this entry posts {} and no rate for it was supplied — a difference \
                     mixing denominations is not a figure",
                    p.currency.as_deref().unwrap_or("an untyped balance")
                )
            })?;
            total += p.amount as i128 * factor as i128 / RATE_SCALE as i128;
        }
        i64::try_from(total)
            .map_err(|_| anyhow::anyhow!("this entry's effect does not fit in 64 bits"))
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
    fn translate(fold: &ViewFold, want: &dyn Fn(i64) -> bool, rates: &Rates) -> Result<i128> {
        let mut total: i128 = 0;
        for ((dim, currency), row) in &fold.totals.by_dim {
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
            total += row.net() * factor as i128 / RATE_SCALE as i128;
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
    pub fn units_as_of(
        &self,
        view: &str,
        dim: i64,
        instrument: &str,
        day: &str,
    ) -> Result<AsOf<i64>> {
        let fold = self.fold_of(view)?;
        let stored = fold
            .positions
            .held
            .get(&(dim, Text::from(instrument)))
            .map(|(_, q)| *q)
            .unwrap_or(0);
        Ok(AsOf {
            value: ratio_ingest::factor::units_as_of(stored, &self.steps_for(instrument, day))?,
            prefix: self.at,
            view: view.to_string(),
            through: Self::through_of(fold),
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

/// Apply the wash write after a sale, if a loss and a dated window exist.
///
/// ⛔ BOTH HALVES. Disallowing without attaching is
/// `Ratio.Lots.Wash.disallowing_without_attaching_destroys_the_loss`.
/// The search is over the holding — the remainder — so a Taken lot is
/// unreachable. `Ratio.Lots.Wash.attaching_cannot_write_a_lot_the_sale_took`.
/// Rank and relieve one sale under MinTax. `Ratio.Lots.MinTax`.
///
/// ⛔ THE PRICE IS THE CASH POSTING, not the posted basis. The basis is
/// what the ranking decides; using it as the price would be circular and
/// would make every sale look like a sale at cost.
fn min_tax_sale(
    held: &mut relief::Holding,
    entry: &JournalEntry,
    terms: &Terms,
    trade_day: Option<relief::Day>,
    want: i64,
    short_weight: i64,
) -> Result<relief::Relief> {
    let Some(day) = trade_day else {
        anyhow::bail!(
            "min-tax relief needs the sale's trade date to classify lots short or \
             long, and this entry has none — assuming today or the epoch would pick \
             a rate the records do not support"
        );
    };
    let Some(roles) = terms.roles else {
        anyhow::bail!(
            "min-tax relief needs the sale PRICE, which is the cash posting, and \
             this configuration names no chart roles — there is no cash dimension \
             to read a price from"
        );
    };
    let cash: Vec<i64> = entry
        .postings
        .iter()
        .filter(|p| p.dim == roles.cash)
        .map(|p| p.amount)
        .collect();
    if cash.is_empty() {
        anyhow::bail!(
            "min-tax relief needs the sale PRICE and this entry has no cash posting \
             — a ranking without a price is a sort, and Ratio.Lots.MinTax is not a \
             sort"
        );
    }
    let proceeds = cash.into_iter().try_fold(0i64, |acc, a| {
        ratio_common::checked::add(acc, a, "the sale's proceeds")
    })?;
    let price = relief::unit_price(proceeds, want)?;
    held.relieve_min_tax(want, price, short_weight, terms.long_term_days, day)
}

fn try_wash_sale(
    held: &mut relief::Holding,
    r: &relief::Relief,
    sold_units: i64,
    gain_leg: Option<i64>,
    sale_day: Option<relief::Day>,
    terms: Terms,
) -> Result<Option<relief::WashMatch>> {
    let Some(window) = terms.wash_window_days else {
        return Ok(None);
    };
    let Some(g) = gain_leg else {
        return Ok(None);
    };
    let Some(sale_day) = sale_day else {
        return Ok(None);
    };
    // `sale_postings` is credit-normal: a loss is a POSITIVE gain leg.
    // `disallowed` takes the Relief sign — negative when money was lost.
    let loss = ratio_common::checked::neg(g, "the realized loss")?;
    if loss >= 0 {
        return Ok(None);
    }
    let original = r.taken.iter().filter_map(|t| t.acquired).min();
    let write = relief::acquired_write(terms.wash_keep_holding_period == Some(true), original);
    Ok(Some(relief::wash_open(
        held,
        loss,
        sold_units,
        sale_day,
        window,
        write,
        &r.taken,
    )?))
}

/// Match a newly opened lot against leftover washes of the same position.
///
/// The window, and the leftover loss, are the SALE's — stored on
/// [`PendingWash`] from the configuration that sale pinned.
fn match_pending_washes(
    held: &mut relief::Holding,
    pending: &mut Vec<PendingWash>,
    key: &(i64, Text),
    replacement_seq: u64,
    bought_units: i64,
    buy_day: Option<relief::Day>,
) -> Result<()> {
    let Some(buy_day) = buy_day else {
        return Ok(());
    };
    let mut leftover = Vec::new();
    let mut units_left = bought_units;
    for mut w in pending.drain(..) {
        if w.key != *key
            || units_left <= 0
            || !relief::in_wash_window(w.window, w.sold_on, buy_day)
        {
            leftover.push(w);
            continue;
        }
        let (_, remaining_units, remaining_loss) = relief::wash_purchase(
            held,
            replacement_seq,
            units_left,
            w.remaining_loss,
            w.remaining_units,
            w.original_acquired,
        )?;
        units_left -= w.remaining_units - remaining_units;
        w.remaining_units = remaining_units;
        w.remaining_loss = remaining_loss;
        if w.remaining_units > 0 && w.remaining_loss < 0 {
            leftover.push(w);
        }
    }
    *pending = leftover;
    Ok(())
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

    /// The one view every book that declares none has — which is every book
    /// these tests seed, so every read below names it. The tests that declare
    /// views name theirs.
    use ratio_rules::UNDECLARED_VIEW as B;

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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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

        let lots = p.lots_of(B, 1, "vti").unwrap().value;
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
        let lots = p.lots_of(B, 1, "vti").unwrap().value;
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

        assert_eq!(p.open_lots(B).unwrap(), 1, "one lot left");
        let left = p.lots_of(B, 1, "vti").unwrap();
        assert_eq!(left.value[0].units, 1);
        assert_eq!(left.value[0].cost, 40, "the DEAR lot survives, not the cheap one");
        assert_eq!(p.relieved_cost(B).unwrap(), 10, "and 10 of basis was given up");
        assert!(p.lot_breaks(B).unwrap().is_empty());
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

        assert_eq!(p.lot_breaks(B).unwrap().len(), 1, "{:?}", p.lot_breaks(B).unwrap());
        let b = &p.lot_breaks(B).unwrap()[0];
        assert!(b.contains("posted 40 of basis"), "{b}");
        assert!(b.contains("costs 10"), "{b}");
        assert!(b.contains("disagree by 30"), "names the gap: {b}");

        // ⚠ A derived model CANNOT enforce this — the journal is the record, and
        // the posted figure is what the trial balance is built from. What it can
        // do is notice, and say which figure it disagrees with.
        assert!(p.nav(B, &|dim| dim == 1 || dim == 2, &Rates::none()).is_ok(), "and the fund still values");
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

        for (key, held) in &p.positions(B).unwrap().value.held {
            let lots = p.lots_of(B, key.0, &key.1).unwrap();
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

        assert_eq!(p.lot_breaks(B).unwrap().len(), 1, "{:?}", p.lot_breaks(B).unwrap());
        assert!(p.lot_breaks(B).unwrap()[0].contains("administration agreement"), "{:?}", p.lot_breaks(B).unwrap());
        assert_eq!(p.open_lots(B).unwrap(), 1, "and the lot is untouched, not half-relieved");
        assert_eq!(p.lots_of(B, 1, "vti").unwrap().value[0].units, 7);

        // ⚠ And the NAV still strikes. A break is something an operator looks
        // at, not something that stops the fund being valued.
        assert!(p.nav(B, &|dim| dim == 1 || dim == 2, &Rates::none()).is_ok());
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
        assert_eq!(piecemeal.open_lots(B).unwrap(), cold.open_lots(B).unwrap());
        assert_eq!(piecemeal.relieved_cost(B).unwrap(), cold.relieved_cost(B).unwrap());
        assert_eq!(piecemeal.lots_of(B, 1, "vti").unwrap().value, cold.lots_of(B, 1, "vti").unwrap().value);
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
            .positions(B).unwrap()
            .value
            .held
            .iter()
            .map(|((d, i), v)| ((*d, i.to_string()), *v))
            .collect();
        assert_eq!(projected, held);
        assert_eq!(p.positions(B).unwrap().value.rest, rest);
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
        let got = p.nav(B, &|dim| dim == 1 || dim == 2, &Rates::none()).unwrap();
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
            })
            .unwrap();
        }

        let rates = Rates::of_facts(
            ratio_store::BASE_CURRENCY,
            &b.records::<ratio_ingest::Fact>(ratio_store::Plane::Facts).unwrap(),
        );
        let p = Projection::rebuild(&entries(&d), FIFO);
        let got = p.nav(B, &|dim| dim == 1 || dim == 2, &rates).unwrap();
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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
        assert_eq!(p.nav(B, &|dim| dim == 1 || dim == 2, &Rates::none()).unwrap().value.0, 0, "buy: asset in, cash out");
        assert_eq!(p.nav(B, &|dim| dim == 1, &Rates::none()).unwrap().value.0, 25_000, "investments alone");
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
        assert_eq!(piecemeal.nav(B, &assets, &Rates::none()).unwrap(), Projection::rebuild(&js, FIFO).nav(B, &assets, &Rates::none()).unwrap());
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
        let once = p.cost_of(B, "vti").unwrap();
        p.advance(&js, FIFO);
        let twice = p.cost_of(B, "vti").unwrap();

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
        assert_eq!(piecemeal.positions(B).unwrap().value, &Projection::rebuild(&js, FIFO).positions(B).unwrap().value.clone());
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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
        assert_eq!(p.positions(B).unwrap().value, &Projection::of_book(&d).unwrap().positions(B).unwrap().value.clone());
        let assets = |dim: i64| dim == 1 || dim == 2;
        assert_eq!(p.nav(B, &assets, &Rates::none()).unwrap().value, Projection::of_book(&d).unwrap().nav(B, &assets, &Rates::none()).unwrap().value);
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

        let read = p.cost_of(B, "vti").unwrap();
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
        let doubled = p.cost_of(B, "vti").unwrap().map(|v| v * 2);
        assert_eq!(doubled, AsOf { value: 20, prefix: 1, view: B.to_string(), through: None });
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
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
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

        assert_eq!(p.units_as_of(B, 1, "vti", "2026-01-14").unwrap().value, 100, "before the ex-date");
        assert_eq!(p.units_as_of(B, 1, "vti", "2026-02-01").unwrap().value, 200, "on and after it");
        assert_eq!(
            p.positions(B).unwrap().value.held[&(1, "vti".into())].1,
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

        assert_eq!(p.units_as_of(B, 1, "vti", "2026-01-14").unwrap().value, 50, "the day before");
        assert_eq!(p.units_as_of(B, 1, "vti", "2026-01-15").unwrap().value, 100, "ON the ex-date");
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

        assert_eq!(p.units_as_of(B, 1, "vti", "2026-02-01").unwrap().value, 100, "split");
        assert_eq!(p.units_as_of(B, 1, "voo", "2026-02-01").unwrap().value, 80, "untouched");
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
        });
        let p = Projection::rebuild(&js, FIFO);

        assert_eq!(
            p.positions(B).unwrap().value.held[&(1, "vti".into())].1,
            200,
            "the rewrite is in the stored units"
        );
        assert!(p.steps_for("vti", "2026-02-01").is_empty(), "so it is NOT read through");
        assert_eq!(p.units_as_of(B, 1, "vti", "2026-02-01").unwrap().value, 200, "not 400");
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

        assert_eq!(p.units_as_of(B, 1, "vti", "2026-01-14").unwrap().value, 5, "before: fine");
        let err = p.units_as_of(B, 1, "vti", "2026-02-01").unwrap_err();
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
        assert_eq!(p.units_as_of(B, 1, "vti", "2026-02-01").unwrap().prefix, 2);
    }

    #[test]
    fn an_empty_journal_projects_to_nothing_at_position_zero() {
        let p = Projection::rebuild(&[], FIFO);
        assert_eq!(p.prefix(), 0);
        assert_eq!(p.cost_of(B, "vti").unwrap(), AsOf { value: 0, prefix: 0, view: B.to_string(), through: None });
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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

        assert_eq!(p.relieved_cost(B).unwrap(), 40, "HIFO gives up the DEAR lot, not the old one");
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 10, "the cheap lot is what remains");
        assert!(p.lot_breaks(B).unwrap().is_empty(), "{:?}", p.lot_breaks(B).unwrap());
    }

    #[test]
    fn electing_a_different_method_relieves_a_different_lot() {
        // The other half of the same claim. One of these passing alone proves
        // nothing — a fold hardcoded to either method passes one of them.
        let d = book_electing("elects-fifo", "fifo");
        sell(&d, "s1", "vti", 1, 10);
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.relieved_cost(B).unwrap(), 10, "FIFO gives up the OLD lot");
        assert_eq!(p.lots_of(B, 1, "vti").unwrap().value[0].cost, 40);
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
            })
            .unwrap();
        }
        sell(&d, "s-under-hifo", "vti", 1, 60);

        let p = Projection::of_book(&d).unwrap();
        // 10 relieved under FIFO, then 60 — the dearest — under HIFO.
        assert_eq!(p.relieved_cost(B).unwrap(), 70);
        let left: Vec<i64> = p.lots_of(B, 1, "vti").unwrap().value.iter().map(|l| l.cost).collect();
        assert_eq!(left, vec![40, 30], "the dear lot went, the cheap ones stayed");
        assert!(p.lot_breaks(B).unwrap().is_empty(), "{:?}", p.lot_breaks(B).unwrap());
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
            })
            .unwrap();
        }
        drop(b);

        let p = Projection::of_book(&d).unwrap();
        let breaks = p.lot_breaks(B).unwrap();
        assert_eq!(breaks.len(), 1, "{breaks:?}");
        assert!(breaks[0].contains("lot method is not known"), "{}", breaks[0]);
        assert_eq!(p.relieved_cost(B).unwrap(), 0, "nothing was relieved under a guess");
        assert_eq!(p.lots_of(B, 1, "vti").unwrap().value.len(), 1, "the lot is still open");
    }

    #[test]
    fn the_drift_break_names_the_method_it_actually_used() {
        // ⚠ This message said "oldest-first" whatever ran, back when the fold
        // ignored the configuration — so the one line an operator reads to
        // investigate a disagreement asserted the thing that was wrong.
        let d = book_electing("break-names-method", "hifo");
        sell(&d, "s1", "vti", 1, 25); // posts 25 of basis; HIFO relieves 40
        let p = Projection::of_book(&d).unwrap();

        let breaks = p.lot_breaks(B).unwrap();
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
    ///
    /// ⛔ RELIEVES UNDER THE METHOD THE BOOK ELECTED. Hard-coding FIFO here
    /// is how a `--method hifo` generator declared HIFO and posted FIFO
    /// gains — the fold then disagreed with every sale.
    fn dispose(d: &std::path::Path, id: &str, units: i64, proceeds: i64, day: &str) {
        let b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        let set = ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&c).unwrap()))
            .unwrap();
        let p = Projection::of_book(d).unwrap();
        let held = p.lots_of(B, 1, "vti").unwrap().value;
        let as_of = ratio_common::days_from_iso_date(day).unwrap() as relief::Day;
        let r = if let Some(w) = set.min_tax_short_weight {
            let price = relief::unit_price(proceeds, units).unwrap();
            relief::relieve_min_tax(&held, units, price, w, set.long_term_days, as_of).unwrap()
        } else if set.average_cost == Some(true) {
            relief::relieve_average_cost(&held, units).unwrap()
        } else {
            relief::relieve_by(relief::Method::from(set.effective_lot_method()), &held, units)
                .unwrap()
        };
        dispose_relieved(d, id, units, proceeds, day, r, None);
    }

    fn dispose_identified(
        d: &std::path::Path,
        id: &str,
        units: i64,
        proceeds: i64,
        day: &str,
        named: &[u64],
    ) {
        let p = Projection::of_book(d).unwrap();
        let held = p.lots_of(B, 1, "vti").unwrap().value;
        let r = relief::relieve_spec_id(&held, units, named).unwrap();
        dispose_relieved(d, id, units, proceeds, day, r, Some(named.to_vec()));
    }

    fn dispose_relieved(
        d: &std::path::Path,
        id: &str,
        units: i64,
        proceeds: i64,
        day: &str,
        r: relief::Relieved,
        identified: Option<Vec<u64>>,
    ) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        let postings =
            relief::sale_postings(ROLES, None, "vti", units, r.cost, proceeds).unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "sell".into(),
            config: c,
            postings,
            trade_date: Some(day.into()),
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: identified,
            special_allocations: None,
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
        let r = p.realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();

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
        let r = Projection::of_book(&d).unwrap().realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.long_term, -200, "the threshold day is long-term");
        assert_eq!(r.short_term, 0);

        // One day short of it is not.
        let d = book_with_gains("threshold-short", 365);
        buy_on(&d, "b", 1, 100, "2025-07-01");
        dispose(&d, "s", 1, 300, "2026-06-30"); // 364 days
        let r = Projection::of_book(&d).unwrap().realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.short_term, -200);
        assert_eq!(r.long_term, 0);

        // And a fund whose agreement says two years gets two years.
        let d = book_with_gains("threshold-730", 730);
        buy_on(&d, "b", 1, 100, "2025-06-30");
        dispose(&d, "s", 1, 300, "2026-06-30"); // 365 days — short, under 730
        let r = Projection::of_book(&d).unwrap().realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
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

        let r = Projection::of_book(&d).unwrap().realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
        })
        .unwrap();
        drop(b);
        dispose(&d, "s", 1, 300, "2026-06-30");

        let r = Projection::of_book(&d).unwrap().realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
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
        let r = p.realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.short_term + r.long_term + r.unclassified(), r.gain);
        assert!(r.unclassified() != 0, "the indivisible disposal is in there");
    }

    // ── wash sales, on a real book ─────────────────────────────────────────

    fn book_with_wash(name: &str, wash_window_days: i64, method: &str) -> std::path::PathBuf {
        book_with_wash_terms(name, wash_window_days, method, "")
    }

    fn book_with_wash_keep(name: &str, wash_window_days: i64, method: &str) -> std::path::PathBuf {
        book_with_wash_terms(
            name,
            wash_window_days,
            method,
            "wash_keep_holding_period = true\n",
        )
    }

    fn book_with_wash_terms(
        name: &str,
        wash_window_days: i64,
        method: &str,
        extra: &str,
    ) -> std::path::PathBuf {
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
                    "lot_method = \"{method}\"\nrules = []\nlong_term_days = 365\n\
                     wash_window_days = {wash_window_days}\n{extra}\n\
                     [chart_roles]\ninvestments = 1\ncash = 2\nrealized_gain = 30\n"
                )
                .as_bytes(),
            )
            .unwrap();
        b.set_active(&c).unwrap();
        d
    }

    #[test]
    fn an_in_window_wash_raises_the_replacement_basis_on_the_book() {
        // ⭐ THE ENGINE ON A REAL BOOK. Buy, top up inside the window, sell the
        // original at a loss: forty of a hundred shares bought back defers 400
        // of a 1000 loss onto the replacement.
        let d = book_with_wash("wash-in", 30, "fifo");
        buy_on(&d, "orig", 100, 2000, "2026-01-01");
        buy_on(&d, "repl", 40, 500, "2026-06-10");
        dispose(&d, "s", 100, 1000, "2026-06-15");

        let p = Projection::of_book(&d).unwrap();
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 900, "500 + 400 of deferred loss");
        assert_eq!(left[0].acquired, Some(day("2026-01-01")), "the period transferred");
        assert!(p.lot_breaks(B).unwrap().is_empty(), "{:?}", p.lot_breaks(B).unwrap());

        // A later sale of the replacement takes the adjusted basis.
        dispose(&d, "s2", 40, 500, "2026-07-01");
        let p = Projection::of_book(&d).unwrap();
        assert_eq!(p.relieved_cost(B).unwrap(), 2000 + 900);
        assert!(p.lots_of(B, 1, "vti").unwrap().value.is_empty());
    }

    #[test]
    fn an_out_of_window_repurchase_does_not_raise_the_book() {
        let d = book_with_wash("wash-out", 30, "fifo");
        buy_on(&d, "orig", 100, 2000, "2026-01-01");
        buy_on(&d, "repl", 40, 500, "2026-04-01"); // 75 days before the sale
        dispose(&d, "s", 100, 1000, "2026-06-15");

        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left[0].cost, 500, "untouched — outside the window");
        assert_eq!(left[0].acquired, Some(day("2026-04-01")), "and the period did not transfer");
    }

    #[test]
    fn a_narrower_window_is_a_different_answer() {
        // ⛔ THE WINDOW IS CONFIGURATION. Twenty-five days is inside 30 and
        // outside 10. Same two dates, two books, two answers.
        // `Ratio.Lots.Wash.the_window_is_a_jurisdiction_number`.
        let wide = book_with_wash("wash-wide", 30, "fifo");
        buy_on(&wide, "orig", 100, 2000, "2026-01-01");
        dispose(&wide, "s", 100, 1000, "2026-06-15");
        buy_on(&wide, "repl", 100, 1000, "2026-07-10"); // 25 days later
        assert_eq!(
            Projection::of_book(&wide).unwrap().lots_of(B, 1, "vti").unwrap().value[0].cost,
            2000,
            "the whole loss attached under a 30-day window"
        );

        let narrow = book_with_wash("wash-narrow", 10, "fifo");
        buy_on(&narrow, "orig", 100, 2000, "2026-01-01");
        dispose(&narrow, "s", 100, 1000, "2026-06-15");
        buy_on(&narrow, "repl", 100, 1000, "2026-07-10");
        assert_eq!(
            Projection::of_book(&narrow).unwrap().lots_of(B, 1, "vti").unwrap().value[0].cost,
            1000,
            "twenty-five days is outside a ten-day window"
        );
    }

    #[test]
    fn choosing_the_wrong_wash_period_rule_flips_the_later_rate() {
        // ⭐ THE ENGINE ON A REAL BOOK. Same trades, two elections, two rates.
        // `Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate`.
        //
        // Buy 1 @ 2000 on 2026-01-01; sell at 1000 on 2026-10-28; buy the
        // replacement on 2026-11-01; sell it at 1500 on 2027-02-05.
        // The first sale is a 1000 loss, washed in full onto the replacement
        // (basis 2000). The later sale realises a 500 loss.
        // Transfer (US, unset): replacement acquired 2026-01-01 → long.
        // Keep (elected): replacement acquired 2026-11-01 → short.
        // Conservation and the trial balance are identical either way.
        let us = book_with_wash("wash-us-hp", 30, "fifo");
        buy_on(&us, "orig", 1, 2000, "2026-01-01");
        dispose(&us, "s", 1, 1000, "2026-10-28");
        buy_on(&us, "repl", 1, 1000, "2026-11-01");
        let left = Projection::of_book(&us).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left[0].cost, 2000, "1000 + 1000 of deferred loss");
        assert_eq!(left[0].acquired, Some(day("2026-01-01")), "the period transferred");
        dispose(&us, "s2", 1, 1500, "2027-02-05");
        let p = Projection::of_book(&us).unwrap();
        assert!(p.lot_breaks(B).unwrap().is_empty(), "{:?}", p.lot_breaks(B).unwrap());
        let r = p.realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.long_term, 500, "the later loss is long under US transfer");
        assert_eq!(r.short_term, 1000, "the first sale was short either way");

        let keep = book_with_wash_keep("wash-keep-hp", 30, "fifo");
        buy_on(&keep, "orig", 1, 2000, "2026-01-01");
        dispose(&keep, "s", 1, 1000, "2026-10-28");
        buy_on(&keep, "repl", 1, 1000, "2026-11-01");
        let left = Projection::of_book(&keep).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left[0].cost, 2000, "the write is still the write");
        assert_eq!(
            left[0].acquired,
            Some(day("2026-11-01")),
            "keep leaves the repurchase's own date"
        );
        dispose(&keep, "s2", 1, 1500, "2027-02-05");
        let p = Projection::of_book(&keep).unwrap();
        assert!(p.lot_breaks(B).unwrap().is_empty(), "{:?}", p.lot_breaks(B).unwrap());
        let r = p.realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.short_term, 1500, "both losses are short when the period is kept");
        assert_eq!(r.long_term, 0, "assuming US transfer here would have put 500 long");
        assert_eq!(r.gain, 1500, "the total is the same under either rule");
    }

    #[test]
    fn a_forward_repurchase_washes_once_the_replacement_opens() {
        // Sale first, then the buy — the half a forward-only engine gets right.
        let d = book_with_wash("wash-fwd", 30, "fifo");
        buy_on(&d, "orig", 100, 2000, "2026-01-01");
        dispose(&d, "s", 100, 1000, "2026-06-15");
        buy_on(&d, "repl", 40, 500, "2026-06-20");

        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 900, "the write landed after the repurchase opened");
        assert_eq!(left[0].acquired, Some(day("2026-01-01")));
    }

    #[test]
    fn a_strike_day_reads_whether_a_wash_window_is_still_open() {
        // ⭐ `Ratio.Lots.WashRestatement`. Sale first, no replacement yet:
        // the leftover is pending. A strike on 20 June sees the window
        // open; one on 16 July does not. After the repurchase attaches,
        // nothing is pending — the figure already moved.
        let d = book_with_wash("wash-qualify", 30, "fifo");
        buy_on(&d, "orig", 100, 2000, "2026-01-01");
        dispose(&d, "s", 100, 1000, "2026-06-15");

        let p = Projection::of_book(&d).unwrap();
        let open = p.open_wash_windows(B, day("2026-06-20")).unwrap();
        assert_eq!(open.value.len(), 1, "the window is still open");
        assert_eq!(open.value[0].sold_on, day("2026-06-15"));
        assert_eq!(open.value[0].remaining_loss, -1000);
        assert!(
            p.open_wash_windows(B, day("2026-07-16"))
                .unwrap()
                .value
                .is_empty(),
            "day 31 is outside a 30-day window"
        );

        let s = relief::strike_gain(
            relief::StrikeId {
                prefix: open.prefix as u64,
            },
            30,
            open.value[0].sold_on,
            day("2026-06-20"),
            open.value[0].remaining_loss,
        )
        .unwrap();
        assert!(s.qualified);
        // The later repurchase moves the leftover. Restate cites the
        // prefix; the struck figure is untouched.
        let r = relief::restate(&s, 30, day("2026-06-20"), -600).expect("moved");
        assert_eq!(r.cites.prefix, open.prefix as u64);
        assert_eq!(s.figure, -1000);
        assert_eq!(r.original, -1000);
        assert_eq!(r.moved_to, -600);

        buy_on(&d, "repl", 100, 1000, "2026-06-20");
        let p = Projection::of_book(&d).unwrap();
        assert!(
            p.open_wash_windows(B, day("2026-06-20"))
                .unwrap()
                .value
                .is_empty(),
            "the write landed; nothing is still pending"
        );
    }

    #[test]
    fn a_wash_write_changes_what_a_later_hifo_sale_gives_up() {
        // ⛔ METHODS AND WASH ARE ONE PROBLEM. After the write, HIFO gives up
        // the replacement it previously ignored.
        let d = book_with_wash("wash-hifo", 30, "hifo");
        buy_on(&d, "dear", 1, 25, "2026-06-01");
        buy_on(&d, "cheap", 1, 10, "2026-06-10");
        // Sell a third lot at a loss so the cheap one is the replacement.
        // Cheaper: open a losing lot, sell it, the cheap lot is in-window.
        // Simpler path: attach via a losing sale of `dear`? HIFO would take
        // `dear` first (25 > 10). Sell 1 at 5: HIFO gives up 25, loss 20.
        // The cheap lot is open, in window, and is the replacement.
        dispose(&d, "s", 1, 5, "2026-06-15");
        let p = Projection::of_book(&d).unwrap();
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 30, "10 + 20 of deferred loss");
        // HIFO now gives that up, not a 10-cost lot.
        dispose(&d, "s2", 1, 30, "2026-06-20");
        let p = Projection::of_book(&d).unwrap();
        assert_eq!(p.relieved_cost(B).unwrap(), 25 + 30);
    }

    // ── min-tax, on a real book ────────────────────────────────────────────

    fn book_with_mintax(name: &str) -> std::path::PathBuf {
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
                b"rules = []\nlong_term_days = 365\nmin_tax_short_weight = 2\n\n\
                  [chart_roles]\ninvestments = 1\ncash = 2\nrealized_gain = 30\n",
            )
            .unwrap();
        b.set_active(&c).unwrap();
        d
    }

    fn mintax_holding(d: &std::path::Path) {
        // A short (basis 10), B long (basis 12). Close bases — the flip.
        buy_on(d, "a", 1, 10, "2025-10-01");
        buy_on(d, "b", 1, 12, "2023-01-01");
    }

    #[test]
    fn mintax_on_the_book_takes_the_long_lot_at_a_gain() {
        // ⭐ `Ratio.Lots.MinTax.mintax_takes_different_lots_at_the_two_prices`.
        let d = book_with_mintax("mintax-gain");
        mintax_holding(&d);
        dispose(&d, "s", 1, 50, "2026-01-01");
        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 10, "gave up B at 12; A remains");
        assert_eq!(left[0].acquired, Some(day("2025-10-01")));
    }

    #[test]
    fn mintax_on_the_book_takes_the_short_lot_at_a_loss() {
        let d = book_with_mintax("mintax-loss");
        mintax_holding(&d);
        dispose(&d, "s", 1, 5, "2026-01-01");
        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 12, "gave up A at 10; B remains");
        assert_eq!(left[0].acquired, Some(day("2023-01-01")));
    }

    #[test]
    fn a_silent_book_does_not_rank_at_a_price() {
        // ⛔ UNSET STAYS UNSET. A book that never elected min-tax still
        // relieves oldest-first by custom. The same two lots, sold at 5:
        // FIFO takes A (10) because it is older in journal order? A was
        // bought first. FIFO takes A at any price. That is not the flip.
        let d = book_with_gains("mintax-silent", 365);
        mintax_holding(&d);
        dispose(&d, "s", 1, 5, "2026-01-01");
        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].cost, 12, "FIFO took A; B remains — no price ranking");
    }

    // ── specific identification, on a real book ────────────────────────────

    fn spec_holding(d: &std::path::Path) {
        // Three one-unit lots: 10, 40, 70. The middle one is the case no
        // Order can pick. `Ratio.Lots.SpecId.specHolding`.
        buy_on(d, "a", 1, 10, "2026-01-01");
        buy_on(d, "b", 1, 40, "2026-01-02");
        buy_on(d, "c", 1, 70, "2026-01-03");
    }

    #[test]
    fn specid_on_the_book_takes_the_named_lot() {
        // ⭐ `Ratio.Lots.SpecId.specid_takes_from_the_middle`.
        let d = book_with_gains("specid-middle", 365);
        spec_holding(&d);
        let seqs: Vec<u64> = Projection::of_book(&d)
            .unwrap()
            .lots_of(B, 1, "vti")
            .unwrap()
            .value
            .iter()
            .map(|l| l.seq)
            .collect();
        assert_eq!(seqs.len(), 3);
        dispose_identified(&d, "s", 1, 50, "2026-06-01", &[seqs[1]]);
        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 2);
        let costs: Vec<i64> = left.iter().map(|l| l.cost).collect();
        assert_eq!(costs, vec![10, 70], "named the middle; ends remain");
    }

    #[test]
    fn specid_unnamed_on_the_book_is_a_break_not_fifo() {
        // ⛔ UNSET ≠ EMPTY. Some([]) is SpecID elected and lots unnamed.
        // FIFO would take the first lot (10). Refuse instead.
        let d = book_with_gains("specid-unnamed", 365);
        spec_holding(&d);
        let mut b = FileBook::open(&d).unwrap();
        let c = b.active().unwrap().unwrap();
        let postings = relief::sale_postings(ROLES, None, "vti", 1, 10, 50).unwrap();
        b.append(&JournalEntry {
            id: "s".into(),
            memo: "sell".into(),
            config: c,
            postings,
            trade_date: Some("2026-06-01".into()),
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: Some(vec![]),
            special_allocations: None,
        })
        .unwrap();
        drop(b);
        let p = Projection::of_book(&d).unwrap();
        let breaks = p.lot_breaks(B).unwrap();
        assert!(!breaks.is_empty(), "unnamed SpecID must break, not walk FIFO");
        let msg = breaks.join("\n");
        assert!(
            msg.contains("no lots were named") || msg.contains("unnamed"),
            "{msg}"
        );
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 3, "the holding is untouched");
    }

    // ── average cost, on a real book ───────────────────────────────────────

    fn book_with_average_cost(name: &str) -> std::path::PathBuf {
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
                b"rules = []\naverage_cost = true\n\n\
                  [chart_roles]\ninvestments = 1\ncash = 2\nrealized_gain = 30\n",
            )
            .unwrap();
        b.set_active(&c).unwrap();
        d
    }

    fn pool_holding_on(d: &std::path::Path) {
        // 10 / 20 / 60. The pool is 30 — no lot carries 30.
        buy_on(d, "a", 1, 10, "2026-01-01");
        buy_on(d, "b", 1, 20, "2026-01-02");
        buy_on(d, "c", 1, 60, "2026-01-03");
    }

    #[test]
    fn average_cost_on_the_book_leaves_a_pool() {
        // ⭐ `Ratio.Lots.AverageCost.the_remainder_is_a_pool_not_the_other_lots`.
        let d = book_with_average_cost("avg-pool");
        pool_holding_on(&d);
        dispose(&d, "s", 1, 50, "2026-06-01");
        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 1, "the remainder is a pool, not the other lots");
        assert_eq!(left[0].units, 2);
        assert_eq!(left[0].cost, 60);
        assert_eq!(left[0].seq, 0);
    }

    #[test]
    fn a_silent_book_does_not_pool() {
        // ⛔ UNSET STAYS UNSET. A book that never elected average cost
        // still relieves oldest-first by custom. FIFO takes 10 and leaves
        // 20 and 60. That is not a pool.
        let d = book_with_gains("avg-silent", 365);
        pool_holding_on(&d);
        dispose(&d, "s", 1, 50, "2026-06-01");
        let left = Projection::of_book(&d).unwrap().lots_of(B, 1, "vti").unwrap().value;
        let costs: Vec<i64> = left.iter().map(|l| l.cost).collect();
        assert_eq!(costs, vec![20, 60], "FIFO took 10; the other lots remain");
    }

    #[test]
    fn a_pool_that_does_not_divide_is_a_break_not_a_round() {
        let d = book_with_average_cost("avg-round");
        buy_on(&d, "a", 1, 12, "2026-01-01");
        buy_on(&d, "b", 1, 13, "2026-01-02");
        let mut b = FileBook::open(&d).unwrap();
        let c = b.active().unwrap().unwrap();
        // Posted basis is a guess the fold will refuse — the engine must
        // not invent 12 or 13.
        let postings = relief::sale_postings(ROLES, None, "vti", 1, 12, 50).unwrap();
        b.append(&JournalEntry {
            id: "s".into(),
            memo: "sell".into(),
            config: c,
            postings,
            trade_date: Some("2026-06-01".into()),
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        })
        .unwrap();
        drop(b);
        let p = Projection::of_book(&d).unwrap();
        let breaks = p.lot_breaks(B).unwrap();
        assert!(!breaks.is_empty(), "a non-dividing pool must break, not round");
        let msg = breaks.join("\n");
        assert!(msg.contains("does not divide"), "{msg}");
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left.len(), 2, "the holding is untouched");
    }

    #[test]
    fn a_pool_with_a_shared_date_classifies() {
        // ⭐ `Ratio.Lots.PoolPeriod.a_shared_long_date_classifies_long`.
        // Two lots acquired the same day. The pool carries that day.
        // Sold more than a year later: long, not unclassified.
        let d = book_with_average_cost("pool-shared-date");
        buy_on(&d, "a", 1, 10, "2024-01-01");
        buy_on(&d, "b", 1, 10, "2024-01-01");
        dispose(&d, "s", 1, 50, "2026-06-30");
        let p = Projection::of_book(&d).unwrap();
        let r = p.realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        // Credit-normal: proceeds 50, pooled basis 10, gain −40.
        assert_eq!(r.gain, -40);
        assert_eq!(r.long_term, -40, "the shared date is long");
        assert_eq!(r.short_term, 0);
        assert_eq!(r.unclassified(), 0);
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left[0].acquired, Some(day("2024-01-01")));
    }

    #[test]
    fn a_pool_with_mixed_dates_stays_unclassified() {
        // ⭐ `Ratio.Lots.PoolPeriod.treating_mixed_dates_as_an_order_invents_a_category`.
        // Day-2024 and day-2026. FIFO would take the old lot and invent
        // long-term. The pool carries neither date. The books still tie.
        let d = book_with_average_cost("pool-mixed-dates");
        buy_on(&d, "a", 1, 10, "2024-01-01");
        buy_on(&d, "b", 1, 10, "2026-06-01");
        dispose(&d, "s", 1, 50, "2026-06-30");
        let p = Projection::of_book(&d).unwrap();
        let r = p.realized(B, Some(ROLES), &Rates::none()).unwrap().value.unwrap();
        assert_eq!(r.gain, -40, "the total is exact and ties to the chart");
        assert_eq!(r.short_term, 0);
        assert_eq!(r.long_term, 0);
        assert_eq!(r.unclassified(), -40, "no category was invented");
        let left = p.lots_of(B, 1, "vti").unwrap().value;
        assert_eq!(left[0].acquired, None, "the remainder stays unset too");
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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

        let err = p.nav(B, &|dim| dim == 1, &Rates::of("USD", [])).unwrap_err();
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
        assert_eq!(p.nav(B, &|dim| dim == 1, &rates).unwrap().value.0, 20_800);

        // ⛔ AND THE RATE CHANGES THE ANSWER, which is the whole reason it must
        // be stated. At par the same book is 19,000 — the number the old flat
        // sum produced, and it is only right if a euro is worth a dollar.
        let par = Rates::of("USD", [("EUR".to_string(), RATE_SCALE)]);
        assert_eq!(p.nav(B, &|dim| dim == 1, &par).unwrap().value.0, 19_000);
    }

    #[test]
    fn the_base_currency_needs_no_rate_and_an_untyped_book_still_values() {
        // ⛔ A FUND DOES NOT RECORD WHAT A DOLLAR IS WORTH IN DOLLARS, so there
        // is no rate fact for the base and looking it up would refuse every
        // book that holds any of its own currency.
        let d = two_currency_book("base-at-par");
        let p = Projection::of_book(&d).unwrap();
        let rates = Rates::of("USD", [("EUR".to_string(), 100)]);
        assert!(p.nav(B, &|dim| dim == 20, &rates).is_ok());

        // And a book written before any of this, whose legs name no currency,
        // values with no rates at all — one group, translated at par.
        let d = book("untyped-values", &[("vti", 10, 1)]);
        let p = Projection::of_book(&d).unwrap();
        assert!(p.nav(B, &|dim| dim == 1, &Rates::none()).is_ok());
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
        let pos_key = p.positions(B).unwrap().value.held.keys().find(|(_, i)| &**i == "vti").unwrap();
        let lot_key = p.fold_of(B).unwrap().lots.open.keys().find(|(_, i)| &**i == "vti").unwrap();
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
        })
        .unwrap();
        drop(b);

        let p = Projection::of_book(&d).unwrap();
        let breaks = p.lot_breaks(B).unwrap();
        assert_eq!(breaks.len(), 1, "{breaks:?}");
        assert!(breaks[0].contains("is not a date"), "{}", breaks[0]);
        // And the lot is open with no acquisition date, which the
        // holding-period methods refuse rather than guess about.
        assert_eq!(p.lots_of(B, 1, "vti").unwrap().value[0].acquired, None);
    }

    #[test]
    fn realized_carries_the_prefix_and_says_nothing_without_a_chart() {
        let d = book_with_gains("no-roles", 365);
        buy_on(&d, "b", 1, 100, "2025-01-10");
        dispose(&d, "s", 1, 300, "2026-06-30");
        let p = Projection::of_book(&d).unwrap();

        assert_eq!(p.realized(B, Some(ROLES), &Rates::none()).unwrap().prefix, p.prefix());
        // ⛔ `None`, NOT ZERO. Without a chart the engine does not know which
        // dimension a gain lands in, and zero is a fund that realized nothing.
        assert!(p.realized(B, None, &Rates::none()).unwrap().value.is_none());
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
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
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
        assert_eq!(quiet.positions(B).unwrap().value, watched.positions(B).unwrap().value);
        assert_eq!(
            quiet.nav(B, &|d| d == 1 || d == 2, &Rates::none()).unwrap().value,
            watched.nav(B, &|d| d == 1 || d == 2, &Rates::none()).unwrap().value
        );
    }

    // ── The per-view fold: the cut, the band, and what accounts for a difference ──

    /// Two books of record over one journal: `abor` on the trade date, `ibor`
    /// settling T+2 over a Saturday/Sunday calendar.
    const DUAL: &str = r#"rules = []
[[calendar]]
id = "wk"
weekend = [0, 6]
[[view]]
id = "abor"
display_name = "ABOR"
basis = "trade"
[[view]]
id = "ibor"
display_name = "IBOR"
basis = "settlement"
settles_in = 2
calendar = "wk"
"#;

    fn dual_book(name: &str) -> std::path::PathBuf {
        let d = tmp_root().join(format!("ratio-project-{name}"));
        let _ = std::fs::remove_dir_all(&d);
        let mut b = FileBook::open(&d).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
            Account { dim: 3, display_name: "Capital".into(), account_type: A::Equity },
        ])
        .unwrap();
        let c = b.put(DUAL.as_bytes()).unwrap();
        b.set_active(&c).unwrap();
        d
    }

    /// A subscription: cash in against capital. ⛔ THE SHAPE THAT MAKES TWO
    /// VIEWS ACTUALLY DISAGREE — a purchase moves cash into investments, both
    /// assets, so recognising it or not moves a NAV by ZERO. HANDOFF.md records
    /// the differential going vacuous twice on tails full of trades.
    fn subscribe_dated(d: &std::path::Path, id: &str, amount: i64, day: &str) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "subscription".into(),
            config: c,
            postings: vec![PostingRecord::new(2, amount), PostingRecord::new(3, -amount)],
            trade_date: Some(day.into()),
            announcement: None,
                due_date: None,
                application: None,
                identified_lots: None,
                special_allocations: None,
        })
        .unwrap();
    }

    /// A subscription in a named currency — the shape that makes translation
    /// residue visible. Untyped legs translate at par and the residue vanishes.
    fn subscribe_dated_currency(
        d: &std::path::Path,
        id: &str,
        amount: i64,
        day: &str,
        currency: &str,
    ) {
        let mut b = FileBook::open(d).unwrap();
        let c = b.active().unwrap().unwrap();
        b.append(&JournalEntry {
            id: id.into(),
            memo: "subscription".into(),
            config: c,
            postings: vec![
                PostingRecord::of_currency(2, amount, currency),
                PostingRecord::of_currency(3, -amount, currency),
            ],
            trade_date: Some(day.into()),
            announcement: None,
            due_date: None,
            application: None,
            identified_lots: None,
            special_allocations: None,
        })
        .unwrap();
    }

    const ASSETS: fn(i64) -> bool = |d| d == 1 || d == 2;

    #[test]
    fn one_pass_advances_every_view_and_every_view_reads_the_same_prefix() {
        // ⭐ `//tla:views_check`'s `EveryViewFoldsTheSamePrefix`, on the real
        // fold: one `of_book`, two books of record, one journal position.
        let d = dual_book("one-pass");
        buy_dated(&d, "t1", "vti", 10, 1_000, "2026-03-02"); // Monday
        subscribe_dated(&d, "s1", 5_000, "2026-03-03"); // Tuesday
        buy_dated(&d, "t2", "voo", 5, 500, "2026-03-04"); // Wednesday
        let p = Projection::of_book(&d).unwrap();

        let a = p.positions("abor").unwrap();
        let i = p.positions("ibor").unwrap();
        assert_eq!(a.prefix, 3);
        assert_eq!(a.prefix, i.prefix, "one pass feeds every view");
        assert_eq!(a.view, "abor");
        assert_eq!(i.view, "ibor");
        // The recorded default is NOT among them: this book declares views.
        assert!(p.positions(B).is_err(), "a declared book keeps no journal-order view");
    }

    #[test]
    fn two_views_disagree_about_the_nav_and_the_difference_is_a_list_of_entries() {
        // The three-day shape: by Wednesday the Monday trade has settled, the
        // Tuesday subscription (settles Thursday) and the Wednesday trade
        // (settles Friday) are in flight under `ibor`, and `abor` holds all
        // three.
        let d = dual_book("difference");
        buy_dated(&d, "t1", "vti", 10, 1_000, "2026-03-02"); // Mon; settles Wed
        subscribe_dated(&d, "s1", 5_000, "2026-03-03"); // Tue; settles Thu
        buy_dated(&d, "t2", "voo", 5, 500, "2026-03-04"); // Wed; settles Fri
        let p = Projection::of_book(&d).unwrap();

        let nav_a = p.nav("abor", &ASSETS, &Rates::none()).unwrap();
        let nav_i = p.nav("ibor", &ASSETS, &Rates::none()).unwrap();
        // ⛔ BOTH HALVES, because either alone is a feature that does nothing:
        // the books tie in every view (a view keeps or drops WHOLE entries)…
        assert_eq!(nav_a.value.1, 0, "abor ties");
        assert_eq!(nav_i.value.1, 0, "ibor ties");
        // …and the NAVs DIFFER, by exactly the subscription in flight.
        assert_eq!(nav_a.value.0 - nav_i.value.0, 5_000, "the settlement gap is the figure");
        // The cut travels with the figure: ibor has recognised through the
        // Wednesday frontier, and abor consults the same clock.
        assert_eq!(nav_i.through, Some(day("2026-03-04")));

        // The difference is a LIST — `two_views_differ_by_exactly_what_is_in_
        // flight` — and it sums to the figure exactly.
        let rec = p.reconcile("abor", "ibor", &ASSETS, &Rates::none()).unwrap();
        assert_eq!(rec.value.difference, 5_000);
        assert_eq!(rec.value.entries.len(), 2, "{:?}", rec.value.entries);
        let sum: i64 = rec.value.entries.iter().map(|e| e.effect).sum();
        assert_eq!(sum, rec.value.difference, "the entries account for the difference");
        // The subscription carries the effect; the trade is in flight too but
        // moves the NAV by zero — shown, not dropped, because an entry that
        // contributes nothing is still an entry the views disagree about.
        let s1 = rec.value.entries.iter().find(|e| e.id == "s1").unwrap();
        assert_eq!(s1.effect, 5_000);
        assert_eq!(s1.recognised_here, Some(day("2026-03-03")), "abor: the trade day");
        assert_eq!(s1.recognised_there, Some(day("2026-03-05")), "ibor: T+2");
        let t2 = rec.value.entries.iter().find(|e| e.id == "t2").unwrap();
        assert_eq!(t2.effect, 0, "a purchase moves cash into investments — both assets");

        // And folded to the head with the gap closed, the views agree again:
        // everything eventually settles.
        buy_dated(&d, "t3", "vti", 1, 100, "2026-03-16"); // the next Monday
        let p = Projection::of_book(&d).unwrap();
        assert_eq!(
            p.nav("abor", &ASSETS, &Rates::none()).unwrap().value,
            p.nav("ibor", &ASSETS, &Rates::none()).unwrap().value,
            "with nothing in flight the views agree"
        );
    }

    #[test]
    fn a_view_a_configuration_does_not_declare_refuses_rather_than_falling_back() {
        // Half one: a view the BOOK does not keep refuses at the read, naming
        // what it keeps.
        let d = dual_book("undeclared-view");
        buy_dated(&d, "t1", "vti", 10, 1_000, "2026-03-02");
        let p = Projection::of_book(&d).unwrap();
        let e = p.nav("emir", &ASSETS, &Rates::none()).unwrap_err().to_string();
        assert!(e.contains("no view \"emir\""), "{e}");
        assert!(e.contains("abor") && e.contains("ibor"), "the refusal names what it keeps: {e}");

        // Half two: an ENTRY whose pinned configuration declares no views is
        // refused by a declared view — never folded as `recorded`, for the
        // reason a bad config refuses the relief rather than falling back to
        // FIFO. The entry predates the view declaration; its terms never
        // mentioned settlement.
        let d2 = tmp_root().join("ratio-project-mixed-history");
        let _ = std::fs::remove_dir_all(&d2);
        let mut b = FileBook::open(&d2).unwrap();
        b.put_accounts(&[
            Account { dim: 1, display_name: "Investments".into(), account_type: A::Asset },
            Account { dim: 2, display_name: "Cash".into(), account_type: A::Asset },
        ])
        .unwrap();
        let plain = b.put(b"rules = []\n").unwrap();
        b.set_active(&plain).unwrap();
        buy_dated(&d2, "old", "vti", 10, 1_000, "2026-03-02");
        let mut b = FileBook::open(&d2).unwrap();
        let dual = b.put(DUAL.as_bytes()).unwrap();
        b.set_active(&dual).unwrap();
        buy_dated(&d2, "new", "vti", 5, 500, "2026-03-03");
        let p = Projection::of_book(&d2).unwrap();

        let refused = p.unplaceable("ibor").unwrap();
        assert_eq!(refused.len(), 1, "{refused:?}");
        assert_eq!(refused[0].id, "old");
        assert!(refused[0].why.contains("declares no view"), "{}", refused[0].why);

        // An entry NEITHER view can place is the reconciliation's third list —
        // it contributes to neither figure, so the difference is still fully
        // accounted for, and omitting it would make the account look complete
        // while an entry sits outside both books of record.
        let rec = p.reconcile("abor", "ibor", &ASSETS, &Rates::none()).unwrap();
        assert_eq!(rec.value.unplaceable.len(), 1);
        assert_eq!(rec.value.unplaceable[0].id, "old");

        // But an entry only ONE view can place REFUSES the reconciliation:
        // the other view counts it, so no list of in-flight entries can
        // account for the difference between them.
        let mut b = FileBook::open(&d2).unwrap();
        let abor_only = b
            .put(b"rules = []\n[[view]]\nid = \"abor\"\ndisplay_name = \"ABOR\"\nbasis = \"trade\"\n")
            .unwrap();
        b.set_active(&abor_only).unwrap();
        buy_dated(&d2, "half", "vti", 1, 100, "2026-03-04");
        let mut b = FileBook::open(&d2).unwrap();
        let dual = b.put(DUAL.as_bytes()).unwrap();
        b.set_active(&dual).unwrap();
        let p = Projection::of_book(&d2).unwrap();
        let e = p.reconcile("abor", "ibor", &ASSETS, &Rates::none()).unwrap_err().to_string();
        assert!(e.contains("\"half\"") && e.contains("cannot place"), "{e}");
    }

    #[test]
    fn the_pending_queue_is_bounded_by_the_settlement_lag_not_by_the_journal() {
        // ⛔ THE TRAP INSIDE THE SURVIVING DESIGN, as an assertion. The lag
        // bounds the band only because the cut moves as the fold reads — a cut
        // that stayed at zero would put this whole journal in the band, which
        // is the retained-entries design rejected in PLAN.md, reached by a
        // different road.
        let d = dual_book("bounded-band");
        // Sixty weekdays of dated trades: twelve weeks, no weekend entries.
        let mut dy = day("2026-03-02"); // a Monday
        let mut n = 0;
        while n < 60 {
            let dow = (i64::from(dy) + 4).rem_euclid(7);
            if dow != 0 && dow != 6 {
                buy_dated(
                    &d,
                    &format!("t{n}"),
                    "vti",
                    1,
                    100,
                    &ratio_common::iso_date_from_days(i64::from(dy)),
                );
                n += 1;
            }
            dy += 1;
        }
        let p = Projection::of_book(&d).unwrap();
        assert_eq!(p.prefix(), 60);
        let banded = p.in_flight("ibor").unwrap();
        assert!(banded > 0, "a T+2 view with nothing in flight at the head folded everything");
        assert!(
            banded <= 4,
            "sixty days of history left {banded} entries in the band — the lag is the \
             bound, and the lag is two open days"
        );
        assert_eq!(p.in_flight("abor").unwrap(), 0, "trade basis holds nothing back");
    }

    #[test]
    fn the_recorded_view_folds_exactly_what_the_projection_used_to() {
        // ⛔ THE MIGRATION, AS A DIFFERENTIAL. A book declaring no views keeps
        // exactly one, and its figures are checked against `FileBook`'s
        // independent fold — the system of record — not against another
        // projection.
        let d = book("recorded-unchanged", &[("vti", 25_000, 100), ("voo", 10_000, 40)]);
        sell(&d, "s1", "vti", 20, 5_000);
        let p = Projection::of_book(&d).unwrap();

        let (held, rest) = FileBook::open(&d).unwrap().positions().unwrap();
        let projected: BTreeMap<(i64, String), (i64, i64)> = p
            .positions(B)
            .unwrap()
            .value
            .held
            .iter()
            .map(|((dim, i), v)| ((*dim, i.to_string()), *v))
            .collect();
        assert_eq!(projected, held);
        assert_eq!(p.positions(B).unwrap().value.rest, rest);
        // Journal order consults no date, so the figure carries no cut.
        assert_eq!(p.positions(B).unwrap().through, None);
        assert_eq!(p.in_flight(B).unwrap(), 0);
        assert!(p.unplaceable(B).unwrap().is_empty());
    }

    #[test]
    fn a_translation_residue_refuses_rather_than_publishing_a_silent_zero() {
        // ⭐ INTEGER TRANSLATION DOES NOT DISTRIBUTE OVER A SUM.
        // Two 1-EUR subscriptions at 1.50: each entry translates to 1,
        // the pair translates to 3. Publishing the difference as 0 — or
        // adjusting a row so the list adds up — would look like agreement.
        let d = dual_book("fx-residue");
        subscribe_dated_currency(&d, "s1", 1, "2026-03-03", "EUR"); // Tue; settles Thu
        subscribe_dated_currency(&d, "s2", 1, "2026-03-04", "EUR"); // Wed; settles Fri
        let p = Projection::of_book(&d).unwrap();
        let rates = Rates::of("USD", [("EUR".to_string(), 150)]);

        let nav_a = p.nav("abor", &ASSETS, &rates).unwrap().value.0;
        let nav_i = p.nav("ibor", &ASSETS, &rates).unwrap().value.0;
        assert_eq!(nav_a, 3, "2 EUR at 1.50 rounds once: 3");
        assert_eq!(nav_i, 0, "neither subscription has settled");

        let e = p.reconcile("abor", "ibor", &ASSETS, &rates).unwrap_err().to_string();
        assert!(e.contains("translation residue"), "{e}");
        assert!(e.contains("differ by 3"), "{e}");
        assert!(e.contains("sum to 2"), "{e}");
        assert!(!e.contains("difference of 0"), "a silent zero would look like agreement: {e}");
    }

    #[test]
    fn a_missing_rate_refuses_reconcile_rather_than_agreeing_at_zero() {
        // ⛔ SKIPPING THE FOREIGN LEG WOULD MAKE BOTH NAVS 0 AND THE
        // DIFFERENCE 0 — silent agreement on a book that holds euros.
        let d = dual_book("fx-no-rate");
        subscribe_dated_currency(&d, "s1", 1, "2026-03-03", "EUR");
        subscribe_dated_currency(&d, "s2", 1, "2026-03-04", "EUR");
        let p = Projection::of_book(&d).unwrap();

        let e = p.reconcile("abor", "ibor", &ASSETS, &Rates::of("USD", [])).unwrap_err().to_string();
        assert!(e.contains("EUR"), "{e}");
        assert!(
            e.contains("mixing denominations") || e.contains("no rate"),
            "the refusal must name the missing rate, not invent 0: {e}"
        );
    }

    #[test]
    fn a_view_declared_after_the_fold_refuses_reconcile_rather_than_a_zero_gap() {
        // ⭐ THE THIRD PLAN REFUSE. A maintained fold has already read past
        // the history the new view would need. Serving a 0.00 difference
        // would look like the new book of record agrees with the old one
        // on a fragment.
        let d = dual_book("declared-after");
        subscribe_dated(&d, "s1", 5_000, "2026-03-03");
        let mut p = Projection::of_book(&d).unwrap();
        let mut b = FileBook::open(&d).unwrap();
        let extra = b
            .put(
                br#"rules = []
[[calendar]]
id = "wk"
weekend = [0, 6]
[[view]]
id = "abor"
display_name = "ABOR"
basis = "trade"
[[view]]
id = "ibor"
display_name = "IBOR"
basis = "settlement"
settles_in = 2
calendar = "wk"
[[view]]
id = "emir"
display_name = "EMIR"
basis = "trade"
"#,
            )
            .unwrap();
        b.set_active(&extra).unwrap();
        drop(b);
        p.follow(&d).unwrap();

        let e = p.reconcile("emir", "abor", &ASSETS, &Rates::none()).unwrap_err().to_string();
        assert!(e.contains("declared after"), "{e}");
        assert!(e.contains("cannot place") || e.contains("already read past"), "{e}");
    }
}
