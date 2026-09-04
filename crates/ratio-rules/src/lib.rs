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

/// Partner allocation cut — named weights, not a partner count.
mod partners;
pub use partners::{
    allocate, apply_facts, check_cut, check_specials, cut_for, AllocationFact, AllocationKind,
    PartnerShare, SpecialAllocation,
};

/// The grading decision, authored in Lean.
mod generated_tolerance;

use std::collections::BTreeMap;

use anyhow::{anyhow, bail, Context, Result};
use ratio_kernel::{transaction_is_balanced, Posting, Transaction};
use ratio_store::{Account, AccountTypeRecord, PostingRecord};
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
    /// `Ratio.Period.one_answer_per_view_per_day` refuses.
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

    /// How many days either side of a sale a repurchase washes the loss.
    ///
    /// ⛔ A JURISDICTION'S NUMBER, exactly like [`long_term_days`].
    /// `Ratio.Lots.Wash.inWashWindow` takes the window as a parameter for the
    /// same reason `isLongTerm` takes 365: a fund administered under other
    /// rules uses a different one, and a constant in the engine would make it
    /// right in one place and wrong in every other.
    /// `Ratio.Lots.Wash.the_window_is_a_jurisdiction_number`.
    ///
    /// ⛔ AND IT REACHES BOTH WAYS. A repurchase before the sale washes it
    /// just as one after does. The number is the half-width, not a horizon.
    ///
    /// ⛔ `None` MEANS NOBODY SAID, AND THAT IS NOT THE SAME AS SAYING 30.
    /// A silent default of 30 restated every in-window loss on every existing
    /// book — the generated fund's short/long split stopped partitioning the
    /// chart total, and `//deploy:seed_test` could not strike a NAV. Thirty
    /// is the US number a fund WRITES; it is not applied to a book that never
    /// elected the rule. Same distinction [`lot_method`] keeps.
    ///
    /// [`long_term_days`]: RuleSet::long_term_days
    /// [`lot_method`]: RuleSet::lot_method
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wash_window_days: Option<i64>,

    /// Short-term tax weight for min-tax relief, relative to long-term = 1.
    ///
    /// ⛔ NOT A `LotMethod`. `Ratio.Lots.MinTax` ranks at a SALE PRICE; an
    /// ordering does not take one. Electing this is a different shape from
    /// `lot_method`, and `lot_method = "min_tax"` stays refused.
    /// `Ratio.Lots.Methods.a_tax_minimising_method_is_not_a_function_of_the_lots`.
    ///
    /// ⛔ `None` MEANS NOBODY SAID, AND THAT IS NOT THE SAME AS SAYING 2.
    /// A silent default would start ranking every existing book at a price
    /// nobody elected, and restating every sale. Two is the Lean example's
    /// weight; it is not applied to a book that never named the rule. Same
    /// distinction [`wash_window_days`] keeps.
    ///
    /// ⛔ AND IT CANNOT SHARE A CONFIGURATION WITH `lot_method`. Two elections
    /// for the same sale is two answers. Read-time refuse, not a silent
    /// precedence.
    ///
    /// [`wash_window_days`]: RuleSet::wash_window_days
    /// [`lot_method`]: RuleSet::lot_method
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_tax_short_weight: Option<i64>,

    /// Whether this book pools the holding at a weighted basis.
    ///
    /// ⛔ NOT A `LotMethod`. `Ratio.Lots.AverageCost` pools the holding;
    /// "which lot" is not a question it answers, and the figure divides.
    /// Electing this is a different shape from `lot_method`, and
    /// `lot_method = "average_cost"` stays refused.
    /// `Ratio.Lots.Methods.average_cost_is_not_a_lot_walk`.
    ///
    /// ⛔ `None` MEANS NOBODY SAID, AND THAT IS NOT THE SAME AS SAYING TRUE.
    /// A silent default would start pooling every existing book and
    /// restating every sale. `Some(true)` elects. `Some(false)` is refused
    /// at read — omit the field. Same distinction [`wash_window_days`]
    /// keeps.
    ///
    /// ⛔ AND IT CANNOT SHARE A CONFIGURATION WITH `lot_method` OR
    /// `min_tax_short_weight`. Two elections for the same sale is two
    /// answers. Read-time refuse, not a silent precedence.
    ///
    /// [`wash_window_days`]: RuleSet::wash_window_days
    /// [`lot_method`]: RuleSet::lot_method
    /// [`min_tax_short_weight`]: RuleSet::min_tax_short_weight
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub average_cost: Option<bool>,

    /// Whether a wash replacement keeps its own acquisition date.
    ///
    /// ⛔ NOT A `LotMethod`, AND NOT `lot_method = "wash"`. The US path
    /// already transfers the holding period through
    /// `Ratio.Lots.Wash.replacementAcquired`. A jurisdiction that does
    /// not transfer is a different rule: the replacement keeps the
    /// repurchase's date. Electing this is a different shape from
    /// `lot_method`.
    /// `Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate`.
    ///
    /// ⛔ `None` MEANS NOBODY SAID, AND THAT IS NOT A SILENT KEEP.
    /// Silence leaves the existing US transfer in place — that path
    /// already landed; this field does not invent a second default.
    /// `Some(true)` elects keep. `Some(false)` is refused at read —
    /// omit the field. Same distinction [`average_cost`] keeps.
    ///
    /// ⛔ AND IT CANNOT BE ELECTED WITHOUT A WASH WINDOW. A holding-
    /// period variant of a rule nobody named is a configuration that
    /// cannot be applied. Read-time refuse, not a silent ignore.
    ///
    /// [`average_cost`]: RuleSet::average_cost
    /// [`wash_window_days`]: RuleSet::wash_window_days
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wash_keep_holding_period: Option<bool>,

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

    /// The books of record this fund keeps.
    ///
    /// ⛔ IN THE CONFIGURATION DOCUMENT, WHICH IS WHAT MAKES A VIEW REPLAYABLE.
    /// Every journal entry pins one config digest, so declaring the views here
    /// means every entry pins EVERY view's terms — and a settlement date
    /// re-derived years later is the one the agreement in force at the time
    /// gives. A per-view chain with its own ACTIVE pointer would leave the
    /// other views' terms pinned by nothing, which is
    /// `Ratio.Actions.Factor.an_unpinned_announcement_changes_the_answer`
    /// wearing a calendar. `//tla:calendar_in_side_file_check` is the run.
    ///
    /// ⚠ EMPTY MEANS ONE VIEW, NOT NO VIEWS. See [`effective_views`].
    ///
    /// [`effective_views`]: RuleSet::effective_views
    #[serde(rename = "view", default, skip_serializing_if = "Vec::is_empty")]
    pub views: Vec<View>,

    /// The calendars a settlement view rolls over.
    ///
    /// ⛔ HERE FOR THE SAME REASON THE VIEWS ARE. A settlement date is a trade
    /// date rolled forward over a calendar; if the calendar is not inside the
    /// prefix a strike pins, the prefix does not determine the figure and a
    /// replay answers differently the moment somebody declares a holiday. That
    /// is worse than a restatement, because a restatement announces itself.
    #[serde(rename = "calendar", default, skip_serializing_if = "Vec::is_empty")]
    pub calendars: Vec<Calendar>,

    /// Project-finance terms. Absent on personal and investment books.
    ///
    /// ⛔ A CONFIGURATION TOTAL, NOT A SECOND LEDGER. Actual costs, WIP and
    /// payables are the journal. Book-level `[project] budget` is the
    /// original contract `/budget` cites. Phase rows (`[[project.phase]]`)
    /// are the per-work-package original baselines `/billing` cites. `None`
    /// (or a phase with no `budget`) means no baseline — not a budget of
    /// zero. Approved change orders are journal facts on their own equity
    /// pair; they do not live here and must not rewrite these keys.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectTerms>,

    /// Household-finance terms. Absent on investment and project books.
    ///
    /// ⛔ A CONFIGURATION TOTAL, NOT A SECOND LEDGER. Actual spend is the
    /// journal's expense accounts (living expenses, taxes — the chart
    /// dimensions). This is the authorized baseline those actuals are
    /// compared to. `None` means no baseline has been set — not a budget
    /// of zero, which would make the first grocery an overrun.
    ///
    /// Envelope grain is `[personal.envelope]`, keyed by chart dimension.
    /// An absent key is unset for that category, not a fake zero.
    ///
    /// `[personal.loan]` is keyed by liability chart dimension; the value
    /// is the paired interest-expense dimension. `None` (or an empty table)
    /// means no named loan — not a roll-forward of zeros. CreateBook seeds
    /// the posting pattern and omits this table.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub personal: Option<PersonalTerms>,

    /// Partner allocation weights. The cut that fills allocated income /
    /// expense / unrealized on `/capital`.
    ///
    /// ⛔ EMPTY MEANS NOBODY SAID, AND THAT IS NOT A SILENT 1/N.
    /// Dividing book NAV by the partner count invents a share nobody
    /// posted. Omit the table. A written `[[partner_cut]]` is the
    /// election. Weights must be positive and partners unique.
    /// `Ratio.Partners.no_cut_is_unset`.
    #[serde(rename = "partner_cut", default, skip_serializing_if = "Vec::is_empty")]
    pub partner_cut: Vec<PartnerShare>,

    /// Standing special allocations by kind, replacing the default cut
    /// for that kind.
    ///
    /// ⛔ A WEIGHT, NOT AN AMOUNT. 100% of expense to the GP is one
    /// row with `weight = 1`. Exact amounts are journal facts.
    /// Empty is silence — that kind uses `partner_cut`, or stays unset.
    /// `Ratio.Partners.cutFor`.
    #[serde(
        rename = "special_allocation",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub special_allocations: Vec<SpecialAllocation>,

    /// Where a period close rolls surplus.
    ///
    /// ⛔ ABSENT MEANS NOBODY SAID, AND THAT IS NOT A DEFAULT TO OPENING
    /// EQUITY OR FUNDING. Closing into the wrong equity account restates
    /// who owns the residual. `None` refuses the close.
    /// `Ratio.Close.missing_destination_refuses_the_close`.
    #[serde(rename = "close", default, skip_serializing_if = "Option::is_none")]
    pub close: Option<CloseTerms>,
}

