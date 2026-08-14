//! What a NAV strike does, written down as a plan somebody can read.
//!
//! # ⛔ THIS IS A DESCRIPTION, NOT A PLANNER, AND THE DIFFERENCE IS THE WHOLE
//! HONESTY OF THE SCREEN IT FEEDS
//!
//! Nothing in this repository chooses a plan. `ratio_nav::strike` folds the
//! journal; `Projection::nav` reads maintained totals; a caller picks one by
//! calling it. There is no plan node type, no rewrite registry and no cost
//! comparison at runtime — `Ratio.Plan` proves two plans agree and is not
//! emitted into Rust at all.
//!
//! So this module does not render a plan the engine built. It writes down what
//! the two paths already do, and attaches to each step the cost that the model
//! proves or the fold measures. Every node here names the function it
//! describes, because a picture of a structure nothing produces is worse than
//! no picture: it would be checked by nothing and believed anyway.
//!
//! # The two groups, and why both are always present
//!
//! [`Group::Recorded`] is `ratio_nav::strike` — the fold that produced the
//! record. Its cost grows with the journal.
//!
//! [`Group::Maintained`] is `Projection::nav` — the same question answered off
//! totals somebody keeps. Its cost is the chart and the currencies, and does
//! not move when fragmentation does.
//!
//! ⛔ **BOTH, ALWAYS, AND THAT IS NOT A LAYOUT PREFERENCE.** `ratio bench`
//! "reports two curves and both must be quoted", and this is the same pair: the
//! flat one is the persuasive number and the growing one is what somebody
//! actually waits for. A plan screen showing only the maintained group would be
//! the overclaim `ratio bench` is shaped to make hard, drawn as a diagram.
//!
//! # ⛔ WHAT THE PROVED MODEL DOES NOT COST
//!
//! `Ratio.Closure` costs a PERIOD END — marking, fx, actions, capital — off a
//! projection that is already current. It has no term for parsing a journal,
//! and there is no theorem that says what a fold costs. So the recorded group's
//! steps carry an estimate only where the strike record itself supplies one
//! (the pinned prefix length) or where the shape does (accounts, currencies).
//! Everywhere else they are measured or they are blank.
//!
//! ⚠ Blank, never zero. [`Node::estimated_reads`] and [`Node::actual_rows`] are
//! `Option`, and the wire carries a flag beside every figure for the same
//! reason: `fx_rate` returned a number two orders of magnitude wrong for months
//! because nothing read it, and a rendered `0` that nothing measured is the
//! same defect facing the other way.

use crate::closure::{human_nanos, Calibration, Estimate};
use crate::Strike;

/// Which of the two answers to "what is this fund worth" a step belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// `ratio_nav::strike` — the fold that produced the record. O(journal).
    Recorded,
    /// `Projection::nav` — the maintained totals. O(chart × currencies).
    Maintained,
}

impl Group {
    pub fn as_str(self) -> &'static str {
        match self {
            Group::Recorded => "RECORDED",
            Group::Maintained => "MAINTAINED",
        }
    }
}

/// What a step is doing on the page.
///
/// ⛔ `Unread` IS NOT `Rejected`. A rejected step is a plan that would have
/// worked and cost more. An unread one is a thing the chosen plan never touches
/// at all — `Ratio.Closure.factored_nav_never_reads_the_lots` — and collapsing
/// the two would lose the only claim on this screen that is a theorem.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A step that runs.
    Chosen,
    /// A plan for the same answer that costs more and is not taken.
    Rejected,
    /// Where the strike stops rather than reporting a figure it cannot stand
    /// behind.
    Refusal,
    /// Read by nothing, and drawn so that is visible.
    Unread,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Chosen => "CHOSEN",
            Role::Rejected => "REJECTED",
            Role::Refusal => "REFUSAL",
            Role::Unread => "UNREAD",
        }
    }
}

/// What an edge means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    /// Rows moving from one step to the next.
    Flow,
    /// Where those rows go instead when the strike refuses them.
    Refusal,
    /// Drawn, and carrying nothing. The unread lots hang off the plan on one of
    /// these.
    Unread,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Flow => "FLOW",
            EdgeKind::Refusal => "REFUSAL",
            EdgeKind::Unread => "UNREAD",
        }
    }
}

/// One step of a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    /// Stable within a plan; edges name these.
    pub id: String,
    /// What the step does, in the operator style `EXPLAIN` uses.
    pub operator: String,
    /// The one line that qualifies it — the equivalent of an index name.
    pub detail: String,
    pub group: Group,
    pub role: Role,
    /// ⛔ THE THEOREM OR THE FUNCTION THIS COST COMES FROM. A number on this
    /// screen with no citation would be exactly what the product argues
    /// against: a figure a reader has to trust rather than check.
    pub cites: String,
    /// Why it costs what it costs. Rendered, not decoration.
    pub note: Vec<String>,
    /// Reads, from the proved model or from the record. `None` where nothing
    /// costs this step — never `Some(0)` standing in for "unknown".
    pub estimated_reads: Option<i64>,
    /// `estimated_reads × nanos_per_read`. The measured half of the estimate.
    pub estimated_nanos: Option<i64>,
    /// What the instrumented fold actually saw. `None` unless analyzed.
    pub actual_rows: Option<i64>,
    pub actual_nanos: Option<i64>,
}

