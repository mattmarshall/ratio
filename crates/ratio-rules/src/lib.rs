//! ratio-rules — posting rules: parsed, checked, and compiled to postings.
//!
//! PLAN.md Stage 1. Three things, in order of how much they matter:
//!
//! **1. A rule is data, not a language.** Rules are TOML against the schema
//! below. The `rule … { }` syntax on the website is a *rendering*, produced by
//! [`render`] — nobody writes it, and there is no parser for it. Building one
//! would be weeks of work no customer can see, and it is the single most
//! likely way this stalls.
//!
//! **2. A rule carries weights, not amounts.** `Ratio.Chart.applyTemplate`
//! scales a template of weights by an amount, and
//! `Ratio.Chart.balanced_template_balances` proves that if the weights net to
//! zero then *every* instantiation nets to zero — at any amount, any number of
//! times. So [`check`] verifies the template **once**, at authoring time, and
//! [`compile`] can then emit postings that are balanced by theorem rather than
//! by inspection. That is what makes the control plane safe to be expressive.
//!
//! **3. The checks speak to a fund accountant.** A rejected rule says which
//! account, what the shortfall is, and what to do — not a stack trace.
//!
//! Amounts are exact integers throughout. Rates are basis points (`i64`), day
//! counts are an enum: a float cannot be *expressed*, so "no floating point"
//! is a property of the schema rather than a check that might be forgotten.

mod render;
pub use render::render;

/// The grading decision, authored in Lean.
mod generated_tolerance;

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Context, Result};
use ratio_kernel::{transaction_is_balanced, Posting, Transaction};
use ratio_store::{Account, PostingRecord};
use serde::{Deserialize, Serialize};

/// What a rule is triggered by, and therefore how its amount is derived.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// A trade: the amount is given by the event.
    Trade,
    /// A dividend: the amount is given by the event.
    Dividend,
    /// An accrual: the amount is *computed* from a basis, a rate and a period.
    Accrual,
    /// A mark to market: the amount is the DIFFERENCE between what the book
    /// holds a position at and what it is worth.
    ///
    /// A posting, never an assignment. `Ratio.Valuation.mark_conserves` proves
    /// the entry balances and `mark_lands_on_market` proves it lands on market
    /// value; `//tla:mark_from_cost_check` shows what happens when the delta is
    /// taken from cost instead — the book drifts by the whole gain on every
    /// mark, and every entry is still balanced.
    Mark,
}

/// The day-count convention an accrual is computed on.
///
/// An enum rather than a number so a rule cannot silently carry a made-up
/// convention, and so the denominator is never a float.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DayCount {
    /// Actual days over 365.
    #[serde(rename = "act/365")]
    Act365,
    /// Actual days over 360.
    #[serde(rename = "act/360")]
    Act360,
    /// 30-day months over 360.
    #[serde(rename = "30/360")]
    Thirty360,
}

impl DayCount {
    /// The denominator, in days.
    pub fn denominator(self) -> i128 {
        match self {
            DayCount::Act365 => 365,
            DayCount::Act360 | DayCount::Thirty360 => 360,
        }
    }

    /// How it is written in a rule and on a report.
    pub fn as_str(self) -> &'static str {
        match self {
            DayCount::Act365 => "act/365",
            DayCount::Act360 => "act/360",
            DayCount::Thirty360 => "30/360",
        }
    }
}

/// One leg of a posting template: an account, and its share of the amount.
///
/// `weight` is a multiplier, not an amount. The weights of a rule must net to
/// zero; see [`check`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Leg {
    /// The conserved dimension — the account this leg posts to.
    pub account: i64,
    /// This leg's multiple of the amount. Positive debits, negative credits.
    pub weight: i64,

    /// Whether this leg is held PER INSTRUMENT.
    ///
    /// An accounting decision, so it belongs in the approved rule rather than
    /// in the mapping: investments at fair value are held per instrument, and
    /// the cash that paid for them is not. Marking every leg would put a
    /// security on the cash account, which is not a thing.
    ///
    /// Instruments partition value further; they do not change how much there
    /// is, so conservation is untouched — proved in
    /// `Ratio.Ingest.partition_preserves_conservation`.
    #[serde(default)]
    pub per_instrument: bool,
}

/// A posting rule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rule {
    /// Stable identifier, used in postings and diffs.
    pub id: String,
    /// What triggers it.
    pub kind: RuleKind,
    /// What it is, in the words of whoever approved it.
    #[serde(default)]
    pub description: String,
    /// The posting template. Must net to zero.
    #[serde(rename = "posting", default)]
    pub legs: Vec<Leg>,
    /// Accrual rate in basis points per year. Required for `accrual`, and
    /// meaningless otherwise. Integer — a rate cannot be a float here.
    #[serde(default)]
    pub rate_bp: Option<i64>,
    /// Day-count convention. Required for `accrual`.
    #[serde(default)]
    pub day_count: Option<DayCount>,
}