/// The equity destination a period close rolls surplus into.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloseTerms {
    pub equity_destination: i64,
}

/// The authorized spend a project book cites against the journal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectTerms {
    /// Book-level original contract in minor units. Omitted means unset;
    /// `0` is a set baseline of nothing. `/budget` cites this as baseline;
    /// approved change orders are a different figure, from the journal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<i64>,
    /// Per work-package account. `account` is a chart dimension.
    #[serde(rename = "phase", default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<PhaseBudget>,
}

/// Authorized spend on one work-package account.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseBudget {
    /// Chart dimension — `chart_for(Project)`'s site / structure / finishes
    /// accounts, or an operator-added partition of the same kind.
    pub account: i64,
    /// Minor units. Omitted means this phase has no baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<i64>,
}

impl ProjectTerms {
    /// ⛔ CHECKED WHEN THE CONFIGURATION IS READ. A negative budget inverts
    /// variance; two rows for one account are two answers under one name.
    pub fn check(&self) -> Result<()> {
        if let Some(b) = self.budget {
            if b < 0 {
                bail!(
                    "a project budget is not negative — it is an authorized magnitude, \
                     not a posting"
                );
            }
        }
        let mut seen = std::collections::BTreeSet::new();
        for p in &self.phases {
            if p.account <= 0 {
                bail!(
                    "a phase budget names account {}, which is not a chart dimension",
                    p.account
                );
            }
            if !seen.insert(p.account) {
                bail!(
                    "this configuration declares a phase budget for account {} twice. \
                     A figure cited against that account could not pick one",
                    p.account
                );
            }
            if let Some(b) = p.budget {
                if b < 0 {
                    bail!(
                        "a phase budget is not negative — it is an authorized magnitude, \
                         not a posting"
                    );
                }
            }
        }
        Ok(())
    }
}

/// The authorized spend a household book cites against the journal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonalTerms {
    /// Minor units. Omitted means unset; `0` is a set baseline of nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<i64>,
    /// Chart dimension (decimal string) → minor units. Living expenses is
    /// `"10"`, taxes `"11"` — `chart_for(Personal)`. Absent keys are unset.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub envelope: BTreeMap<String, i64>,
    /// Liability dimension (decimal string) → interest-expense dimension.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub loan: BTreeMap<String, i64>,
}

impl PersonalTerms {
    /// ⛔ CHECKED WHEN THE CONFIGURATION IS READ. A negative budget inverts
    /// variance: every spend would look like remaining authorization. A
    /// loan key that is not a chart dimension, or an interest account
    /// equal to the liability, would put a schedule on a number nobody
    /// can cite.
    pub fn check(&self) -> Result<()> {
        if let Some(b) = self.budget {
            if b < 0 {
                bail!(
                    "a household budget is not negative — it is an authorized magnitude, \
                     not a posting"
                );
            }
        }
        for (k, v) in &self.envelope {
            if k.parse::<i64>().is_err() {
                bail!(
                    "household envelope {k:?} is not a chart dimension — keys are the \
                     decimal dim `chart_for(Personal)` writes (10 living expenses, 11 taxes)"
                );
            }
            if *v < 0 {
                bail!(
                    "household envelope {k} is not negative — it is an authorized magnitude, \
                     not a posting"
                );
            }
        }
        for (k, interest) in &self.loan {
            let dim: i64 = k.parse().map_err(|_| {
                anyhow!(
                    "household loan {k:?} is not a chart dimension — keys are the \
                     decimal dimension (41, 42, 43), not a name"
                )
            })?;
            if dim <= 0 {
                bail!(
                    "household loan {k} is not a chart dimension — dimensions are \
                     positive"
                );
            }
            if *interest <= 0 {
                bail!(
                    "household loan {k} interest is not a chart dimension — \
                     values are the interest-expense dimension (12, 13, 14)"
                );
            }
            if dim == *interest {
                bail!(
                    "household loan {k} cannot use itself as the interest \
                     expense — the liability and the expense are two accounts"
                );
            }
        }
        Ok(())
    }
}