/// An edge of the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edge {
    /// ⚠ `source`/`target`, NOT `from`/`to`, ALL THE WAY DOWN. The contract
    /// cannot spell it `from` — `from` is a reserved word in AIP-140's list
    /// because it is a Python keyword — and carrying one vocabulary here and
    /// another on the wire is how a mapping layer acquires a bug nobody reads.
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
    /// Rows that travelled it, when something measured them.
    pub rows: Option<i64>,
}

/// The fund's shape, as the estimate was taken over it.
///
/// ⛔ MEASURED OFF THE BOOK, NEVER TYPED IN. `/scale` offers dials because it
/// is arguing about funds that do not exist; this is explaining one that does,
/// and a dial here would let a reader produce any number they liked and read it
/// as this fund's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The bounds-checked cost model over this fund's dials.
    pub estimate: Estimate,
    /// Rows in the chart — one account type per dimension.
    pub accounts: i64,
    /// Rows the maintained plan actually walks: one per (dimension, currency).
    ///
    /// ⛔ NOT THE SAME AS `Estimate::marks`. The model charges one read per
    /// SECURITY; `Projection::translate` walks `Totals::by_dim`, which is keyed
    /// by dimension AND currency. Reporting one as the other is how an estimate
    /// stops being checkable against the thing it estimates.
    pub total_rows: i64,
    /// Open tax lots actually held.
    ///
    /// ⚠ NOT `Estimate::open_lots`, which is `securities × lots_per` and rounds
    /// down — `lots_per` is an average this crate derives by dividing. The
    /// sidecar quotes this one, because it is the number the claim is about.
    pub open_lots_held: i64,
}

/// What an instrumented fold saw.
///
/// ⛔ COUNTED AT STAGE BOUNDARIES, TIMED PER PHASE. `FoldCost`'s comment in
/// `ratio-project` is the precedent and the reason: seven million
/// `Instant::now()` calls would be a measurement that changed what it measured.
/// Every field here is either a counter incremented in a branch that was
/// already taken, or one `Instant` around a whole phase.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Measured {
    pub accounts_read: i64,
    pub rates_read: i64,
    pub setup_nanos: i64,

    pub entries_walked: i64,
    pub walk_nanos: i64,

    /// Every lookup the fold made for a view definition.
    pub config_lookups: i64,
    /// Of those, the ones that missed and had to read and parse the TOML.
    ///
    /// ⛔ THE CACHE IS THE OPTIMIZATION, AND THIS IS WHAT SAYS SO. A book holds
    /// a handful of configurations and millions of entries naming them; without
    /// the miss count the node reads as "one read per entry", which is what the
    /// code would do if the cache were removed and nothing would notice.
    pub config_misses: i64,

    pub placed: i64,
    pub unplaceable: i64,
    pub recognised: i64,

    pub digest_fed: i64,

    pub currencies_accumulated: i64,
    pub finish_nanos: i64,
}

/// A NAV strike's plan, both ways round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    /// The strike this explains, as a resource name.
    pub name: String,
    pub view: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub shape: Option<Shape>,
    /// Why there is no estimate, when there is none. Empty otherwise.
    ///
    /// ⛔ CARRIED, NOT SWALLOWED. The maintained projection folds with no cut,
    /// so it cannot supply a shape for a trade- or settlement-basis view — and
    /// answering with the recorded view's shape under another name is the exact
    /// defect the view feature exists to prevent.
    pub estimate_refusal: String,
    pub analyzed: bool,
    /// The rate the durations were derived with, and where it came from.
    pub nanos_per_read: i64,
    pub provenance: String,

    /// What the maintained plan reads: `Ratio.Closure.factorNavCost`.
    pub chosen_reads: Option<i64>,
    /// The same period end with its open actions applied by rewriting the lots
    /// they touch: `Ratio.Closure.navCost`.
    pub rewrite_reads: Option<i64>,
    /// Folding every open tax lot instead of reading the maintained totals:
    /// `Ratio.Plan.scanCost`.
    pub scan_reads: Option<i64>,
}

impl Plan {
    /// Reads attributed to steps of one group, ignoring the ones nothing costs.
    ///
    /// ⚠ NOT A TOTAL OF THE PLAN. A group holds rejected and unread steps too,
    /// and adding those to the chosen ones would report a cost nothing pays.
    pub fn chosen_reads_in(&self, group: Group) -> i64 {
        self.nodes
            .iter()
            .filter(|n| n.group == group && n.role == Role::Chosen)
            .filter_map(|n| n.estimated_reads)
            .sum()
    }
}