/// A configuration: the whole rule set, as promoted to the control plane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSet {
    #[serde(rename = "rule", default)]
    pub rules: Vec<Rule>,

    /// Which lots a sale gives up.
    ///
    /// ⛔ A CONFIGURATION TERM, NOT A CODE CHOICE, and it belongs here for the
    /// same reason every rule does: it is a term of an administration agreement,
    /// it decides a figure somebody is taxed on, and changing it is an approval
    /// rather than a deployment. `Ratio.Lots.Methods.the_method_decides_the_
    /// taxable_gain` — the same holding and the same trade produce four
    /// different taxable incomes across the four methods, with nothing on the
    /// balance sheet moving.
    ///
    /// ⚠ AND CHANGING IT IS NOT RETROACTIVE. A strike pins the configuration in
    /// force at the last entry it folded, so past reliefs stay computed under
    /// the method that was agreed then. Re-running history under a new method
    /// would restate every investor's tax position, which is exactly what
    /// `Ratio.Period.one_answer_per_day` refuses.
    ///
    /// ⛔ `None` MEANS NOBODY SAID, AND THAT IS NOT THE SAME AS SAYING FIFO.
    /// A fund with no declared method is relieved oldest-first by custom rather
    /// than by election, and the two are indistinguishable once the absence has
    /// been defaulted away. That mattered the moment a screen reported the
    /// method as "a term of the administration agreement": on the seeded demo
    /// books, whose configuration declares nothing, it asserted an election
    /// nobody had made.
    ///
    /// Use [`effective_lot_method`] for what the engine should DO, and this
    /// field for whether anyone chose it.
    ///
    /// [`effective_lot_method`]: RuleSet::effective_lot_method
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lot_method: Option<LotMethod>,

    /// Which dimensions play which part when the engine posts a sale.
    ///
    /// ⛔ CONFIGURATION, BECAUSE A CHART IS. The engine cannot guess which
    /// dimension is realized gain — that is a decision about somebody's chart of
    /// accounts, and `Ratio.Lots.Posting.a_collided_chart_hides_the_gain` is
    /// what happens when two roles point at one dimension.
    #[serde(default)]
    pub chart_roles: Option<ChartRoles>,

    /// How long a holding must be held for its gain to be long-term, in days.
    ///
    /// ⛔ A JURISDICTION'S NUMBER, NOT ARITHMETIC — the same reason
    /// `Ratio.Lots.Methods.isLongTerm` takes the threshold as a PARAMETER
    /// rather than writing 365 into the definition. It is a term of the same
    /// agreement that names the method, a fund administered under other rules
    /// uses a different one, and hard-coding it makes the engine wrong
    /// somewhere instead of configurable everywhere.
    ///
    /// ⚠ THE BOUNDARY IS ON THE DAY. A lot held exactly this many days is
    /// long-term: `Ratio.Lots.Methods.the_threshold_day_is_long_term`. Off by
    /// one moves a disposal between tax rates, and the resulting figure looks
    /// entirely ordinary.
    #[serde(default = "default_long_term_days")]
    pub long_term_days: i64,

    /// How big a difference has to be before it stops the NAV.
    ///
    /// ⛔ `None` MEANS NOBODY SAID, AND THAT IS NOT THE SAME AS SAYING 5.00 AND
    /// 1,000.00. A fund with no declared tolerance has its breaks graded by
    /// custom rather than by agreement, and the two are indistinguishable once
    /// the absence has been defaulted away — the same distinction
    /// [`lot_method`] keeps, and for the same reason: a screen calling the
    /// grading "a term of the administration agreement" would be asserting an
    /// agreement nobody made.
    ///
    /// Use [`effective_tolerance`] for what the grader should USE, and this
    /// field for whether anyone chose it.
    ///
    /// [`lot_method`]: RuleSet::lot_method
    /// [`effective_tolerance`]: RuleSet::effective_tolerance
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tolerance: Option<Tolerance>,
}

fn default_long_term_days() -> i64 {
    365
}

/// ⛔ HAND-WRITTEN, NOT DERIVED, AND THAT IS THE POINT. `#[derive(Default)]`
/// gives every field its type's default, so `long_term_days` would be **0**
/// here while `#[serde(default = ...)]` makes it 365 on the way in from TOML.
/// One fact with two answers, differing by which door the rule set came
/// through — and a threshold of zero classifies every disposal ever made as
/// long-term, at the favorable rate, silently.
impl Default for RuleSet {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            lot_method: None,
            chart_roles: None,
            long_term_days: default_long_term_days(),
            tolerance: None,
        }
    }
}

impl RuleSet {
    /// The grading the console should USE: what was declared, or the custom
    /// bands.
    ///
    /// ⚠ BY CUSTOM, NOT BECAUSE THE ENGINE PREFERS THESE NUMBERS. Every caller
    /// that needs to GRADE something wants this; the only caller that wants the
    /// raw field is one reporting whether a fund actually chose.
    pub fn effective_tolerance(&self) -> Tolerance {
        self.tolerance.unwrap_or_default()
    }
    /// The method the engine relieves under: what was declared, or FIFO.
    ///
    /// ⚠ FIFO BY CUSTOM, NOT BECAUSE THE ENGINE PREFERS IT. Every caller that
    /// needs to RELIEVE something wants this; the only caller that wants the
    /// raw field is one reporting whether a fund actually elected a method.
    pub fn effective_lot_method(&self) -> LotMethod {
        self.lot_method.unwrap_or_default()
    }
}

/// How serious one difference is. `Ratio.Tolerance.Severity`.
///
/// ⛔ THE CODES ARE THE WIRE'S. 1, 2, 3 are `ratio.console.v1.Severity`'s LOW,
/// MEDIUM and HIGH; its 0 is UNSPECIFIED and is not a grade anything returns.
/// The emitted `severity_of` answers in those numbers so there is no second
/// table for the order to be wrong in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    /// Beneath notice.
    Low,
    /// Worth reporting, and not blocking.
    Medium,
    /// Stops the NAV.
    High,
}

/// How big a difference has to be before it stops the NAV.
///
/// ⛔ A TERM OF AN AGREEMENT, NOT A PROPERTY OF THE SOFTWARE, and it belongs
/// here for the same reason the lot method does: two funds looking at the same
/// break disagree about whether it matters and both are right, changing it is
/// an approval rather than a deployment, and it decides whether somebody's
/// close stops. `Ratio.Tolerance` is the proof side.
///
/// ⚠ MINOR UNITS. A tolerance of 1,000.00 is `100_000`; a fractional one is
/// inexpressible rather than rejected by a check somebody might forget to run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tolerance {
    /// At or above this, a difference is worth reporting.
    pub below_notice: i64,
    /// At or above this, a difference stops the NAV.
    ///
    /// ⚠ THE BOUNDARY IS ON THE AMOUNT. A difference of exactly this blocks:
    /// `Ratio.Tolerance.a_difference_at_the_threshold_blocks_the_nav`. Off by
    /// one moves a break between stopping the close and waiting until tomorrow,
    /// and neither figure looks unusual on a screen.
    pub blocks_nav: i64,
}

/// ⛔ THE CUSTOM GRADING, AND IT IS NOT A `Default` DERIVE. See the note on
/// [`RuleSet::default`]: a derived one gives `blocks_nav` of **0**, and
/// `Ratio.Tolerance.a_tolerance_of_zero_blocks_on_everything` is what that
/// makes of every fund — every difference, including a difference of nothing,
/// grading as one that stops the NAV.
impl Default for Tolerance {
    fn default() -> Self {
        Self { below_notice: 500, blocks_nav: 100_000 }
    }
}

impl Tolerance {
    /// ⛔ CHECKED WHEN THE CONFIGURATION IS READ, not when a break is graded.
    /// Bounds the wrong way round are not a strict tolerance or a lenient one:
    /// they leave a grade nothing can ever be —
    /// `Ratio.Tolerance.an_inverted_tolerance_makes_the_middle_band_unreachable`
    /// — so the queue offers "in review" and no difference can reach it. A
    /// chart that cannot express a gain is refused on the way in for exactly
    /// this reason; so is this.
    pub fn check(&self) -> Result<()> {
        if !generated_tolerance::tolerance_is_well_formed(self.below_notice, self.blocks_nav) {
            bail!(
                "tolerance is not usable as written: below_notice {} and blocks_nav {} must \
                 both be at least zero, and below_notice must not exceed blocks_nav. As \
                 written, nothing can be reportable-and-not-blocking.",
                self.below_notice,
                self.blocks_nav
            );
        }
        Ok(())
    }