/// How a view decides the day it recognises an entry on.
///
/// ⛔ THREE, AND THE FIRST IS NOT A CONVENTION. `Recorded` is the ABSENCE of an
/// election — the journal's own order, consulting no date — which is what every
/// book has always done and the only basis that answers over the entries
/// carrying no `trade_date`, i.e. most of every book written so far. `Trade`
/// and `Settlement` are elections: they read the record's dates and REFUSE an
/// entry that cannot support them.
///
/// ⚠ COLLAPSING `Recorded` INTO `Settlement { settles_in: 0 }` IS THE MISTAKE
/// THIS ENUM EXISTS TO PREVENT, and it is `lot_method: None` versus
/// `Some(Fifo)` one layer out — a distinction that reached three live funds
/// before it was given a field of its own.
/// `Ratio.Views.nobody_said_is_not_a_settlement_convention`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    #[default]
    Recorded,
    Trade,
    Settlement,
}

impl Basis {
    /// Every basis, so a caller can offer them without listing them.
    ///
    /// ⛔ A LIST NOTHING CHECKS IS A LIST THAT GOES STALE — the same argument as
    /// `LotMethod::ALL`, and `every_basis_round_trips_through_its_declared_name`
    /// is what makes a variant added without being added here a build failure.
    pub const ALL: [Basis; 3] = [Basis::Recorded, Basis::Trade, Basis::Settlement];

    /// How it is written in a configuration.
    pub fn as_declared(self) -> &'static str {
        match self {
            Basis::Recorded => "recorded",
            Basis::Trade => "trade",
            Basis::Settlement => "settlement",
        }
    }
}

/// One book of record over the shared journal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct View {
    /// Stable identifier. Becomes a URL segment and a `NAVS` field.
    pub id: String,
    /// What it is called on a screen. Defaults to the id.
    #[serde(default)]
    pub display_name: String,
    pub basis: Basis,
    /// Open days from trade to settlement. Required by `settlement`, and
    /// meaningless on anything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settles_in: Option<i64>,
    /// Names a `[[calendar]]` in the same document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calendar: Option<String>,
}

/// The days a market is shut.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    /// ISO dates, `YYYY-MM-DD`.
    #[serde(default)]
    pub holidays: Vec<String>,
    /// Weekday numbers the market is shut on, 0 = Sunday.
    ///
    /// ⚠ A LIST RATHER THAN A HARDCODED SATURDAY AND SUNDAY. Gulf funds settle
    /// Sunday to Thursday, and a convention written into the code is a fund the
    /// engine is wrong about instead of one it is configurable for — the same
    /// argument `long_term_days` already makes about 365.
    #[serde(default = "default_weekend")]
    pub weekend: Vec<i64>,
}

fn default_weekend() -> Vec<i64> {
    vec![0, 6]
}

/// The id a book with no declared views has.
///
/// ⛔ NOT `"abor"`, AND NOT ANYTHING THAT READS AS AN ELECTION. A book that
/// declares nothing is not administered on a trade-date basis by agreement; it
/// is folded in journal order by custom. Naming this `abor` would put a claim on
/// a screen that no administration agreement supports.
pub const UNDECLARED_VIEW: &str = "book";

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
            wash_window_days: None,
            min_tax_short_weight: None,
            average_cost: None,
            wash_keep_holding_period: None,
            tolerance: None,
            // ⛔ EMPTY IS NOT ZERO VIEWS. `effective_views` turns this into the
            // one view every book has, recognising in journal order — and
            // `views_declared` reports that nobody chose it. Seeding a view
            // here would make silence look like an election, which is the
            // failure the whole `declared` distinction exists for.
            views: Vec::new(),
            calendars: Vec::new(),
            project: None,
            // ⛔ NONE IS UNSET, NOT AN EMPTY LOAN TABLE. A derived Default
            // that put `Some(PersonalTerms { loan: {} })` here would make
            // "nobody said" indistinguishable from "said, named none".
            personal: None,
            // ⛔ NONE REFUSES THE CLOSE. A derived Default that pointed at
            // Opening equity or Funding would invent a destination nobody
            // named. `Ratio.Close.missing_destination_refuses_the_close`.
            close: None,
            // ⛔ EMPTY IS UNSET, NOT 1/N. A derived Default that invented
            // equal weights from a partner count would print somebody
            // else's share. `Ratio.Partners.no_cut_is_unset`.
            partner_cut: Vec::new(),
            special_allocations: Vec::new(),
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

    /// The views the engine folds: what was declared, or the one every book has.
    ///
    /// ⚠ THE `effective_lot_method` PAIR, EXACTLY. Every caller that needs to
    /// FOLD something wants this; the only caller that wants the raw field is
    /// one reporting whether a fund actually declared a book of record. A book
    /// with no `[[view]]` still has one, it recognises entries in the journal's
    /// own order, and it is not an election — [`views_declared`] is how a
    /// screen tells the two apart.
    ///
    /// [`views_declared`]: RuleSet::views_declared
    pub fn effective_views(&self) -> Vec<View> {
        if self.views.is_empty() {
            return vec![View {
                id: UNDECLARED_VIEW.to_string(),
                display_name: "Journal order".to_string(),
                basis: Basis::Recorded,
                settles_in: None,
                calendar: None,
            }];
        }
        self.views.clone()
    }

    /// Whether anyone declared a book of record, or whether it is the default.
    pub fn views_declared(&self) -> bool {
        !self.views.is_empty()
    }

    /// The view the console opens when it has no other reason to pick one.
    ///
    /// ⚠ THE FIRST DECLARED ONE, IN DOCUMENT ORDER, and that is a decision
    /// rather than an accident: an administrator writing the configuration puts
    /// the official book first, and inferring it from the basis would guess.
    pub fn default_view(&self) -> String {
        self.views
            .first()
            .map(|v| v.id.clone())
            .unwrap_or_else(|| UNDECLARED_VIEW.to_string())
    }

    /// Find a calendar by id.
    pub fn calendar(&self, id: &str) -> Option<&Calendar> {
        self.calendars.iter().find(|c| c.id == id)
    }
}

impl View {
    /// What a screen calls this view.
    pub fn label(&self) -> &str {
        if self.display_name.is_empty() {
            &self.id
        } else {
            &self.display_name
        }
    }