/// One helper so a node's two derived durations cannot be derived two ways.
fn nanos(reads: Option<i64>, rate: i64) -> Option<i64> {
    reads.map(|r| r.saturating_mul(rate))
}

struct Build<'a> {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    rate: i64,
    m: Option<&'a Measured>,
}

impl<'a> Build<'a> {
    #[allow(clippy::too_many_arguments)]
    fn node(
        &mut self,
        id: &str,
        operator: &str,
        detail: String,
        group: Group,
        role: Role,
        cites: &str,
        note: Vec<String>,
        estimated_reads: Option<i64>,
        actual_rows: Option<i64>,
        actual_nanos: Option<i64>,
    ) {
        self.nodes.push(Node {
            id: id.to_string(),
            operator: operator.to_string(),
            detail,
            group,
            role,
            cites: cites.to_string(),
            note,
            estimated_reads,
            estimated_nanos: nanos(estimated_reads, self.rate),
            actual_rows,
            actual_nanos,
        });
    }

    fn edge(&mut self, source: &str, target: &str, kind: EdgeKind, rows: Option<i64>) {
        self.edges.push(Edge {
            source: source.to_string(),
            target: target.to_string(),
            kind,
            rows,
        });
    }

    /// A measurement, or nothing. Never a zero standing in for "not measured".
    fn measured<T>(&self, f: impl Fn(&Measured) -> T) -> Option<T> {
        self.m.map(f)
    }
}