    /// What one difference grades as.
    ///
    /// ⛔ THE MAGNITUDE IS TAKEN HERE, BEFORE THE EMITTED CODE IS ASKED
    /// ANYTHING. `Ratio.Tolerance.severity` reads a size, and taking one means
    /// negating — the single operation in this path that `i64` cannot always
    /// do. `i64::MIN` has no positive counterpart, `abs()` panics on it, and a
    /// wrapping negation would hand the grader a difference that never
    /// happened. So it is `checked_abs`, and a difference with no representable
    /// magnitude grades HIGH: see [`Self::severity`]'s caller contract in
    /// `ratio-console`, and `Ratio.Bounded` for the general shape.
    pub fn severity(&self, difference: i64) -> Severity {
        let Some(magnitude) = difference.checked_abs() else {
            return Severity::High;
        };
        match generated_tolerance::severity_of(magnitude, self.below_notice, self.blocks_nav) {
            3 => Severity::High,
            2 => Severity::Medium,
            _ => Severity::Low,
        }
    }
}

/// The dimensions a sale posts to. `Ratio.Lots.Posting.Accounts`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChartRoles {
    pub investments: i64,
    pub cash: i64,
    /// ⛔ Where the difference lands. A partition like any other — the gain is
    /// not a fourth kind of thing, it is value sitting in a different account.
    pub realized_gain: i64,

    /// Where both sides of a currency exchange land.
    ///
    /// ⛔ ONE ACCOUNT, TAKING BOTH LEGS, WHICH IS WHAT RECORDS THE RATE.
    /// `Ratio.Chart.Dimensions.an_exchange_conserves_and_leaves_the_rate_behind`
    /// — a hundred dollars out and ninety euros in, both through here, leaves
    /// `+10000 USD` and `−9000 EUR` sitting in one place. That pair IS the rate,
    /// as it was actually struck, rather than something inferred later from two
    /// unrelated legs.
    ///
    /// ⚠ ITS FLAT TOTAL ACROSS CURRENCIES IS NOT ZERO AND MUST NOT BE NETTED.
    /// That residual is the fund's FX position. Separating it into a currency
    /// gain and a security gain is a modelling question this does not answer.
    ///
    /// ⚠ OPTIONAL, because a single-currency book never exchanges anything and
    /// requiring it would refuse every chart written before this existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency_conversion: Option<i64>,
}

impl ChartRoles {
    /// ⛔ THE THREE MUST BE DIFFERENT DIMENSIONS.
    /// `Ratio.Lots.Posting.Accounts.distinct` — a hypothesis the proofs turned
    /// out to need, and its absence is a real way a chart goes wrong. Map the
    /// realized gain to the same dimension as investments and the gain NETS
    /// AGAINST the disposal: the income account reports what is left over rather
    /// than what was earned, the entry conserves, the trial balance ties, and the
    /// taxable income is nowhere.
    ///
    /// ⚠ Checked when the configuration is READ, not when a sale is posted. A
    /// chart that cannot express a gain is wrong the moment it is written down,
    /// and finding out at the first disposal means finding out in production.
    pub fn check(&self) -> Result<()> {
        // ⚠ THE CONVERSION ROLE IS IN THE SWEEP WHEN IT IS DECLARED. Colliding
        // it with cash would net an exchange away against the balance it was
        // supposed to move — the rate would be nowhere and the entry would
        // still conserve in both currencies.
        let mut named = vec![
            ("investments", self.investments),
            ("cash", self.cash),
            ("realized gain", self.realized_gain),
        ];
        if let Some(c) = self.currency_conversion {
            named.push(("currency conversion", c));
        }
        for (i, (an, a)) in named.iter().enumerate() {
            for (bn, b) in named.iter().skip(i + 1) {
                if a == b {
                    bail!(
                        "the chart maps {an} and {bn} to the same dimension ({a}). A sale would \
                         post its gain into the account it relieved, so the gain would net \
                         against the disposal and the income account would report what was \
                         left over rather than what was earned — while the entry conserved and \
                         the trial balance tied"
                    );
                }
            }
        }
        Ok(())
    }
}

/// Which lots a sale gives up. `Ratio.Lots.Methods.Order`.
///
/// ⚠ THESE ARE THE METHODS THAT ARE ORDERINGS. Specific identification is a
/// SELECTION the client supplies per sale, and average cost pools the holding so
/// there is no lot to give up — neither is a configuration setting of this
/// shape, and `Ratio.Lots.Methods` models them separately so that adding them
/// here as variants is visibly the wrong move.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LotMethod {
    /// Oldest acquisition first.
    #[default]
    Fifo,
    /// Newest first.
    Lifo,
    /// Dearest PER UNIT first — chosen to reduce a gain.
    Hifo,
    /// Cheapest per unit first — chosen to realize one deliberately, against a
    /// capital-loss carryforward.
    Lofo,
    /// Longest held first. ⛔ REFUSES a holding where any lot has no acquisition
    /// date, rather than defaulting: the epoch makes everything long-term at the
    /// favorable rate on records that do not support it, and today makes
    /// everything short-term on a holding held for years.
    LongestHeldFirst,
    /// Shortest held first.
    ShortestHeldFirst,
}

impl LotMethod {
    /// Every method, so a caller can offer them without listing them.
    ///
    /// ⛔ A LIST NOTHING CHECKS IS A LIST THAT GOES STALE, so
    /// `every_method_round_trips_through_its_declared_name` walks this and
    /// fails if a variant is added without being added here — which would
    /// otherwise show up as a CLI that silently cannot select the new method.
    pub const ALL: [LotMethod; 6] = [
        LotMethod::Fifo,
        LotMethod::Lifo,
        LotMethod::Hifo,
        LotMethod::Lofo,
        LotMethod::LongestHeldFirst,
        LotMethod::ShortestHeldFirst,
    ];

    /// The name this method is written as in a configuration.
    ///
    /// ⚠ THIS IS A SECOND SPELLING OF EVERY VARIANT, and serde owns the first.
    /// They must agree: a fund declaring a method the writer spells differently
    /// from the reader parses as the DEFAULT, silently, and is relieved FIFO
    /// while its agreement says otherwise. `every_method_round_trips_through_
    /// its_declared_name` is what makes the disagreement a build failure rather
    /// than a tax position.
    pub fn as_declared(self) -> &'static str {
        match self {
            LotMethod::Fifo => "fifo",
            LotMethod::Lifo => "lifo",
            LotMethod::Hifo => "hifo",
            LotMethod::Lofo => "lofo",
            LotMethod::LongestHeldFirst => "longest_held_first",
            LotMethod::ShortestHeldFirst => "shortest_held_first",
        }
    }
}