    /// Whether a view is checkable at all, checked when the configuration is
    /// READ.
    ///
    /// ⛔ AT READ TIME, FOR THE REASON `ChartRoles::check` IS. A view that names
    /// a calendar nobody declared is wrong the moment it is written down;
    /// finding out at the first settlement means finding out in production, on
    /// a NAV day, against a figure somebody is about to be paid on.
    pub fn check(&self, calendars: &[Calendar]) -> Result<()> {
        // ⛔ THE ID REACHES A URL SEGMENT AND A TAB-SEPARATED FIELD IN `NAVS`.
        // A tab or a newline would split the strike record in two, and a slash
        // would make the resource name ambiguous — so the charset is narrow and
        // the refusal is at authoring time rather than at write time.
        if self.id.is_empty()
            || !self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bail!(
                "view id {:?} is not usable: an id becomes a URL segment and a field in the \
                 NAVS record, so it must be lowercase letters, digits and hyphens",
                self.id
            );
        }

        match self.basis {
            Basis::Settlement => {
                let days = self.settles_in.ok_or_else(|| {
                    anyhow!(
                        "view {:?} settles but does not say in how many days. Set \
                         `settles_in` — a market settling two days after the trade is \
                         `settles_in = 2`",
                        self.id
                    )
                })?;
                if !(0..=30).contains(&days) {
                    bail!(
                        "view {:?} settles in {days} days, which is not a settlement \
                         convention any market uses. Nothing here rolls further than 30",
                        self.id
                    );
                }
                // ⚠ A CALENDAR IS REQUIRED, AND THE ABSENCE IS NOT "EVERY DAY IS
                // A BUSINESS DAY". A settlement date computed over a calendar
                // nobody wrote down is a date nobody agreed to, and it lands
                // trades in the wrong period on every weekend of the year.
                let named = self.calendar.as_deref().ok_or_else(|| {
                    anyhow!(
                        "view {:?} settles in {days} days and names no calendar. Which days \
                         is the market shut? Declare a `[[calendar]]` and point `calendar` \
                         at its id — rolling over an unstated calendar would settle trades \
                         on weekends",
                        self.id
                    )
                })?;
                if !calendars.iter().any(|c| c.id == named) {
                    bail!(
                        "view {:?} rolls over calendar {named:?}, which this configuration \
                         does not declare. Add a `[[calendar]]` with that id, or point the \
                         view at one that exists",
                        self.id
                    );
                }
            }
            Basis::Recorded | Basis::Trade => {
                if self.settles_in.is_some() {
                    bail!(
                        "view {:?} is a {} view and carries `settles_in`, which only a \
                         settlement view reads. Did you mean `basis = \"settlement\"`?",
                        self.id,
                        self.basis.as_declared()
                    );
                }
            }
        }
        Ok(())
    }
}