/// Write down what this strike did, and what the same question costs off the
/// maintained totals.
///
/// `shape` is `None` when the projection cannot answer for this view;
/// `refusal` then carries the server's own words. `measured` is `None` unless
/// the caller asked for the fold to be re-run under instrumentation.
pub fn plan_of(
    name: &str,
    strike: &Strike,
    shape: Option<&Shape>,
    refusal: &str,
    measured: Option<&Measured>,
    cal: &Calibration,
) -> Plan {
    let mut b = Build {
        nodes: Vec::new(),
        edges: Vec::new(),
        rate: cal.nanos_per_read,
        m: measured,
    };
    let est = shape.map(|s| &s.estimate);
    let prefix = strike.journal_position as i64;

    // ── The recorded fold ────────────────────────────────────────────────
    //
    // ⛔ EVERY ONE OF THESE NAMES A REAL STEP OF `ratio_nav::strike`. A node
    // that described something the function does not do would be a diagram
    // nothing holds to the code, which is the failure mode HANDOFF.md records
    // for every comment that drifted.

    b.node(
        "open",
        "Open Book",
        "one open, then one walk".into(),
        Group::Recorded,
        Role::Chosen,
        "ratio_nav::strike",
        vec!["The strike used to read the journal into a `Vec`, hash a second \
              `Vec` of every serialized entry, and fold the first — three copies \
              of a thing that only ever needed to be walked once."
            .into()],
        Some(1),
        b.measured(|_| 1),
        None,
    );
    b.node(
        "chart",
        "Read Chart",
        match shape {
            Some(s) => format!("{}, dimension → account type", plural(s.accounts, "account", "accounts")),
            None => "dimension → account type".into(),
        },
        Group::Recorded,
        Role::Chosen,
        "FileBook::accounts",
        vec!["Which dimensions are assets and liabilities. The NAV filter is a \
              question about the chart, and the projection deliberately does not \
              know the chart."
            .into()],
        shape.map(|s| s.accounts),
        b.measured(|m| m.accounts_read),
        None,
    );
    b.node(
        "rates",
        "Read Rates",
        match est {
            Some(e) => format!("one factor per currency — {} of them", grouped(e.fx)),
            None => "one factor per currency".into(),
        },
        Group::Recorded,
        Role::Chosen,
        "Ratio.Closure.fx_does_not_grow_with_the_chart",
        vec![
            "⛔ Per CURRENCY, not per position. A fund with five hundred names \
             in three currencies does three translations, because translation \
             applies to per-currency subtotals."
                .into(),
            "⚠ THE MEASURED COUNT IS ONE LOWER THAN THE ESTIMATE, ON EVERY BOOK, \
             AND THAT IS THE MODEL BEING RIGHT. The base currency has no rate \
             fact — a fund does not record what a dollar is worth in dollars — \
             so three currencies are two rows. The estimate counts the \
             denominations; the measurement counts the facts read."
                .into(),
        ],
        est.map(|e| e.fx),
        b.measured(|m| m.rates_read),
        b.measured(|m| m.setup_nanos),
    );
    b.node(
        "scan",
        "Journal Scan",
        format!("streaming; {} in the pinned prefix", plural(prefix, "entry", "entries")),
        Group::Recorded,
        Role::Chosen,
        "Journal::for_each_entry_since — the strike's own journal_position",
        vec![
            "⛔ THE CURVE THAT GROWS. The proved cost model has no term for this \
             at all: `Ratio.Closure` costs a period end off a projection that is \
             already current, and says nothing about what building one costs."
                .into(),
            "Streaming. `entries()` materializes and is documented as doing so; \
             at the twenty-million-lot shape the `Vec<JournalEntry>` alone would \
             be about eighty gigabytes."
                .into(),
        ],
        Some(prefix),
        b.measured(|m| m.entries_walked),
        b.measured(|m| m.walk_nanos),
    );
    b.node(
        "terms",
        "Resolve View Terms",
        match measured {
            Some(m) => format!(
                "cached by config digest — {} lookups, {} read the TOML",
                grouped(m.config_lookups), grouped(m.config_misses)
            ),
            None => "cached by config digest".into(),
        },
        Group::Recorded,
        Role::Chosen,
        "NavFold::def_for",
        vec![
            "⛔ FROM THE DIGEST THE ENTRY PINNED, never from ACTIVE. A settlement \
             date is a trade date rolled over a calendar, so resolving it from \
             whatever is in force now makes a recorded NAV re-derive differently \
             the moment somebody declares a holiday."
                .into(),
            "⚠ Not estimable. How many distinct configurations a book names is a \
             property of the book, not a term of the model — it is measured or \
             it is blank."
                .into(),
        ],
        None,
        b.measured(|m| m.config_misses),
        None,
    );
    b.node(
        "place",
        "Place by Trade Date",
        "trade date → day, through the pinned calendar".into(),
        Group::Recorded,
        Role::Chosen,
        "views::ViewDef::placement",
        vec!["A date that will not parse is unplaceable and named, never a silent \
              absence."
            .into()],
        None,
        b.measured(|m| m.placed),
        None,
    );
    b.node(
        "recognise",
        "Recognise vs Cut",
        match measured {
            Some(m) => format!(
                "{} of {} entries recognised",
                grouped(m.recognised), grouped(m.entries_walked)
            ),
            None => "the view's convention, as of the valuation day".into(),
        },
        Group::Recorded,
        Role::Chosen,
        "Ratio.Views.a_fold_with_no_cut_hides_the_settlement_gap",
        vec!["⛔ THE CUT IS DERIVED FROM THE VALUATION POINT, not carried beside \
              it. Folded to the end of history every view agrees, because \
              everything eventually settles."
            .into()],
        None,
        b.measured(|m| m.recognised),
        None,
    );
    b.node(
        "classify",
        "Classify by Account Type",
        "assets and liabilities only".into(),
        Group::Recorded,
        Role::Chosen,
        "NavFold::push",
        vec!["A liability nets negative because it is credit-normal, so summing \
              assets and liabilities subtracts without a special case."
            .into()],
        None,
        None,
        None,
    );
    b.node(
        "accumulate",
        "Accumulate per Currency",
        match est {
            Some(e) => format!("{} running totals, in i128", grouped(e.fx)),
            None => "one running total per currency, in i128".into(),
        },
        Group::Recorded,
        Role::Chosen,
        "Ratio.Chart.Dimensions.a_flat_total_hides_a_currency_mismatch",
        vec!["⛔ PER CURRENCY. This was one accumulator, and the strike added \
              dollars, euros and pounds and called the total USD — returning the \
              identical figure for a one-currency and a three-currency book. It \
              tied the whole way: trial balance 0, digest reproducible, replay \
              reporting reproduced, of the wrong figure."
            .into()],
        est.map(|e| e.fx),
        b.measured(|m| m.currencies_accumulated),
        None,
    );
    b.node(
        "digest",
        "Feed Digest",
        format!("{} — the WHOLE prefix", plural(prefix, "entry", "entries")),
        Group::Recorded,
        Role::Chosen,
        "ratio_nav::strike — journal_digest",
        vec!["⛔ RECOGNISED OR NOT. The digest covers every entry in the prefix, \
              because a replay has to be able to notice that an entry this view \
              DECLINED was edited afterwards."
            .into()],
        Some(prefix),
        b.measured(|m| m.digest_fed),
        None,
    );
    b.node(
        "translate",
        "Translate",
        match est {
            Some(e) => format!("{} rates, rounded down, in i128", grouped(e.fx)),
            None => "one rate per currency, rounded down, in i128".into(),
        },
        Group::Recorded,
        Role::Chosen,
        "Ratio.Closure.fxCost",
        vec!["⚠ Translation rounds and relief does not. A translated figure has \
              no exact answer at any rate with finitely many digits, so refusing \
              would refuse every foreign holding."
            .into()],
        est.map(|e| e.fx),
        None,
        b.measured(|m| m.finish_nanos),
    );
    b.node(
        "nav",
        "Net Asset Value",
        format!("{} minor units", grouped(strike.net_asset_value)),
        Group::Recorded,
        Role::Chosen,
        "the figure this strike recorded",
        vec![],
        None,
        None,
        None,
    );
    b.node(
        "trialbalance",
        "Trial Balance Difference",
        grouped(strike.trial_balance_difference),
        Group::Recorded,
        Role::Chosen,
        "NavFold::finish",
        vec!["⚠ The DIFFERENCE is safe to sum flat and only the difference. Every \
              currency nets to zero independently, so the flat total is zero too \
              — the two column totals would not survive mixing."
            .into()],
        None,
        None,
        None,
    );
    b.node(
        "journaldigest",
        "Journal Digest",
        format!("{}…", short(&strike.journal_digest)),
        Group::Recorded,
        Role::Chosen,
        "what makes a rewritten history visible",
        vec![],
        None,
        None,
        None,
    );

    // The two places a strike stops rather than reporting a figure.
    b.node(
        "refuse-unplaceable",
        "Refuse: unplaceable entry",
        match measured {
            Some(m) if m.unplaceable > 0 => {
                format!("{} this view cannot date", plural(m.unplaceable, "entry", "entries"))
            }
            Some(_) => "none".into(),
            None => "an entry this view cannot date".into(),
        },
        Group::Recorded,
        Role::Refusal,
        "ratio_nav::strike — the unplaceable bail",
        vec!["⛔ REFUSED, NEVER SILENTLY DROPPED. An entry left out of a figure \
              nobody was told about is value leaving a book whose trial balance \
              goes on tying."
            .into()],
        None,
        b.measured(|m| m.unplaceable),
        None,
    );
    b.node(
        "refuse-rate",
        "Refuse: no rate for a currency",
        "a fund holding what nothing translates".into(),
        Group::Recorded,
        Role::Refusal,
        "NavFold::finish",
        vec!["Skipping the leg reports a fund that does not hold what it holds; \
              translating at par reports yen as though it were dollars. Neither \
              looks wrong on the page."
            .into()],
        None,
        None,
        None,
    );

    b.edge("open", "chart", EdgeKind::Flow, None);
    b.edge("open", "rates", EdgeKind::Flow, None);
    b.edge("open", "scan", EdgeKind::Flow, None);
    b.edge("scan", "terms", EdgeKind::Flow, Some(prefix));
    // ⛔ THE FORK. The digest walks the same entries and is not downstream of
    // recognition — which is exactly what makes this a DAG rather than a chain,
    // and what a diagram drawn as a list would hide.
    b.edge("scan", "digest", EdgeKind::Flow, Some(prefix));
    b.edge("digest", "journaldigest", EdgeKind::Flow, None);
    b.edge("terms", "place", EdgeKind::Flow, b.measured(|m| m.config_lookups));
    b.edge("place", "recognise", EdgeKind::Flow, b.measured(|m| m.placed));
    b.edge("place", "refuse-unplaceable", EdgeKind::Refusal, b.measured(|m| m.unplaceable));
    b.edge("recognise", "classify", EdgeKind::Flow, b.measured(|m| m.recognised));
    b.edge("chart", "classify", EdgeKind::Flow, None);
    b.edge("classify", "accumulate", EdgeKind::Flow, None);
    b.edge("accumulate", "translate", EdgeKind::Flow, est.map(|e| e.fx));
    b.edge("accumulate", "trialbalance", EdgeKind::Flow, None);
    b.edge("rates", "translate", EdgeKind::Flow, None);
    b.edge("translate", "nav", EdgeKind::Flow, None);
    b.edge("translate", "refuse-rate", EdgeKind::Refusal, None);

    // ── The maintained totals ────────────────────────────────────────────

    b.node(
        "totals",
        "Read Totals by (dim, currency)",
        match shape {
            Some(s) => format!(
                "{} rows; the model charges {} — one per security",
                grouped(s.total_rows), grouped(s.estimate.marks)
            ),
            None => "one row per dimension and currency".into(),
        },
        Group::Maintained,
        Role::Chosen,
        "Ratio.Plan.aggregate_cost_is_the_securities",
        vec![
            "⭐ THE PLANNER THEOREM. The fast plan equals the slow one exactly \
             when every maintained total is honest — `Ratio.Plan.aggregate_agrees\
             _with_scan` — and `Ratio.Lots.cost_is_conserved` is what keeps them \
             honest across a sale. Take that away and the fast plan is a \
             different number wearing the same name."
                .into(),
            "⛔ Keyed on (dimension, currency). A total over both is not a figure."
                .into(),
        ],
        est.map(|e| e.marks),
        None,
        None,
    );
    b.node(
        "factors",
        "Read Through Factors",
        match est {
            Some(e) => format!(
                "{}, nothing rewritten",
                plural(e.dials.open_actions, "open action", "open actions")
            ),
            None => "open actions, read at the point of use".into(),
        },
        Group::Maintained,
        Role::Chosen,
        "Ratio.Closure.factorActionCost — zero",
        vec![
            "⭐ THIS IS WHAT MAKES AN OUTSTANDING ACTION FREE. Nothing is \
             rewritten, so bringing the book up to date costs nothing."
                .into(),
            "⚠ Not free in general — each lot READ must still check that every \
             step divided, which is O(splits) on the lots actually touched. A \
             sale relieving five lots pays for five. A NAV pays for none."
                .into(),
        ],
        est.map(|_| 0),
        None,
        None,
    );
    b.node(
        "capital",
        "Capital Activity",
        "not estimated".into(),
        Group::Maintained,
        Role::Chosen,
        "Ratio.Closure.capitalCost",
        vec!["⛔ NOT ESTIMATED, AND NOT ZERO EITHER. Counting subscriptions and \
              redemptions needs the chart roles, and the maintained projection \
              deliberately does not know the chart. A zero here would be a guess \
              wearing a decimal point, so the term is left blank and the totals \
              beside it say they exclude it."
            .into()],
        None,
        None,
        None,
    );
    b.node(
        "filter",
        "Filter Assets and Liabilities",
        "the NAV's question about the chart".into(),
        Group::Maintained,
        Role::Chosen,
        "Projection::translate",
        vec![],
        None,
        None,
        None,
    );
    b.node(
        "translate-2",
        "Translate",
        match est {
            Some(e) => format!("{} rates over per-currency subtotals", grouped(e.fx)),
            None => "one rate per currency".into(),
        },
        Group::Maintained,
        Role::Chosen,
        "Ratio.Closure.fxCost",
        vec![],
        est.map(|e| e.fx),
        None,
        None,
    );
    b.node(
        "nav-2",
        "Net Asset Value",
        match est {
            Some(e) => format!("{} reads, whatever the history", grouped(e.factored_reads)),
            None => "the same figure, off the chart".into(),
        },
        Group::Maintained,
        Role::Chosen,
        "Ratio.Closure.factored_nav_never_reads_the_lots",
        vec!["⭐ Flat in fragmentation. A fund's twentieth year strikes as fast \
              as its first — and it is a theorem rather than a hope, because the \
              maintained totals stand in for the lots."
            .into()],
        None,
        None,
        None,
    );

    b.edge("factors", "totals", EdgeKind::Flow, None);
    b.edge("totals", "filter", EdgeKind::Flow, shape.map(|s| s.total_rows));
    b.edge("filter", "translate-2", EdgeKind::Flow, None);
    b.edge("translate-2", "nav-2", EdgeKind::Flow, None);
    b.edge("capital", "nav-2", EdgeKind::Flow, None);

    // ── The plans not taken, and the thing nothing reads ─────────────────

    b.node(
        "scan-lots",
        "Scan Open Lots",
        match shape {
            Some(s) => format!("{} open lots", grouped(s.open_lots_held)),
            None => "every open tax lot".into(),
        },
        Group::Maintained,
        Role::Rejected,
        "Ratio.Plan.scan_cost_is_the_lots",
        vec!["Always right, and its cost is the number of lots — which for a fund \
              that has traded for years is millions. The aggregate plan gives the \
              same answer for the securities."
            .into()],
        shape.map(|s| s.open_lots_held),
        None,
        None,
    );
    b.node(
        "rewrite",
        "Apply Open Actions by Rewrite",
        match est {
            Some(e) => format!(
                "{} × {} lots per instrument = {}",
                plural(e.dials.open_actions, "open action", "open actions"),
                grouped(e.dials.lots_per), grouped(e.actions)
            ),
            None => "every lot of the instrument the action touches".into(),
        },
        Group::Maintained,
        Role::Rejected,
        "Ratio.Closure.the_cliff",
        vec![
            "⛔ THE ONLY SUPERLINEAR TERM, AND THE ONLY RETROACTIVE ONE. A split \
             reaches into ONE instrument and touches every LOT of it — forty \
             thousand for an S&P tracker, against five hundred for the whole \
             quiet period end."
                .into(),
            "⚠ `lots_per` here is an average, derived by dividing the open lots \
             by the securities. The model's own term wants the fragmentation of \
             the instrument the action touches, and this fund's is not uniform."
                .into(),
        ],
        est.map(|e| e.actions),
        None,
        None,
    );
    b.node(
        "unread-lots",
        "Open Tax Lots",
        match shape {
            Some(s) => format!("{} lots — 0 reads", grouped(s.open_lots_held)),
            None => "0 reads".into(),
        },
        Group::Maintained,
        Role::Unread,
        "Ratio.Closure.factored_nav_never_reads_the_lots",
        vec!["⭐ THE CLAIM THE WHOLE SYSTEM RESTS ON, DRAWN. Fragmentation is not \
              in the cost of a NAV at all. This zero is proved, not unmeasured."
            .into()],
        Some(0),
        None,
        None,
    );

    b.edge("scan-lots", "filter", EdgeKind::Flow, shape.map(|s| s.open_lots_held));
    b.edge("rewrite", "totals", EdgeKind::Flow, est.map(|e| e.actions));
    b.edge("unread-lots", "totals", EdgeKind::Unread, Some(0));

    Plan {
        name: name.to_string(),
        view: strike.view.clone(),
        nodes: b.nodes,
        edges: b.edges,
        shape: shape.cloned(),
        estimate_refusal: refusal.to_string(),
        analyzed: measured.is_some(),
        nanos_per_read: cal.nanos_per_read,
        provenance: cal.provenance.clone(),
        chosen_reads: est.map(|e| e.factored_reads),
        rewrite_reads: est.map(|e| e.reads),
        scan_reads: shape.map(|s| s.open_lots_held),
    }
}