/// A method this engine knows about and deliberately does not offer as an
/// ordering, with the reason.
///
/// ⛔ NAMED RATHER THAN GUESSED. This matches the declared value textually
/// because by the time serde has failed there is no value left to inspect — and
/// a substring search over the whole document would fire on a rule id that
/// happens to contain the word.
fn unsupported_method(toml_src: &str) -> Option<&'static str> {
    let declared = toml_src.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        if k.trim() != "lot_method" {
            return None;
        }
        Some(v.trim().trim_matches('"').to_string())
    })?;
    match declared.as_str() {
        "min_tax" | "mintax" | "minimize_tax" | "tax_minimising" | "tax_minimizing" => Some(
            "min-tax relief is not an ordering, so it is not a lot method: which lot costs \
             least in tax depends on the SALE PRICE and not on the holding. A short-term LOSS \
             is worth more than a long-term one while a short-term GAIN is worth less, so the \
             same two lots invert between two prices. See \
             Ratio.Lots.Methods.a_tax_minimising_method_is_not_a_function_of_the_lots, and \
             issue #9 for the shape it needs instead",
        ),
        "specific_identification" | "specific_id" | "specid" => Some(
            "specific identification is not an ordering, so it is not a lot method: the client \
             names the lots PER SALE, possibly from the middle of a holding and possibly \
             partially. It is an instruction carried on the disposal, not a term of the rule \
             set. See Ratio.Lots.Methods.specific_identification_takes_from_the_middle",
        ),
        "average_cost" | "average" | "pooled" => Some(
            "average cost is not an ordering, so it is not a lot method: it POOLS the holding, \
             so which lot is given up is not a question it answers. It also divides — total \
             cost over total units rarely lands on a whole minor unit — which is a rounding \
             term no ordering method carries. See \
             Ratio.Lots.Methods.average_cost_is_not_a_lot_walk",
        ),
        _ => None,
    }
}

impl RuleSet {
    /// Parse a configuration from TOML.
    ///
    /// A malformed rate — `rate_bp = 75.5` — fails here, with TOML's own error.
    /// That is deliberate: the schema makes a float inexpressible, so there is
    /// no separate "no floats" check to forget to run.
    pub fn from_toml(s: &str) -> Result<Self> {
        let set: Self = toml::from_str(s).map_err(|e| {
            // ⛔ THE THREE THAT ARE NOT ORDERINGS GET THEIR OWN ANSWER. Serde
            // refuses an unknown variant with "configuration is not valid TOML
            // for a rule set", which is FALSE and unhelpful in the same breath:
            // the TOML parsed fine, and the administrator wrote down a method
            // their fund is genuinely administered under. Telling them their
            // file is malformed sends them to look for a typo that is not
            // there.
            //
            // ⚠ AND THESE ARE NOT MISSING FEATURES TO BE ADDED AS VARIANTS.
            // `Ratio.Lots.Methods` proves each is a different SHAPE — a
            // tax-minimising method is not a function of the lots (it needs the
            // sale price), specific identification is a per-sale selection
            // rather than a sort, and average cost pools the holding so "which
            // lot" is not a question it answers. Issue #9 exists because adding
            // one here is the natural and wrong move.
            match unsupported_method(s) {
                Some(m) => anyhow!("{}", m),
                None => anyhow::Error::new(e)
                    .context("configuration is not valid TOML for a rule set"),
            }
        })?;
        // ⛔ AT READ TIME. A chart that cannot express a gain is wrong the
        // moment it is written down; finding out at the first disposal means
        // finding out in production.
        if let Some(r) = &set.chart_roles {
            r.check()?;
        }
        // ⛔ SAME PLACEMENT, SAME REASON. A tolerance whose bounds are the wrong
        // way round has a grade nothing can ever be, and finding that out means
        // noticing that a category on the exceptions screen is always empty —
        // which nobody does.
        if let Some(t) = &set.tolerance {
            t.check()?;
        }
        Ok(set)
    }

    /// Serialize back to TOML — the canonical form that gets content-addressed.
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self).context("serializing rule set")
    }

    /// Find a rule by id.
    pub fn rule(&self, id: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

/// The event a rule is applied to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// Which rule to apply.
    pub rule: String,
    /// Event identifier, carried onto the journal entry.
    pub id: String,
    /// For `trade` / `dividend`: the amount, in minor units. For `accrual`:
    /// the basis the rate is applied to.
    pub amount: i64,
    /// Accrual only: how many days of the period this covers.
    #[serde(default)]
    pub days: Option<i64>,
    #[serde(default)]
    pub memo: String,

    /// The instrument this event concerns, if any. Legs that declare
    /// `per_instrument` carry it; the rest do not.
    #[serde(default)]
    pub instrument: Option<String>,

    /// Whole units moved. ⚠ A MEASURE, not a conserved quantity — see
    /// `PostingRecord::quantity`.
    #[serde(default)]
    pub quantity: Option<i64>,
}

/// Why a rule was rejected, in words a fund accountant would use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// The rule this concerns.
    pub rule: String,
    /// What is wrong, and what to do about it.
    pub message: String,
    /// A question for the author rather than an outright error — the rule may
    /// be right, but somebody has to say so.
    pub is_question: bool,
}

impl Finding {
    fn error(rule: &str, message: impl Into<String>) -> Self {
        Finding {
            rule: rule.to_string(),
            message: message.into(),
            is_question: false,
        }
    }
    fn question(rule: &str, message: impl Into<String>) -> Self {
        Finding {
            rule: rule.to_string(),
            message: message.into(),
            is_question: true,
        }
    }
}