impl Calendar {
    /// Checked when the configuration is read, for the same reason.
    pub fn check(&self) -> Result<()> {
        if self.weekend.iter().any(|d| !(0..=6).contains(d)) {
            bail!(
                "calendar {:?} names a weekend day outside 0..=6. Sunday is 0",
                self.id
            );
        }
        // ⛔ SEVEN CLOSED DAYS IS A MARKET THAT NEVER OPENS, and a roll over it
        // would never terminate. `Ratio.Views.a_calendar_that_never_opens_
        // refuses_rather_than_looping` makes the engine refuse rather than hang;
        // this makes the configuration refuse rather than reach the engine.
        if self.weekend.len() >= 7 {
            bail!(
                "calendar {:?} closes the market every day of the week, so no trade under it \
                 would ever settle",
                self.id
            );
        }
        for d in &self.holidays {
            ratio_common::days_from_iso_date(d).with_context(|| {
                format!("calendar {:?} names a holiday that is not a date", self.id)
            })?;
        }
        Ok(())
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
             same two lots invert between two prices. Elect it with min_tax_short_weight, \
             not lot_method. See Ratio.Lots.MinTax and \
             Ratio.Lots.Methods.a_tax_minimising_method_is_not_a_function_of_the_lots",
        ),
        "specific_identification" | "specific_id" | "specid" => Some(
            "specific identification is not an ordering, so it is not a lot method: the client \
             names the lots PER SALE, possibly from the middle of a holding and possibly \
             partially. It is an instruction carried on the disposal, not a term of the rule \
             set. Carry the names on the sale as identified_lots; an empty list refuses \
             rather than walking FIFO. See Ratio.Lots.SpecId and \
             Ratio.Lots.Methods.specific_identification_takes_from_the_middle",
        ),
        "average_cost" | "average" | "pooled" => Some(
            "average cost is not an ordering, so it is not a lot method: it POOLS the holding, \
             so which lot is given up is not a question it answers. It also divides — total \
             cost over total units rarely lands on a whole minor unit — which is a rounding \
             term no ordering method carries. Elect it with average_cost = true, not \
             lot_method. See Ratio.Lots.AverageCost and \
             Ratio.Lots.Methods.average_cost_is_not_a_lot_walk",
        ),
        "wash" | "wash_sale" | "wash_sales" => Some(
            "a wash is not an ordering, so it is not a lot method: it DEFERS a loss onto a \
             replacement's basis, and the holding-period rule is which DATE that replacement \
             carries. Elect the window with wash_window_days; elect the non-US keep with \
             wash_keep_holding_period = true. See Ratio.Lots.Wash and \
             Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate",
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
        // ⛔ A NEGATIVE WINDOW IS NOT A WINDOW. `-30` would make `inWashWindow`
        // never fire, which is "we handle wash sales" in name only — every
        // conservation check would pass and no loss would ever be deferred.
        if let Some(w) = set.wash_window_days {
            if w < 0 {
                bail!(
                    "wash_window_days is {w}, and a negative window is not a window — \
                     every repurchase would sit outside it, and a loss the rule \
                     defers would be recognized in full"
                );
            }
        }
        // ⛔ MINTAX IS NOT AN ORDERING, AND A NON-POSITIVE WEIGHT IS NOT A
        // WEIGHT. Zero makes every short-term result free; negative inverts
        // the preference the election exists to express.
        if let Some(w) = set.min_tax_short_weight {
            if w <= 0 {
                bail!(
                    "min_tax_short_weight is {w}, and a non-positive weight is not a \
                     weight — short-term results would cost nothing or the opposite \
                     of what the election says"
                );
            }
            if set.lot_method.is_some() {
                bail!(
                    "this configuration elects both lot_method and min_tax_short_weight. \
                     Min-tax is not an ordering — it ranks at the SALE PRICE — and the \
                     two cannot govern the same sales. Drop lot_method, or drop \
                     min_tax_short_weight. See Ratio.Lots.MinTax"
                );
            }
        }
        // ⛔ AVERAGE COST IS NOT AN ORDERING, AND `false` IS NOT AN ELECTION.
        // Omitting the field is how a book is not pooled. Writing false would
        // make "somebody said no" look like a term, and a silent true would
        // start pooling every existing book.
        if let Some(flag) = set.average_cost {
            if !flag {
                bail!(
                    "average_cost = false is not an election — omit the field. None means \
                     nobody said. See Ratio.Lots.AverageCost"
                );
            }
            if set.lot_method.is_some() {
                bail!(
                    "this configuration elects both lot_method and average_cost. \
                     Average cost is not an ordering — it POOLS the holding — and the \
                     two cannot govern the same sales. Drop lot_method, or drop \
                     average_cost. See Ratio.Lots.AverageCost"
                );
            }
            if set.min_tax_short_weight.is_some() {
                bail!(
                    "this configuration elects both min_tax_short_weight and average_cost. \
                     One ranks at a SALE PRICE; the other POOLS. Two answers for one \
                     sale. Drop min_tax_short_weight, or drop average_cost. \
                     See Ratio.Lots.AverageCost"
                );
            }
        }
        // ⛔ KEEP IS NOT AN ORDERING, AND `false` IS NOT AN ELECTION.
        // Omitting the field leaves the US transfer that already landed.
        // Writing false would make "somebody said transfer" look like a
        // new term, and a silent true would restate every existing wash
        // book's later disposal at the other rate.
        if let Some(flag) = set.wash_keep_holding_period {
            if !flag {
                bail!(
                    "wash_keep_holding_period = false is not an election — omit the field. \
                     None means nobody said, and the US transfer already named in \
                     Ratio.Lots.Wash.replacementAcquired stays in force. \
                     See Ratio.Lots.WashHolding"
                );
            }
            if set.wash_window_days.is_none() {
                bail!(
                    "this configuration elects wash_keep_holding_period without \
                     wash_window_days. A holding-period variant of a wash nobody \
                     named cannot be applied. Write the window, or omit the keep. \
                     See Ratio.Lots.WashHolding"
                );
            }
        }
        // ⛔ AND THE VIEWS, FOR THE SAME REASON. A view naming a calendar
        // nobody declared is wrong when it is written, not on the NAV day it
        // first has to roll a trade over one.
        for c in &set.calendars {
            c.check()?;
        }
        if let Some(p) = &set.project {
            p.check()?;
        }
        if let Some(p) = &set.personal {
            p.check()?;
        }
        // ⛔ A CUT IS NAMED WEIGHTS, NOT A PARTNER COUNT. Checked when
        // the configuration is READ: a zero weight or a duplicate
        // partner is wrong the moment it is written down.
        partners::check_cut(&set.partner_cut)?;
        partners::check_specials(&set.special_allocations)?;
        let mut seen = std::collections::BTreeSet::new();
        for v in &set.views {
            v.check(&set.calendars)?;
            // ⛔ TWO VIEWS WITH ONE ID ARE TWO ANSWERS UNDER ONE NAME. A strike
            // recorded against the id could not be attributed to either, and the
            // console would render whichever the iterator reached first.
            if !seen.insert(v.id.clone()) {
                bail!(
                    "this configuration declares view {:?} twice. A figure recorded against \
                     that id could not be attributed to either one",
                    v.id
                );
            }
            if v.id == UNDECLARED_VIEW {
                bail!(
                    "{UNDECLARED_VIEW:?} is the id a book with NO declared views has, so a \
                     declared view cannot take it — the two would be indistinguishable, and \
                     an election nobody made would read as one somebody did"
                );
            }
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

    if let Some(c) = &set.close {
        match by_dim.get(&c.equity_destination) {
            None => out.push(Finding::error(
                "close",
                format!(
                    "names equity destination {} which is not in the chart of accounts. \
                     Add it, or point [close] at an existing equity account. \
                     Ratio.Close.missing_destination_refuses_the_close",
                    c.equity_destination
                ),
            )),
            Some(account) if account.account_type != AccountTypeRecord::Equity => {
                out.push(Finding::error(
                    "close",
                    format!(
                        "names equity destination {} ({}) which is {:?}, not equity. \
                         A close that rolled surplus into an income or asset account \
                         would move the residual off the sheet while the books still tied",
                        c.equity_destination, account.display_name, account.account_type
                    ),
                ));
            }
            Some(_) => {}
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

/// Merge legs that share (dimension, currency, instrument).
///
/// ⭐ TWO BALANCED TEMPLATES REMAIN BALANCED. A loan payment compiles
/// interest and principal as separate rules (each nets to zero by
/// `balanced_template_balances`) and concatenates them. The cash account
/// then has two credits; merging those legs is the 3-posting identity
/// (interest expense + principal reduction against cash) and does not
/// change the net. A zero after the merge is dropped — a $0 interest
/// posting is nothing that happened.
pub fn merge_postings(
    legs: Vec<ratio_store::PostingRecord>,
) -> Result<Vec<ratio_store::PostingRecord>> {
    let mut by: BTreeMap<(i64, Option<String>, Option<String>), (i64, Option<i64>)> =
        BTreeMap::new();
    for p in legs {
        let key = (p.dim, p.currency.clone(), p.instrument.clone());
        let slot = by.entry(key).or_insert((0, None));
        slot.0 = ratio_common::checked::add(slot.0, p.amount, "a merged posting")?;
        match (slot.1, p.quantity) {
            (None, q) => slot.1 = q,
            (Some(a), Some(b)) => {
                slot.1 = Some(ratio_common::checked::add(a, b, "a merged quantity")?);
            }
            (Some(_), None) => {}
        }
    }
    Ok(by
        .into_iter()
        .filter(|(_, (amount, _))| *amount != 0)
        .map(|((dim, currency, instrument), (amount, quantity))| PostingRecord {
            dim,
            amount,
            currency,
            instrument,
            quantity,
        })
        .collect())
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
            ("wash", "DATE"),
            ("wash_sale", "DATE"),
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

    const TWO_VIEWS: &str = r#"
rules = []
[[calendar]]
id = "us-settlement"
holidays = ["2026-01-01", "2026-12-25"]
[[view]]
id = "abor"
display_name = "ABOR"
basis = "trade"
[[view]]
id = "ibor"
display_name = "IBOR"
basis = "settlement"
settles_in = 2
calendar = "us-settlement"
"#;

    #[test]
    fn a_silent_configuration_declares_no_views_but_still_has_one() {
        // ⛔ THE DISTINCTION A DEFAULT DESTROYS, AND IT IS THE `lot_method` ONE
        // EXACTLY. A book that says nothing about views is folded in journal
        // order by CUSTOM; one that declares a trade-date view is folded that
        // way by ELECTION. The engine does something in both cases, and only
        // one of them is a term somebody agreed to.
        //
        // ⚠ AND `recorded` IS NOT `settlement 0`. The undeclared view consults
        // no date at all, which is the only thing that answers over the entries
        // carrying no trade date — most of every book written so far. A
        // same-day settlement convention would refuse every one of them.
        let silent = RuleSet::from_toml("rules = []\n").unwrap();
        assert!(silent.views.is_empty(), "nobody said");
        assert!(!silent.views_declared());
        let one = silent.effective_views();
        assert_eq!(one.len(), 1, "and it still folds");
        assert_eq!(one[0].basis, Basis::Recorded);
        assert_eq!(one[0].settles_in, None, "recorded consults no calendar");
        assert_eq!(silent.default_view(), UNDECLARED_VIEW);

        let declared = RuleSet::from_toml(TWO_VIEWS).unwrap();
        assert!(declared.views_declared(), "somebody said");
        assert_eq!(declared.effective_views().len(), 2);
        assert_eq!(declared.default_view(), "abor");

        // ⛔ BOTH FOLD, AND THEY ARE DIFFERENT TO A READER — which is the whole
        // point. An assertion on the effective views alone passes whether or
        // not the distinction survives.
        assert!(!silent.effective_views().is_empty());
        assert!(!declared.effective_views().is_empty());
        assert_ne!(silent.views_declared(), declared.views_declared());
    }

    #[test]
    fn a_declared_view_cannot_take_the_undeclared_id() {
        // Otherwise a screen could not tell an election from its absence, which
        // is the one thing `views_declared` exists to report.
        let e = RuleSet::from_toml(&format!(
            "rules = []\n[[view]]\nid = \"{UNDECLARED_VIEW}\"\nbasis = \"trade\"\n"
        ))
        .expect_err("the undeclared id is reserved")
        .to_string();
        assert!(e.contains("an election nobody made"), "{e}");
    }

    #[test]
    fn every_basis_round_trips_through_its_declared_name() {
        // ⛔ TWO SPELLINGS OF EVERY VARIANT — serde's and `as_declared`'s — and a
        // disagreement is silent in the worst way: `Basis` derives `Default`, so
        // a view declaring a basis the reader does not recognize would fold in
        // journal order while its agreement says settlement, and only the timing
        // of every figure would move.
        for b in Basis::ALL {
            let toml = format!(
                "rules = []\n[[view]]\nid = \"v\"\nbasis = \"{}\"\n{}",
                b.as_declared(),
                if b == Basis::Settlement {
                    "settles_in = 2\ncalendar = \"c\"\n[[calendar]]\nid = \"c\"\n"
                } else {
                    ""
                }
            );
            let back = RuleSet::from_toml(&toml)
                .unwrap_or_else(|e| panic!("{b:?} declares itself as {:?}: {e}", b.as_declared()))
                .views[0]
                .basis;
            assert_eq!(back, b, "{b:?} does not survive its own declared name");
        }
        assert_eq!(
            Basis::ALL.len(),
            Basis::ALL.iter().map(|b| b.as_declared()).collect::<std::collections::BTreeSet<_>>().len(),
            "ALL lists a basis twice, or two share a declared name"
        );
    }

    #[test]
    fn a_settlement_view_that_cannot_roll_is_refused_when_the_config_is_read() {
        // ⛔ AT READ TIME, LIKE `ChartRoles::check`. Each of these is wrong the
        // moment it is written down; finding out at the first settlement means
        // finding out on a NAV day.
        for (toml, must_say) in [
            (
                "rules = []\n[[view]]\nid = \"ibor\"\nbasis = \"settlement\"\n",
                "in how many days",
            ),
            (
                "rules = []\n[[view]]\nid = \"ibor\"\nbasis = \"settlement\"\nsettles_in = 2\n",
                "names no calendar",
            ),
            (
                "rules = []\n[[view]]\nid = \"ibor\"\nbasis = \"settlement\"\nsettles_in = 2\ncalendar = \"nope\"\n",
                "does not declare",
            ),
            (
                "rules = []\n[[view]]\nid = \"abor\"\nbasis = \"trade\"\nsettles_in = 2\n",
                "only a settlement view reads",
            ),
            (
                "rules = []\n[[view]]\nid = \"A BOR\"\nbasis = \"trade\"\n",
                "URL segment",
            ),
        ] {
            let e = RuleSet::from_toml(toml).expect_err(must_say).to_string();
            assert!(e.contains(must_say), "expected {must_say:?}, got: {e}");
        }
    }

    #[test]
    fn a_calendar_that_never_opens_is_refused_rather_than_looping() {
        // The configuration half of `Ratio.Views.a_calendar_that_never_opens_
        // refuses_rather_than_looping`: the engine refuses instead of hanging,
        // and this stops such a calendar reaching the engine at all.
        let e = RuleSet::from_toml(
            "rules = []\n[[calendar]]\nid = \"never\"\nweekend = [0, 1, 2, 3, 4, 5, 6]\n",
        )
        .expect_err("a market that never opens")
        .to_string();
        assert!(e.contains("would ever settle"), "{e}");

        let e = RuleSet::from_toml(
            "rules = []\n[[calendar]]\nid = \"c\"\nholidays = [\"the fifth of March\"]\n",
        )
        .expect_err("not a date")
        .to_string();
        assert!(e.contains("not a date"), "{e}");
    }

    #[test]
    fn two_views_with_one_id_are_refused() {
        let e = RuleSet::from_toml(
            "rules = []\n[[view]]\nid = \"abor\"\nbasis = \"trade\"\n\
             [[view]]\nid = \"abor\"\nbasis = \"recorded\"\n",
        )
        .expect_err("one id, two answers")
        .to_string();
        assert!(e.contains("twice"), "{e}");
    }

    #[test]
    fn a_view_survives_a_round_trip_through_toml() {
        let set = RuleSet::from_toml(TWO_VIEWS).unwrap();
        let back = RuleSet::from_toml(&set.to_toml().unwrap()).unwrap();
        assert_eq!(back, set);
        assert_eq!(back.views[1].settles_in, Some(2));
        assert_eq!(back.calendar("us-settlement").unwrap().holidays.len(), 2);
        // The default weekend is a real answer and survives being unstated.
        assert_eq!(back.calendar("us-settlement").unwrap().weekend, vec![0, 6]);
    }

    #[test]
    fn a_configuration_with_no_views_serializes_without_the_section() {
        // ⛔ SO NO EXISTING BOOK'S DIGEST MOVES. Every configuration in every
        // book predates this field; emitting an empty `view = []` would change
        // the canonical bytes, change the content address, and orphan every
        // journal entry pinning the old one.
        let toml = RuleSet::from_toml("rules = []\n").unwrap().to_toml().unwrap();
        assert!(!toml.contains("view"), "{toml}");
        assert!(!toml.contains("calendar"), "{toml}");
        assert!(!toml.contains("project"), "{toml}");
        assert!(!toml.contains("personal"), "{toml}");
    }

    #[test]
    fn a_project_budget_round_trips_and_a_negative_one_is_refused() {
        let set = RuleSet::from_toml("rules = []\n[project]\nbudget = 1000000\n").unwrap();
        assert_eq!(set.project.as_ref().and_then(|p| p.budget), Some(1_000_000));
        let back = RuleSet::from_toml(&set.to_toml().unwrap()).unwrap();
        assert_eq!(back.project, set.project);

        let e = RuleSet::from_toml("rules = []\n[project]\nbudget = -1\n")
            .expect_err("a negative budget must not parse")
            .to_string();
        assert!(e.contains("not negative"), "{e}");
    }

    #[test]
    fn a_phase_budget_round_trips_and_a_negative_or_duplicate_is_refused() {
        let set = RuleSet::from_toml(
            "rules = []\n[[project.phase]]\naccount = 11\nbudget = 400000\n",
        )
        .unwrap();
        assert_eq!(set.project.as_ref().unwrap().phases[0].account, 11);
        assert_eq!(set.project.as_ref().unwrap().phases[0].budget, Some(400_000));
        let back = RuleSet::from_toml(&set.to_toml().unwrap()).unwrap();
        assert_eq!(back.project, set.project);

        let e = RuleSet::from_toml("rules = []\n[[project.phase]]\naccount = 11\nbudget = -1\n")
            .expect_err("a negative phase budget must not parse")
            .to_string();
        assert!(e.contains("not negative"), "{e}");

        let e = RuleSet::from_toml(
            "rules = []\n[[project.phase]]\naccount = 11\nbudget = 1\n\
             [[project.phase]]\naccount = 11\nbudget = 2\n",
        )
        .expect_err("two budgets for one account must not parse")
        .to_string();
        assert!(e.contains("twice"), "{e}");
    }

    #[test]
    fn a_household_budget_round_trips_and_a_negative_one_is_refused() {
        let set = RuleSet::from_toml(
            "rules = []\n[personal]\nbudget = 500000\n[personal.envelope]\n10 = 400000\n11 = 100000\n",
        )
        .unwrap();
        let p = set.personal.as_ref().expect("personal table present");
        assert_eq!(p.budget, Some(500_000));
        assert_eq!(p.envelope.get("10"), Some(&400_000));
        assert_eq!(p.envelope.get("11"), Some(&100_000));
        assert!(
            p.envelope.get("2").is_none(),
            "an envelope nobody set is absent, not a fake zero"
        );

        let silent = RuleSet::from_toml("rules = []\n").unwrap();
        assert!(silent.personal.is_none(), "nobody said");

        let zero = RuleSet::from_toml("rules = []\n[personal]\nbudget = 0\n").unwrap();
        assert_eq!(
            zero.personal.as_ref().and_then(|p| p.budget),
            Some(0),
            "a set baseline of nothing is not the same as unset"
        );

        let e = RuleSet::from_toml("rules = []\n[personal]\nbudget = -1\n")
            .expect_err("a negative budget must not parse")
            .to_string();
        assert!(e.contains("not negative"), "{e}");

        let env = RuleSet::from_toml("rules = []\n[personal.envelope]\n10 = -5\n")
            .expect_err("a negative envelope must not parse")
            .to_string();
        assert!(env.contains("not negative"), "{env}");

        let bad_key = RuleSet::from_toml("rules = []\n[personal.envelope]\nliving = 1\n")
            .expect_err("an envelope key that is not a dimension must not parse")
            .to_string();
        assert!(bad_key.contains("chart dimension"), "{bad_key}");
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
    fn a_wash_window_nobody_declared_is_absent_rather_than_thirty() {
        // ⛔ NOT THE `long_term_days` SHAPE. Silence-as-30 restated every
        // in-window loss on every existing book. Thirty is the US number a
        // fund writes; it is not applied to a book that never elected the rule.
        assert_eq!(RuleSet::default().wash_window_days, None);
        assert_eq!(RuleSet::from_toml("rules = []\n").unwrap().wash_window_days, None);

        let set = RuleSet::from_toml("rules = []\nwash_window_days = 10\n").unwrap();
        assert_eq!(set.wash_window_days, Some(10));

        let toml = RuleSet::from_toml("rules = []\n").unwrap().to_toml().unwrap();
        assert!(
            !toml.contains("wash_window"),
            "silence must not write a window: {toml}"
        );
        let named = RuleSet::from_toml("rules = []\nwash_window_days = 10\n")
            .unwrap()
            .to_toml()
            .unwrap();
        assert!(named.contains("wash_window_days = 10"), "{named}");

        let e = RuleSet::from_toml("rules = []\nwash_window_days = -1\n")
            .expect_err("a negative window is not a window")
            .to_string();
        assert!(e.contains("not a window"), "{e}");
    }

    #[test]
    fn a_min_tax_weight_nobody_declared_is_absent_rather_than_two() {
        // ⛔ NOT A SILENT 2. Two is the Lean example's short-term weight; it
        // is not applied to a book that never elected the ranking. Silence
        // that became a default would start taking different lots on every
        // existing sale.
        assert_eq!(RuleSet::default().min_tax_short_weight, None);
        assert_eq!(RuleSet::from_toml("rules = []\n").unwrap().min_tax_short_weight, None);

        let set = RuleSet::from_toml("rules = []\nmin_tax_short_weight = 2\n").unwrap();
        assert_eq!(set.min_tax_short_weight, Some(2));
        assert_eq!(set.lot_method, None, "min-tax is not a lot_method");

        let toml = RuleSet::from_toml("rules = []\n").unwrap().to_toml().unwrap();
        assert!(
            !toml.contains("min_tax"),
            "silence must not write a weight: {toml}"
        );
        let named = RuleSet::from_toml("rules = []\nmin_tax_short_weight = 2\n")
            .unwrap()
            .to_toml()
            .unwrap();
        assert!(named.contains("min_tax_short_weight = 2"), "{named}");

        let zero = RuleSet::from_toml("rules = []\nmin_tax_short_weight = 0\n")
            .expect_err("zero is not a weight")
            .to_string();
        assert!(zero.contains("not a weight"), "{zero}");

        let both = RuleSet::from_toml(
            "rules = []\nlot_method = \"hifo\"\nmin_tax_short_weight = 2\n",
        )
        .expect_err("two elections for one sale")
        .to_string();
        assert!(both.contains("SALE PRICE"), "{both}");
        assert!(both.contains("lot_method"), "{both}");
    }

    #[test]
    fn an_average_cost_election_nobody_declared_is_absent_rather_than_true() {
        // ⛔ NOT A SILENT TRUE. Pooling a book that never elected it would
        // restate every sale. None means nobody said.
        assert_eq!(RuleSet::default().average_cost, None);
        assert_eq!(RuleSet::from_toml("rules = []\n").unwrap().average_cost, None);

        let set = RuleSet::from_toml("rules = []\naverage_cost = true\n").unwrap();
        assert_eq!(set.average_cost, Some(true));
        assert_eq!(set.lot_method, None, "average cost is not a lot_method");

        let toml = RuleSet::from_toml("rules = []\n").unwrap().to_toml().unwrap();
        assert!(
            !toml.contains("average_cost"),
            "silence must not write a pool: {toml}"
        );
        let named = RuleSet::from_toml("rules = []\naverage_cost = true\n")
            .unwrap()
            .to_toml()
            .unwrap();
        assert!(named.contains("average_cost = true"), "{named}");

        let denied = RuleSet::from_toml("rules = []\naverage_cost = false\n")
            .expect_err("false is not an election")
            .to_string();
        assert!(denied.contains("not an election"), "{denied}");

        let both = RuleSet::from_toml(
            "rules = []\nlot_method = \"hifo\"\naverage_cost = true\n",
        )
        .expect_err("two elections for one sale")
        .to_string();
        assert!(both.contains("POOLS"), "{both}");
        assert!(both.contains("lot_method"), "{both}");

        let with_mintax = RuleSet::from_toml(
            "rules = []\nmin_tax_short_weight = 2\naverage_cost = true\n",
        )
        .expect_err("pool and ranking are two answers")
        .to_string();
        assert!(with_mintax.contains("POOLS") || with_mintax.contains("SALE PRICE"), "{with_mintax}");
    }

    #[test]
    fn a_keep_election_nobody_declared_is_absent_rather_than_true() {
        // ⛔ NONE IS UNSET, NOT A SILENT KEEP. The US transfer already
        // landed via replacementAcquired. A silent true would restate
        // every existing wash book's later disposal at the other rate.
        // `Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate`.
        assert_eq!(RuleSet::default().wash_keep_holding_period, None);
        assert_eq!(
            RuleSet::from_toml("rules = []\n").unwrap().wash_keep_holding_period,
            None
        );

        let set = RuleSet::from_toml(
            "rules = []\nwash_window_days = 30\nwash_keep_holding_period = true\n",
        )
        .unwrap();
        assert_eq!(set.wash_keep_holding_period, Some(true));
        assert_eq!(set.lot_method, None, "keep is not a lot_method");

        let toml = RuleSet::from_toml("rules = []\n").unwrap().to_toml().unwrap();
        assert!(
            !toml.contains("wash_keep_holding_period"),
            "silence must not write a keep: {toml}"
        );
        let named = RuleSet::from_toml(
            "rules = []\nwash_window_days = 30\nwash_keep_holding_period = true\n",
        )
        .unwrap()
        .to_toml()
        .unwrap();
        assert!(
            named.contains("wash_keep_holding_period = true"),
            "{named}"
        );

        let denied = RuleSet::from_toml("rules = []\nwash_keep_holding_period = false\n")
            .expect_err("false is not an election")
            .to_string();
        assert!(denied.contains("not an election"), "{denied}");

        let orphan = RuleSet::from_toml("rules = []\nwash_keep_holding_period = true\n")
            .expect_err("keep without a window cannot be applied")
            .to_string();
        assert!(orphan.contains("wash_window_days"), "{orphan}");
    }

    #[test]
    fn a_partner_cut_nobody_declared_is_absent_rather_than_one_over_n() {
        // ⛔ NOT A SILENT 1/N. Empty is unset. Allocated plugs stay
        // unset. Writing equal weights is an election; inventing them
        // from a partner count is not. `Ratio.Partners.no_cut_is_unset`.
        assert!(RuleSet::default().partner_cut.is_empty());
        assert!(RuleSet::from_toml("rules = []\n").unwrap().partner_cut.is_empty());

        let set = RuleSet::from_toml(
            "rules = []\n\n[[partner_cut]]\npartner = \"LP\"\nweight = 80\n\n\
             [[partner_cut]]\npartner = \"GP\"\nweight = 20\n",
        )
        .unwrap();
        assert_eq!(
            set.partner_cut,
            vec![
                crate::PartnerShare { partner: "LP".into(), weight: 80 },
                crate::PartnerShare { partner: "GP".into(), weight: 20 },
            ]
        );

        let toml = RuleSet::from_toml("rules = []\n").unwrap().to_toml().unwrap();
        assert!(
            !toml.contains("partner_cut"),
            "silence must not write a cut: {toml}"
        );
        let named = set.to_toml().unwrap();
        assert!(named.contains("partner_cut"), "{named}");
        assert!(named.contains("LP"), "{named}");

        let zero = RuleSet::from_toml(
            "rules = []\n\n[[partner_cut]]\npartner = \"LP\"\nweight = 0\n",
        )
        .expect_err("zero is not a weight")
        .to_string();
        assert!(zero.contains("not a weight"), "{zero}");

        let dup = RuleSet::from_toml(
            "rules = []\n\n[[partner_cut]]\npartner = \"LP\"\nweight = 50\n\n\
             [[partner_cut]]\npartner = \"LP\"\nweight = 50\n",
        )
        .expect_err("two rows for one partner")
        .to_string();
        assert!(dup.contains("twice"), "{dup}");
    }

    #[test]
    fn a_standing_special_replaces_the_default_for_that_kind() {
        let set = RuleSet::from_toml(
            "rules = []\n\n[[partner_cut]]\npartner = \"LP\"\nweight = 80\n\n\
             [[partner_cut]]\npartner = \"GP\"\nweight = 20\n\n\
             [[special_allocation]]\npartner = \"GP\"\nkind = \"expense\"\nweight = 1\n",
        )
        .unwrap();
        assert_eq!(set.special_allocations.len(), 1);
        assert_eq!(set.special_allocations[0].kind, crate::AllocationKind::Expense);
        let expense = crate::cut_for(
            crate::AllocationKind::Expense,
            &set.partner_cut,
            &set.special_allocations,
        );
        assert_eq!(expense.len(), 1);
        assert_eq!(expense[0].partner, "GP");
        let denied = RuleSet::from_toml(
            "rules = []\n\n[[special_allocation]]\npartner = \"GP\"\nkind = \"fee\"\nweight = 1\n",
        )
        .expect_err("fee is not a kind")
        .to_string();
        assert!(denied.contains("unknown") || denied.contains("fee") || denied.contains("TOML"), "{denied}");
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

    #[test]
    fn a_household_loan_schedule_round_trips_and_an_unset_one_is_absent() {
        let set = RuleSet::from_toml(
            "rules = []\n[personal.loan]\n41 = 12\n42 = 13\n",
        )
        .unwrap();
        let p = set.personal.as_ref().expect("personal table present");
        assert_eq!(p.loan.get("41"), Some(&12));
        assert_eq!(p.loan.get("42"), Some(&13));
        assert!(
            p.loan.get("43").is_none(),
            "a loan nobody set is absent, not a fake zero"
        );

        let silent = RuleSet::from_toml("rules = []\n").unwrap();
        assert!(silent.personal.is_none(), "nobody said");

        let e = RuleSet::from_toml("rules = []\n[personal.loan]\nmortgage = 12\n")
            .expect_err("a name that is not a dimension must not parse")
            .to_string();
        assert!(e.contains("chart dimension"), "{e}");

        let same = RuleSet::from_toml("rules = []\n[personal.loan]\n41 = 41\n")
            .expect_err("liability cannot be its own interest")
            .to_string();
        assert!(same.contains("itself"), "{same}");
    }
}