/// The first twelve characters of a digest, which is what a screen shows.
fn short(d: &str) -> String {
    d.chars().take(12).collect()
}

/// `252843` → `252,843`.
///
/// ⛔ IN THE PROSE ONLY. Every FIGURE on this message crosses the wire as bare
/// digits and is grouped by the client, which is the rule for every int64 on
/// this contract. What is grouped here is the text of a step's `detail` — where
/// the number is part of a sentence and an ungrouped `252843` is the difference
/// between a reader seeing a quarter of a million and seeing a serial number.
fn grouped(n: i64) -> String {
    let neg = n < 0;
    let digits = n.unsigned_abs().to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 1);
    if neg {
        out.push('-');
    }
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `1 open action` / `2 open actions`. A trailing `(s)` is a sentence nobody
/// wrote, on a screen whose whole argument is that its figures read like prose.
fn plural(n: i64, one: &str, many: &str) -> String {
    format!("{} {}", grouped(n), if n == 1 { one } else { many })
}

/// A duration a person can read, for a caller rendering this at a terminal.
pub fn duration(n: Option<i64>) -> String {
    match n {
        Some(n) => human_nanos(n),
        // ⛔ NOT "0 ns". A dash is unmeasured; a zero is a measurement.
        None => "—".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closure::{estimate, Dials};

    fn cal() -> Calibration {
        Calibration { nanos_per_read: 1_000, provenance: "a test".into() }
    }

    fn strike() -> Strike {
        Strike {
            view: "abor".into(),
            id: "2026-06-30T1600Z".into(),
            valuation_time: 1_782_835_200,
            actor: "e.marsh".into(),
            journal_position: 1_769_907,
            journal_digest: "abcdef0123456789".into(),
            net_asset_value: 13_443_918_751,
            trial_balance_difference: 0,
            config_digest: "0011223344556677".into(),
        }
    }

    fn shape(open_actions: i64) -> Shape {
        let d = Dials {
            securities: 500,
            currencies: 3,
            lots_per: 40_000,
            open_actions,
            capital_txns: 0,
        };
        Shape {
            estimate: estimate(d, &cal()).unwrap(),
            accounts: 12,
            total_rows: 36,
            open_lots_held: 20_000_000,
        }
    }

    fn plan(open_actions: i64) -> Plan {
        let s = shape(open_actions);
        plan_of("funds/f/views/abor/navStrikes/x", &strike(), Some(&s), "", None, &cal())
    }

    #[test]
    fn the_chosen_maintained_nodes_sum_to_the_reported_total() {
        // A diagram whose steps do not add up to the figure beside it is a
        // picture of a cost nobody pays. The arithmetic underneath is proved in
        // `Ratio.Closure`; what is unproved is that this description spends it
        // on the same things.
        let p = plan(1);
        let summed = p.chosen_reads_in(Group::Maintained);
        // ⚠ The capital term is deliberately absent from BOTH sides, and the
        // dials say so: a book with capital activity would make this assertion
        // wrong in a way that is the model's honesty rather than a defect.
        assert_eq!(p.shape.as_ref().unwrap().estimate.dials.capital_txns, 0);
        assert_eq!(Some(summed), p.chosen_reads);
    }

    #[test]
    fn the_rejected_scan_costs_the_lots_and_the_chosen_plan_does_not() {
        let p = plan(0);
        let scan = p.nodes.iter().find(|n| n.id == "scan-lots").unwrap();
        assert_eq!(scan.role, Role::Rejected);
        assert_eq!(scan.estimated_reads, Some(20_000_000));
        // `Ratio.Closure.factored_nav_never_reads_the_lots`, as an assertion
        // about what this screen actually draws rather than about the model.
        assert!(p.chosen_reads.unwrap() < 1_000);
        let unread = p.nodes.iter().find(|n| n.id == "unread-lots").unwrap();
        assert_eq!(unread.role, Role::Unread);
        assert_eq!(unread.estimated_reads, Some(0));
    }

    #[test]
    fn an_open_action_moves_the_rewrite_node_and_leaves_the_chosen_plan_flat() {
        // `Ratio.Closure.the_cliff`: one outstanding split is eighty times the
        // whole quiet period end under a rewrite, and free under a factor.
        let quiet = plan(0);
        let busy = plan(1);
        assert_eq!(quiet.chosen_reads, busy.chosen_reads);
        assert!(busy.rewrite_reads.unwrap() > quiet.rewrite_reads.unwrap());
        let moved = |p: &Plan, id: &str| {
            p.nodes.iter().find(|n| n.id == id).unwrap().estimated_reads
        };
        assert_eq!(moved(&quiet, "rewrite"), Some(0));
        assert_eq!(moved(&busy, "rewrite"), Some(40_000));
        // Only that one. Everything else the chosen plan reads is unchanged.
        for id in ["totals", "translate-2", "factors", "scan-lots"] {
            assert_eq!(moved(&quiet, id), moved(&busy, id), "{id} moved");
        }
    }

    #[test]
    fn an_unanalyzed_plan_reports_no_measurement_rather_than_zero() {
        // ⛔ THE NEGATIVE TEST THAT MATTERS. A rendered `0` that nothing measured
        // reads as "instant", and this whole module's `Option`s exist for it.
        let p = plan(1);
        assert!(!p.analyzed);
        assert!(p.nodes.iter().all(|n| n.actual_rows.is_none()));
        assert!(p.nodes.iter().all(|n| n.actual_nanos.is_none()));
        assert_eq!(duration(None), "—");
    }

    #[test]
    fn an_analyzed_plan_carries_what_the_fold_saw() {
        let s = shape(1);
        let m = Measured {
            accounts_read: 12,
            rates_read: 3,
            setup_nanos: 4_000,
            entries_walked: 1_769_907,
            walk_nanos: 8_600_000_000,
            config_lookups: 1_769_907,
            config_misses: 2,
            placed: 1_769_907,
            unplaceable: 0,
            recognised: 1_769_900,
            digest_fed: 1_769_907,
            currencies_accumulated: 3,
            finish_nanos: 12_000,
        };
        let p = plan_of("n", &strike(), Some(&s), "", Some(&m), &cal());
        assert!(p.analyzed);
        let n = |id: &str| p.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(n("scan").actual_rows, Some(1_769_907));
        // ⛔ The cache is the optimization, and the node has to be able to say
        // so: two reads of the TOML behind 1.77 million lookups.
        assert_eq!(n("terms").actual_rows, Some(2));
        assert!(n("terms").actual_rows.unwrap() < n("scan").actual_rows.unwrap());
    }

    #[test]
    fn a_view_the_projection_cannot_answer_carries_the_refusal_and_no_figures() {
        let p = plan_of(
            "funds/f/views/settle/navStrikes/x",
            &strike(),
            None,
            "the maintained projection folds the whole journal with no cut",
            None,
            &cal(),
        );
        assert!(!p.estimate_refusal.is_empty());
        assert_eq!(p.chosen_reads, None);
        assert_eq!(p.scan_reads, None);
        // Every modelled figure is absent — not defaulted to zero, which would
        // draw a fund with no chart, no currencies and no lots.
        assert!(p.nodes.iter().filter(|n| n.group == Group::Maintained).all(|n| {
            n.id == "unread-lots" || n.id == "factors" || n.estimated_reads.is_none()
        }));
        // The plan still has its shape: a refusal is not an empty page.
        assert!(p.nodes.len() > 15);
        assert!(!p.edges.is_empty());
    }

    #[test]
    fn the_digest_forks_off_the_walk_rather_than_hanging_off_recognition() {
        // ⛔ This is the edge that makes it a DAG, and the one a diagram drawn as
        // a list would quietly straighten: the digest covers entries the view
        // DECLINED, so it cannot be downstream of the filter that declined them.
        let p = plan(0);
        assert!(p.edges.iter().any(|e| e.source == "scan" && e.target == "digest"));
        assert!(!p.edges.iter().any(|e| e.target == "digest" && e.source == "recognise"));
    }

    #[test]
    fn every_edge_names_a_node_that_exists() {
        let p = plan(1);
        for e in &p.edges {
            assert!(p.nodes.iter().any(|n| n.id == e.source), "no node {}", e.source);
            assert!(p.nodes.iter().any(|n| n.id == e.target), "no node {}", e.target);
        }
    }

    #[test]
    fn every_node_cites_something() {
        let p = plan(1);
        for n in &p.nodes {
            assert!(!n.cites.is_empty(), "{} cites nothing", n.id);
        }
    }
}