/// Check a rule set against the chart of accounts.
///
/// Returns every finding rather than stopping at the first, so an author fixes
/// a configuration in one pass instead of one round trip per mistake.
///
/// The obligations, and why each is here:
///
/// * **the template balances** — the load-bearing one. Verified once here;
///   `Ratio.Chart.balanced_template_balances` then guarantees every posting the
///   rule ever emits balances, at any amount.
/// * **every account exists** — a rule referencing an account nobody has
///   defined posts into a dimension with no name on a report.
/// * **an accrual has a rate and a convention** — without both, the amount is
///   not computable and the rule is not a rule.
/// * **a non-accrual has neither** — a rate on a trade rule means somebody
///   expected it to do something it will not do.
///
/// There is deliberately **no per-leg normal-side check**. Crediting cash is
/// what every purchase does; flagging it would fire on nearly every real rule.
/// Normal side is a property of an account's *balance*, not of a posting, so
/// the indicator lives on the trial balance where it means something. A check
/// that cries wolf trains people to ignore the checks — which is the same
/// failure PLAN.md names for false breaks in a shadow run.
pub fn check(set: &RuleSet, chart: &[Account]) -> Vec<Finding> {
    let by_dim: BTreeMap<i64, &Account> = chart.iter().map(|a| (a.dim, a)).collect();
    let mut out = Vec::new();

    for rule in &set.rules {
        if rule.legs.is_empty() {
            out.push(Finding::error(
                &rule.id,
                "has no postings, so it cannot record anything. Add at least a debit and a credit.",
            ));
            continue;
        }

        // ── the template balances ──────────────────────────────────────────
        // Reuse the kernel's own check, over the weights. This is the same
        // predicate `Ratio.Core.Balanced` the ledger enforces, applied one
        // level up — so a template that passes here cannot produce a posting
        // that fails there.
        let as_txn = Transaction {
            postings: rule
                .legs
                .iter()
                .map(|l| Posting {
                    dim: l.account,
                    amount: l.weight,
                })
                .collect(),
        };
        if !transaction_is_balanced(&as_txn) {
            let net: i64 = rule.legs.iter().map(|l| l.weight).sum();
            out.push(Finding::error(
                &rule.id,
                format!(
                    "does not balance: the posting weights net to {net}, not 0. \
                     Every debit needs a matching credit — check the signs."
                ),
            ));
        }

        // ── every account exists, and sits on a sensible side ──────────────
        for leg in &rule.legs {
            match by_dim.get(&leg.account) {
                None => out.push(Finding::error(
                    &rule.id,
                    format!(
                        "posts to account {} which is not in the chart of accounts. \
                         Add it, or point the rule at an existing account.",
                        leg.account
                    ),
                )),
                Some(account) => {
                    if leg.weight == 0 {
                        out.push(Finding::error(
                            &rule.id,
                            format!(
                                "gives {} a weight of 0, so the leg never posts anything. \
                                 Remove it, or give it a weight.",
                                account.display_name
                            ),
                        ));
                    }
                }
            }
        }

        // ── accruals need a rate and a convention; others need neither ─────
        match rule.kind {
            RuleKind::Accrual => {
                if rule.rate_bp.is_none() {
                    out.push(Finding::error(
                        &rule.id,
                        "is an accrual with no rate. Set `rate_bp` — 75 basis points a year is `rate_bp = 75`.",
                    ));
                }
                if rule.day_count.is_none() {
                    out.push(Finding::question(
                        &rule.id,
                        "is an accrual with no day-count convention. Which applies — act/365, act/360 or 30/360?",
                    ));
                }
                if let Some(rate) = rule.rate_bp {
                    if rate < 0 {
                        out.push(Finding::error(
                            &rule.id,
                            format!("has a negative rate ({rate} bp). A rebate is a separate rule, not a negative fee."),
                        ));
                    }
                }
            }
            _ => {
                if rule.rate_bp.is_some() || rule.day_count.is_some() {
                    out.push(Finding::question(
                        &rule.id,
                        format!(
                            "is a {} rule but carries a rate or day-count, which only \
                             accruals use. They will be ignored — did you mean `kind = \"accrual\"`?",
                            kind_word(rule.kind)
                        ),
                    ));
                }
            }
        }
    }

    out
}


fn kind_word(k: RuleKind) -> &'static str {
    match k {
        RuleKind::Trade => "trade",
        RuleKind::Dividend => "dividend",
        RuleKind::Accrual => "accrual",
        RuleKind::Mark => "mark",
    }
}

/// Apply a rule to an event, producing postings.
///
/// The postings are balanced **by theorem** provided the rule passed [`check`]:
/// `Ratio.Chart.balanced_template_balances` says a template whose weights net
/// to zero nets to zero at every amount. This function is therefore not
/// permitted to invent a leg or adjust a total — it scales, and nothing else.
pub fn compile(rule: &Rule, event: &Event) -> Result<Vec<PostingRecord>> {
    let amount = match rule.kind {
        // A mark's amount is the delta its caller computed from the carrying
        // value; the rule decides only where it lands.
        RuleKind::Trade | RuleKind::Dividend | RuleKind::Mark => event.amount,
        RuleKind::Accrual => accrual_amount(rule, event)?,
    };
    rule.legs
        .iter()
        .map(|leg| {
            // `Ratio.Chart.applyTemplate`: weight × amount, per leg.
            //
            // ⛔ CHECKED, BECAUSE THIS IS THE PRODUCTION POSTING PATH. Every
            // figure in every book comes through this multiply, and an
            // overflowing product does not look wrong — it wraps to a plausible
            // number of the other sign. The legs would still net to zero, so the
            // door lets it through and the trial balance ties on it.
            let value = ratio_common::checked::mul(leg.weight, amount, "a posting leg")?;
            Ok(match (leg.per_instrument, event.instrument.as_deref()) {
                (true, Some(i)) => PostingRecord::of(
                    leg.account,
                    value,
                    i,
                    // The quantity follows the leg's SIGN: a purchase debits
                    // investments and adds shares; the disposal leg credits and
                    // removes them. Taking the event's sign instead would add
                    // shares on a sale.
                    event.quantity.map(|q| if leg.weight < 0 { -q } else { q }),
                ),
                _ => PostingRecord::new(leg.account, value),
            })
        })
        .collect()
}

/// The accrued amount: `basis × rate × days ÷ (10 000 × denominator)`.
///
/// Computed in `i128` because the numerator overflows `i64` on a large book:
/// a 20-billion-unit basis at 75bp over 365 days is ~5.5e17 before division,
/// which is within `i64` but leaves no headroom, and a bigger book or a longer
/// period would silently wrap. Rounded half-up rather than truncated — an
/// accrual that always rounds toward zero under-accrues every single day, and
/// the error compounds across a quarter.
fn accrual_amount(rule: &Rule, event: &Event) -> Result<i64> {
    let rate = rule
        .rate_bp
        .with_context(|| format!("rule {:?} is an accrual with no rate", rule.id))?;
    let day_count = rule
        .day_count
        .with_context(|| format!("rule {:?} is an accrual with no day-count", rule.id))?;
    let days = event
        .days
        .with_context(|| format!("event {:?} accrues but does not say over how many days", event.id))?;
    if days < 0 {
        anyhow::bail!("event {:?} accrues over {days} days", event.id);
    }

    let numerator = (event.amount as i128) * (rate as i128) * (days as i128);
    let denominator = 10_000i128 * day_count.denominator();
    let rounded = round_half_up(numerator, denominator);
    i64::try_from(rounded)
        .with_context(|| format!("accrual for {:?} does not fit in i64", event.id))
}

/// Integer division rounding half away from zero.
fn round_half_up(numerator: i128, denominator: i128) -> i128 {
    debug_assert!(denominator > 0);
    if numerator >= 0 {
        (numerator + denominator / 2) / denominator
    } else {
        -((-numerator + denominator / 2) / denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratio_store::AccountTypeRecord as A;

    #[test]
    fn the_three_that_are_not_orderings_are_refused_by_name() {
        // ⛔ REFUSED WITH THE REASON, not with "your TOML is malformed". Serde
        // rejects an unknown variant, so a fund administered under average cost
        // was told its configuration file was invalid — which is false and
        // unhelpful in the same breath. The TOML parsed; the method is a
        // different SHAPE, and the message now says which and why.
        //
        // ⚠ AND THESE ARE NOT MISSING VARIANTS. `Ratio.Lots.Methods` proves
        // each is a different shape, and issue #9 exists because adding one to
        // `LotMethod` is the natural and wrong move.
        for (name, must_say) in [
            ("min_tax", "SALE PRICE"),
            ("mintax", "SALE PRICE"),
            ("specific_identification", "PER SALE"),
            ("average_cost", "POOLS"),
            ("pooled", "POOLS"),
        ] {
            let e = RuleSet::from_toml(&format!("lot_method = \"{name}\"\nrules = []\n"))
                .expect_err("not an ordering, so it cannot be a lot method")
                .to_string();
            assert!(
                e.contains(must_say),
                "{name} is refused without saying why — a reader is sent to look for a typo \
                 that is not there. Got: {e}"
            );
            assert!(
                !e.contains("not valid TOML"),
                "{name} is refused as malformed TOML, which it is not. Got: {e}"
            );
        }

        // And an actual typo still reads as one.
        let e = RuleSet::from_toml("lot_method = \"fifoo\"\nrules = []\n")
            .expect_err("not a method")
            .to_string();
        assert!(e.contains("not valid TOML"), "a genuine typo should not be explained away: {e}");
    }

    #[test]
    fn every_method_round_trips_through_its_declared_name() {
        // ⛔ TWO SPELLINGS OF EVERY VARIANT — serde's and `as_declared`'s — and
        // a disagreement is silent in the worst way. TOML that names a method
        // the reader does not recognize does not fail; `LotMethod` derives
        // `Default`, so the fund is relieved FIFO while its agreement says
        // HIFO, the trial balance ties, and only the taxable gain is somebody
        // else's number.
        //
        // ⚠ AND WALKING `ALL` IS WHAT CATCHES AN ADDED VARIANT. A test naming
        // the six by hand proves the six agree and says nothing about the
        // seventh, which is the one that would be wrong.
        for m in LotMethod::ALL {
            let toml = format!("lot_method = \"{}\"\nrules = []\n", m.as_declared());
            let back = RuleSet::from_toml(&toml)
                .unwrap_or_else(|e| panic!("{:?} declares itself as {:?}, which does not parse: {e}", m, m.as_declared()))
                .lot_method;
            assert_eq!(back, Some(m), "{:?} does not survive its own declared name", m);
        }
        assert_eq!(
            LotMethod::ALL.len(),
            LotMethod::ALL
                .iter()
                .map(|m| m.as_declared())
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            "ALL lists a method twice, or two methods share a declared name"
        );
    }

    fn chart() -> Vec<Account> {
        vec![
            acct(1, "Investments at fair value", A::Asset),
            acct(2, "Cash and equivalents", A::Asset),
            acct(10, "Management fee expense", A::Expense),
            acct(30, "Dividend income", A::Income),
            acct(40, "Management fee payable", A::Liability),
        ]
    }
    fn acct(dim: i64, name: &str, t: A) -> Account {
        Account {
            dim,
            display_name: name.into(),
            account_type: t,
        }
    }

    const FEE_TOML: &str = r#"
[[rule]]
id = "management_fee_accrual"
kind = "accrual"
description = "Management fee, 75bp a year on prior-day net assets"
rate_bp = 75
day_count = "act/365"
[[rule.posting]]
account = 10
weight = 1
[[rule.posting]]
account = 40
weight = -1
"#;

    #[test]
    fn a_rule_set_round_trips_through_toml() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        assert_eq!(set.rules.len(), 1);
        let r = set.rule("management_fee_accrual").unwrap();
        assert_eq!(r.kind, RuleKind::Accrual);
        assert_eq!(r.rate_bp, Some(75));
        assert_eq!(r.day_count, Some(DayCount::Act365));
        let back = RuleSet::from_toml(&set.to_toml().unwrap()).unwrap();
        assert_eq!(back, set);
    }

    #[test]
    fn a_fractional_rate_cannot_be_expressed() {
        // No "reject floats" check exists, because the schema makes a float
        // unrepresentable. This is the enforcement.
        let bad = FEE_TOML.replace("rate_bp = 75", "rate_bp = 75.5");
        assert!(RuleSet::from_toml(&bad).is_err());
    }

    #[test]
    fn a_good_rule_has_nothing_to_say_about_it() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        assert_eq!(check(&set, &chart()), vec![]);
    }

    #[test]
    fn an_unbalanced_template_is_rejected_with_the_shortfall() {
        let bad = FEE_TOML.replace("weight = -1", "weight = -2");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("net to -1"), "{}", f[0].message);
        assert!(!f[0].is_question);
    }

    #[test]
    fn an_unknown_account_is_rejected_by_number() {
        let bad = FEE_TOML.replace("account = 40", "account = 999");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert!(f.iter().any(|x| x.message.contains("999")
            && x.message.contains("not in the chart")));
    }

    /// A purchase credits cash, and cash is an asset. If the checker flagged
    /// that, it would fire on almost every rule a fund has — the false-positive
    /// failure PLAN.md warns about, one level up from shadow-run breaks.
    #[test]
    fn crediting_an_asset_is_not_flagged() {
        let buy = r#"
[[rule]]
id = "buy_equity"
kind = "trade"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1
"#;
        assert_eq!(check(&RuleSet::from_toml(buy).unwrap(), &chart()), vec![]);
    }

    /// …and debiting income is not flagged either, for the same reason: a
    /// reversal is ordinary. Normal side is a property of a BALANCE, and the
    /// trial balance is where it is reported.
    #[test]
    fn debiting_income_is_not_flagged_either() {
        let rev = r#"
[[rule]]
id = "dividend_reversal"
kind = "dividend"
[[rule.posting]]
account = 30
weight = 1
[[rule.posting]]
account = 2
weight = -1
"#;
        assert_eq!(check(&RuleSet::from_toml(rev).unwrap(), &chart()), vec![]);
    }

    #[test]
    fn an_accrual_without_a_rate_is_rejected_with_an_example() {
        let bad = FEE_TOML.replace("rate_bp = 75\n", "");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        let e = f.iter().find(|x| !x.is_question).unwrap();
        assert!(e.message.contains("rate_bp = 75"), "{}", e.message);
    }

    #[test]
    fn an_accrual_without_a_day_count_asks_which_one() {
        let bad = FEE_TOML.replace("day_count = \"act/365\"\n", "");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        let q = f.iter().find(|x| x.is_question).unwrap();
        assert!(q.message.contains("act/365"), "{}", q.message);
    }

    #[test]
    fn a_zero_weight_leg_is_rejected() {
        let bad = FEE_TOML.replace("weight = 1\n", "weight = 0\n");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert!(f.iter().any(|x| x.message.contains("weight of 0")));
    }

    #[test]
    fn a_rule_with_no_postings_is_rejected() {
        let set = RuleSet::from_toml(
            "[[rule]]\nid = \"empty\"\nkind = \"trade\"\n",
        )
        .unwrap();
        let f = check(&set, &chart());
        assert_eq!(f.len(), 1);
        assert!(f[0].message.contains("no postings"));
    }

    #[test]
    fn compiling_a_trade_scales_the_template() {
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "buy"
kind = "trade"
[[rule.posting]]
account = 1
weight = 1
[[rule.posting]]
account = 2
weight = -1
"#,
        )
        .unwrap();
        let out = compile(
            set.rule("buy").unwrap(),
            &Event {
                rule: "buy".into(),
                id: "t1".into(),
                amount: 1_690_421_107,
                days: None,
                memo: String::new(), instrument: None, quantity: None },
        )
        .unwrap();
        assert_eq!(out[0].amount, 1_690_421_107);
        assert_eq!(out[1].amount, -1_690_421_107);
    }

    #[test]
    fn a_leg_whose_product_would_wrap_is_refused_rather_than_wrapped() {
        // ⛔ THE PRODUCTION POSTING PATH, and it multiplied unguarded. The
        // wrapped product is the failure this repo keeps finding: both legs
        // wrap by the same magnitude in opposite directions, so they still net
        // to zero, the door admits the entry, and the trial balance ties on a
        // pair of figures with the wrong sign and the wrong size.
        let set = RuleSet::from_toml(
            r#"
[[rule]]
id = "buy"
kind = "trade"
[[rule.posting]]
account = 1
weight = 4
[[rule.posting]]
account = 2
weight = -4
"#,
        )
        .unwrap();
        let huge = i64::MAX / 3;
        let err = compile(
            set.rule("buy").unwrap(),
            &Event {
                rule: "buy".into(),
                id: "t1".into(),
                amount: huge,
                days: None,
                memo: String::new(),
                instrument: None,
                quantity: None,
            },
        )
        .unwrap_err();
        assert!(format!("{err:#}").contains("a posting leg"), "{err:#}");

        // And what the unguarded version would have produced: a number that is
        // negative, plausible, and nowhere near four times the amount.
        assert!(4i64.wrapping_mul(huge) < 0);
    }

    /// The property `Ratio.Chart.balanced_template_balances` states, exercised
    /// over a range of amounts rather than argued about.
    #[test]
    fn a_checked_rule_balances_at_every_amount() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        assert!(check(&set, &chart()).is_empty());
        let rule = set.rule("management_fee_accrual").unwrap();
        for basis in [0i64, 1, 97, 100_000, 1_750_000_000, 20_000_000_000] {
            for days in [0i64, 1, 30, 91, 365] {
                let out = compile(
                    rule,
                    &Event {
                        rule: rule.id.clone(),
                        id: "a".into(),
                        amount: basis,
                        days: Some(days),
                        memo: String::new(), instrument: None, quantity: None },
                )
                .unwrap();
                let net: i64 = out.iter().map(|p| p.amount).sum();
                assert_eq!(net, 0, "basis={basis} days={days} did not balance");
            }
        }
    }

    #[test]
    fn an_accrual_is_computed_in_integers_and_rounds_half_up() {
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        let rule = set.rule("management_fee_accrual").unwrap();
        let at = |basis: i64, days: i64| {
            compile(
                rule,
                &Event {
                    rule: rule.id.clone(),
                    id: "a".into(),
                    amount: basis,
                    days: Some(days),
                    memo: String::new(), instrument: None, quantity: None },
            )
            .unwrap()[0]
                .amount
        };
        // 1,750,000,000 minor units at 75bp for 1 day, act/365:
        //   1_750_000_000 * 75 * 1 / (10_000 * 365) = 35_958.9… -> 35_959
        assert_eq!(at(1_750_000_000, 1), 35_959);
        // A full year returns the whole 75bp.
        assert_eq!(at(1_750_000_000, 365), 13_125_000);
        // Zero days accrues nothing.
        assert_eq!(at(1_750_000_000, 0), 0);
    }

    #[test]
    fn a_large_book_does_not_overflow_the_accrual() {
        // The i128 intermediate is what makes this safe: the numerator here is
        // ~2.7e19, which does not fit in i64 at all.
        let set = RuleSet::from_toml(FEE_TOML).unwrap();
        let rule = set.rule("management_fee_accrual").unwrap();
        let out = compile(
            rule,
            &Event {
                rule: rule.id.clone(),
                id: "big".into(),
                amount: 1_000_000_000_000, // 10 billion units
                days: Some(365),
                memo: String::new(), instrument: None, quantity: None },
        )
        .unwrap();
        assert_eq!(out[0].amount, 7_500_000_000);
        assert_eq!(out.iter().map(|p| p.amount).sum::<i64>(), 0);
    }

    #[test]
    fn rounding_is_symmetric_about_zero() {
        assert_eq!(round_half_up(5, 10), 1);
        assert_eq!(round_half_up(4, 10), 0);
        assert_eq!(round_half_up(-5, 10), -1);
        assert_eq!(round_half_up(-4, 10), 0);
    }

    #[test]
    fn every_finding_is_reported_not_just_the_first() {
        let bad = FEE_TOML
            .replace("weight = -1", "weight = -2")
            .replace("account = 10", "account = 999");
        let f = check(&RuleSet::from_toml(&bad).unwrap(), &chart());
        assert!(f.len() >= 2, "{f:?}");
    }

    #[test]
    fn a_silent_configuration_declares_no_method_but_still_relieves() {
        // ⛔ THE DISTINCTION A DEFAULT DESTROYS. A rule set that says nothing
        // about lots is relieved oldest-first by CUSTOM; one that says "fifo"
        // is relieved oldest-first by ELECTION. The engine does the same thing
        // either way, and only one of them is a term somebody agreed to.
        //
        // ⚠ Found on the live demo, where the console reported the default as
        // "a term of the administration agreement" on three funds whose
        // configuration declares nothing at all.
        let silent = RuleSet::from_toml("rules = []\n").unwrap();
        assert_eq!(silent.lot_method, None, "nobody said");
        assert_eq!(silent.effective_lot_method(), LotMethod::Fifo, "and it still relieves");

        let elected = RuleSet::from_toml("rules = []\nlot_method = \"fifo\"\n").unwrap();
        assert_eq!(elected.lot_method, Some(LotMethod::Fifo), "somebody said");
        assert_eq!(elected.effective_lot_method(), LotMethod::Fifo);

        // ⛔ The two are the SAME to the engine and DIFFERENT to a reader, which
        // is the whole point — an assertion on the effective method alone
        // passes whether or not the distinction survives.
        assert_eq!(silent.effective_lot_method(), elected.effective_lot_method());
        assert_ne!(silent.lot_method, elected.lot_method);

        // And a declared non-default still round-trips.
        let hifo = RuleSet::from_toml("rules = []\nlot_method = \"hifo\"\n").unwrap();
        assert_eq!(hifo.effective_lot_method(), LotMethod::Hifo);
        assert_eq!(RuleSet::default().lot_method, None);
    }

    #[test]
    fn the_holding_period_threshold_is_the_same_number_through_either_door() {
        // ⛔ `#[derive(Default)]` WOULD HAVE MADE THIS 0. A rule set built in
        // Rust and one parsed from TOML would then disagree about the threshold
        // by 365 days, and a threshold of zero makes every disposal ever made
        // long-term — the favorable rate, on every fund, silently.
        assert_eq!(RuleSet::default().long_term_days, 365);
        assert_eq!(RuleSet::from_toml("rules = []\n").unwrap().long_term_days, 365);

        // And a fund administered under other rules says so.
        let set = RuleSet::from_toml("rules = []\nlong_term_days = 730\n").unwrap();
        assert_eq!(set.long_term_days, 730);
    }

    #[test]
    fn a_tolerance_nobody_declared_is_absent_rather_than_zero() {
        // ⛔ THE SAME DISTINCTION `lot_method` KEEPS. A fund that said nothing
        // is graded by custom; a fund that wrote these two numbers down is
        // graded by agreement. Collapsing them lets a screen report a term
        // nobody agreed to — which is exactly what happened with the lot method
        // on the seeded books.
        let silent = RuleSet::from_toml("rules = []\n").unwrap();
        assert_eq!(silent.tolerance, None, "nobody said");
        assert_eq!(
            silent.effective_tolerance(),
            Tolerance { below_notice: 500, blocks_nav: 100_000 },
            "and it still grades"
        );

        let declared =
            RuleSet::from_toml("rules = []\n[tolerance]\nbelow_notice = 100\nblocks_nav = 250\n")
                .unwrap();
        assert_eq!(declared.tolerance, Some(Tolerance { below_notice: 100, blocks_nav: 250 }));
    }

    #[test]
    fn the_tolerance_bands_are_the_same_numbers_through_either_door() {
        // ⛔ `#[derive(Default)]` WOULD HAVE MADE `blocks_nav` 0, and
        // `Ratio.Tolerance.a_tolerance_of_zero_blocks_on_everything` says what
        // that grades: everything, including a difference of nothing at all.
        // Every fund blocked, on every close, depending on which door its
        // configuration came through.
        assert_eq!(RuleSet::default().effective_tolerance(), Tolerance::default());
        assert_eq!(
            RuleSet::from_toml("rules = []\n").unwrap().effective_tolerance(),
            Tolerance::default()
        );
        assert_eq!(Tolerance::default().blocks_nav, 100_000);
        assert_ne!(Tolerance::default().blocks_nav, 0, "the derive would have said 0");
    }

    #[test]
    fn a_tolerance_whose_bands_are_inverted_fails_the_parse() {
        // ⛔ NOT A STRICTER TOLERANCE — one with a grade nothing can be.
        // `Ratio.Tolerance.an_inverted_tolerance_makes_the_middle_band_
        // unreachable`. Caught when the configuration is READ, because the
        // symptom otherwise is a category on the exceptions screen that is
        // always empty, and nobody notices an absence.
        let e = RuleSet::from_toml(
            "rules = []\n[tolerance]\nbelow_notice = 100000\nblocks_nav = 500\n",
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("not usable as written"), "{e}");
        assert!(e.contains("reportable-and-not-blocking"), "says what is lost: {e}");
    }

    #[test]
    fn a_negative_tolerance_fails_the_parse() {
        // A magnitude is never negative, so a negative bound is not a lenient
        // tolerance either — it is one every difference is at or above.
        let e =
            RuleSet::from_toml("rules = []\n[tolerance]\nbelow_notice = -1\nblocks_nav = 500\n")
                .unwrap_err()
                .to_string();
        assert!(e.contains("not usable as written"), "{e}");
    }

    #[test]
    fn a_fractional_tolerance_cannot_be_expressed() {
        // Minor units, like every other money figure here. The schema refuses
        // it; no check has to remember to.
        assert!(RuleSet::from_toml(
            "rules = []\n[tolerance]\nbelow_notice = 5.00\nblocks_nav = 1000.00\n"
        )
        .is_err());
    }

    #[test]
    fn a_difference_exactly_at_the_threshold_blocks_the_nav() {
        // ⚠ THE BOUNDARY IS ON THE AMOUNT.
        // `Ratio.Tolerance.a_difference_at_the_threshold_blocks_the_nav`. One
        // minor unit either side of this line is the difference between a close
        // that stops and one that does not, and both figures look ordinary.
        let t = Tolerance { below_notice: 500, blocks_nav: 100_000 };
        assert_eq!(t.severity(100_000), Severity::High, "exactly at it blocks");
        assert_eq!(t.severity(99_999), Severity::Medium, "one under does not");
        assert_eq!(t.severity(500), Severity::Medium, "exactly at notice is reportable");
        assert_eq!(t.severity(499), Severity::Low, "one under is beneath notice");
    }

    #[test]
    fn a_credit_break_grades_the_same_as_a_debit_of_the_same_size() {
        // `Ratio.Tolerance.severity_reads_only_the_magnitude`. Our figure being
        // under theirs is exactly as serious as being over, and a grading that
        // read the sign would let half of every category through ungraded.
        let t = Tolerance::default();
        for d in [1i64, 499, 500, 99_999, 100_000, 250_000] {
            assert_eq!(t.severity(d), t.severity(-d), "{d} and -{d} grade alike");
        }
    }

    #[test]
    fn the_smallest_difference_an_i64_can_hold_grades_rather_than_panics() {
        // ⛔ `Ratio.Bounded`, in miniature. The theorem is over `Int`, where
        // negation always exists; the running code is `i64`, where `i64::MIN`
        // has no positive counterpart. `abs()` PANICS on it and a wrapping
        // negation would hand the grader a difference that never happened, so
        // the magnitude is taken with `checked_abs` and an unrepresentable one
        // grades HIGH — the direction that costs a look rather than a NAV.
        let t = Tolerance::default();
        assert_eq!(t.severity(i64::MIN), Severity::High);
        assert_eq!(t.severity(i64::MAX), Severity::High);
    }
}
