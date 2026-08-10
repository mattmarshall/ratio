//! Relieving tax lots — the walk, over decisions made in Lean.
//!
//! `Ratio.Lots` proves what one relief computes and `Ratio.Lots.Relief` proves
//! what it produces; `//tla:relief_engine_check` proves what a run of them must
//! not do. This is the fold those describe, and it deliberately owns nothing
//! they decide: `takes_whole_lot`, `partial_divides`, `partial_cost` and
//! `lot_is_sound` are emitted.
//!
//! # ⛔ What conservation cannot see
//!
//! Every failure this module guards against leaves the books tying:
//!
//! * a **husk** — a lot holding zero units and carrying cost — is consumed by
//!   `takes_whole_lot`, since `0 <= want`, and hands its whole basis to a sale
//!   that received no units for it. `Ratio.Lots.Edges.a_husk_gives_away_its_cost`
//! * a **partial** relief that rounds gives up the wrong basis by a minor unit.
//!   `Ratio.Lots.partial_relief_is_exactly_pro_rata`
//! * an **unsorted** list makes "FIFO" mean whatever order storage returned.
//!   `Ratio.Lots.Edges.sorting_is_the_callers_job`
//!
//! In all three the entry balances, the units are right, and only the realized
//! gain is wrong — which is the figure nobody reconciles because it has no
//! counterparty.

use anyhow::{bail, Result};

use crate::generated_lots::{lot_is_sound, partial_cost, partial_divides, takes_whole_lot};

/// A calendar day, as days since 1970-01-01.
///
/// ⛔ A DATE IS A NUMBER, NOT A STRING, and this field proved it. `Lot.acquired`
/// held an `Option<String>`, so a book with a million open lots retained a
/// MILLION small heap allocations — and the mark went from 927 µs to 5.0 ms on
/// the same five hundred price lookups, because the heap underneath them had
/// been shredded. The lookups never got more numerous.
///
/// ⭐ AND IT IS ALSO THE RIGHT TYPE. `Ratio.Lots.Methods.heldDays` is
/// `asOf - acquired` over `Int`: the holding period was being decided by
/// parsing ISO text on every classification, and `Method::arrange` sorted dates
/// as STRINGS — correct only because the format is fixed, and only for as long
/// as nobody wrote one that was not.
///
/// `i32` spans ±5.8 million years, and a lot is lighter for it than for the
/// pointer alone.
pub type Day = i32;

/// A tax lot: when it was acquired, what it holds, what it cost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lot {
    /// Acquisition ordinal. ⛔ FIFO IS THIS FIELD, not the order of the vector —
    /// `relieve` sorts by it rather than trusting what it was handed.
    pub seq: u64,
    pub units: i64,
    pub cost: i64,
    /// The day it was acquired, `YYYY-MM-DD`.
    ///
    /// ⛔ OPTIONAL, AND ITS ABSENCE IS NOT A DEFAULT. A lot opened by an entry
    /// with no trade date cannot be classified short- or long-term, and the two
    /// obvious fallbacks are wrong in opposite directions: the epoch makes
    /// everything long-term (the favorable rate, on records that do not support
    /// it) and today makes everything short-term (punitive, on a holding held
    /// for years). The holding-period methods REFUSE such a holding.
    pub acquired: Option<Day>,
}

/// What a sale took out of one lot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Taken {
    pub seq: u64,
    pub units: i64,
    pub cost: i64,
    /// The day the relieved lot was acquired, carried through from it.
    ///
    /// ⭐ WITHOUT THIS THE REALIZED GAIN CANNOT BE CLASSIFIED. Short- and
    /// long-term gains are taxed differently, so a disposal that does not know
    /// when the lot it gave up was acquired cannot be reported — and the whole
    /// reason a fund chooses a holding-period method is to control that split.
    /// A `Taken` carrying only a cost is enough for the books and not enough for
    /// the return.
    ///
    /// ⛔ OPTIONAL, AND ITS ABSENCE IS NOT A DEFAULT. A lot opened by an entry
    /// with no trade date cannot be classified short- or long-term, and the two
    /// obvious fallbacks are wrong in opposite directions: the epoch makes
    /// everything long-term (the favorable rate, on records that do not support
    /// it) and today makes everything short-term (punitive, on a holding held
    /// for years). The holding-period methods REFUSE such a holding.
    pub acquired: Option<Day>,
}

/// What a relief produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relieved {
    pub taken: Vec<Taken>,
    pub left: Vec<Lot>,
    /// What the relieved lots cost. `Ratio.Lots.takenCost`.
    pub cost: i64,
}

impl Relieved {
    /// Proceeds less the cost given up. `Ratio.Lots.Relief.gain`.
    ///
    /// ⚠ THE ONE FIGURE HERE WITH NO COUNTERPARTY. A wrong NAV is caught by a
    /// reconciliation; a wrong realized gain is caught by nobody until a tax
    /// authority asks, which is why every guard in this module is about it
    /// rather than about conservation.
    pub fn gain(&self, proceeds: i64) -> Result<i64> {
        // ⛔ `sub`, NOT `add(proceeds, -self.cost)`. The negation in that form
        // is an ordinary expression in the argument list, so it wraps before
        // anything checks it and `add` then verifies a number nobody asked for.
        // `checked::a_difference_routed_through_negation_is_not_the_same_
        // function` is the demonstration.
        ratio_common::checked::sub(proceeds, self.cost, "the realized gain")
    }
}

/// Which lots a sale gives up.
///
/// ⛔ A TERM OF AN ADMINISTRATION AGREEMENT, NOT AN IMPLEMENTATION CHOICE. The
/// method decides the REALIZED GAIN — the same holding and the same trade
/// produce different taxable income under each, with no figure on the balance
/// sheet moving. `Ratio.Lots.Methods.the_method_decides_the_taxable_gain`.
///
/// ⚠ AND THE SPACE IS NOT ALL ORDERINGS. These four sort and walk. SPECIFIC
/// IDENTIFICATION is a selection that may take from the middle of a holding, and
/// AVERAGE COST pools the holding so there is no lot to give up at all — both
/// are modelled in `Ratio.Lots.Methods` and neither belongs in this enum.
/// Adding them here as variants is the mistake the Lean file exists to prevent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Method {
    /// Oldest acquisition first.
    #[default]
    Fifo,
    /// Newest first.
    Lifo,
    /// ⛔ Dearest PER UNIT first — the method chosen to reduce a gain.
    /// `Ratio.Lots.Methods.hifo_is_per_unit_not_per_lot`: by TOTAL cost a large
    /// cheap lot outranks a small dear one, and taking it shelters less. Getting
    /// this wrong does not fail; it overstates the taxable gain, which is the
    /// opposite of what the method was chosen for.
    Hifo,
    /// Cheapest per unit first — chosen to REALIZE a gain deliberately, which a
    /// fund does against a capital-loss carryforward.
    Lofo,
    /// Longest held first. ⛔ REFUSES a holding where any lot has no
    /// acquisition date. `Ratio.Lots.Methods.a_missing_acquisition_date_refuses`.
    LongestHeldFirst,
    /// Shortest held first.
    ShortestHeldFirst,
}

impl From<ratio_rules::LotMethod> for Method {
    /// The configured method, as the engine's.
    ///
    /// ⛔ A CONVERSION RATHER THAN A SHARED TYPE, so the rules crate does not
    /// depend on the engine and the engine does not depend on TOML. The two
    /// enums are checked against each other by
    /// `the_configured_method_reaches_the_engine`, because a silent mismatch
    /// here would mean a fund declaring HIFO and being relieved FIFO — with
    /// nothing anywhere reporting a difference.
    fn from(m: ratio_rules::LotMethod) -> Self {
        match m {
            ratio_rules::LotMethod::Fifo => Method::Fifo,
            ratio_rules::LotMethod::Lifo => Method::Lifo,
            ratio_rules::LotMethod::Hifo => Method::Hifo,
            ratio_rules::LotMethod::Lofo => Method::Lofo,
            ratio_rules::LotMethod::LongestHeldFirst => Method::LongestHeldFirst,
            ratio_rules::LotMethod::ShortestHeldFirst => Method::ShortestHeldFirst,
        }
    }
}

impl Method {
    /// Whether this method needs to know when each lot was acquired.
    pub fn needs_acquisition_dates(self) -> bool {
        matches!(self, Method::LongestHeldFirst | Method::ShortestHeldFirst)
    }

    /// How this method takes a holding, in the words an operator would use.
    ///
    /// ⛔ FOR MESSAGES THAT WOULD OTHERWISE ASSERT A METHOD THEY DID NOT CHECK.
    /// The lot-book drift break described every relief as "oldest-first"
    /// regardless of what ran, so the one line pointing at a disagreement
    /// misdescribed how the figure it was reporting had been produced.
    pub fn describe(self) -> &'static str {
        match self {
            Method::Fifo => "oldest-first",
            Method::Lifo => "newest-first",
            Method::Hifo => "dearest-per-unit-first",
            Method::Lofo => "cheapest-per-unit-first",
            Method::LongestHeldFirst => "longest-held-first",
            Method::ShortestHeldFirst => "shortest-held-first",
        }
    }

    /// Order a holding for this method.
    ///
    /// ⛔ THE TIEBREAK APPLIES ONLY ON A TIE. `dearer(a,b) || a.seq <= b.seq` is
    /// true whenever the sequence ascends, whatever the costs — so the tiebreak
    /// overrides the method and HIFO quietly performs FIFO. Lean's `decide`
    /// reported that theorem FALSE, which is the only reason it was caught: a
    /// test asserting "HIFO differs from LOFO" would have PASSED, because both
    /// were broken the same way in opposite directions.
    fn arrange(self, lots: &mut [Lot]) {
        // Dearer per unit, cross-multiplied. ⚠ Not `cost / units`: integer
        // division ties lots whose per-unit costs differ by less than a minor
        // unit, and a method that ties lots which are not tied picks by whatever
        // the sort does next.
        fn dearer(a: &Lot, b: &Lot) -> std::cmp::Ordering {
            (b.units as i128 * a.cost as i128).cmp(&(a.units as i128 * b.cost as i128))
        }
        match self {
            Method::Fifo => lots.sort_by_key(|l| l.seq),
            Method::Lifo => lots.sort_by_key(|l| std::cmp::Reverse(l.seq)),
            Method::Hifo => lots.sort_by(|a, b| dearer(b, a).then(a.seq.cmp(&b.seq))),
            Method::Lofo => lots.sort_by(|a, b| dearer(a, b).then(a.seq.cmp(&b.seq))),
            // ⚠ ISO dates compare as strings in date order, which is the whole
            // reason the format is fixed. A date held as `03/04/2026` would sort
            // by month and nobody would see it in a total.
            Method::LongestHeldFirst => {
                lots.sort_by(|a, b| a.acquired.cmp(&b.acquired).then(a.seq.cmp(&b.seq)))
            }
            Method::ShortestHeldFirst => {
                lots.sort_by(|a, b| b.acquired.cmp(&a.acquired).then(a.seq.cmp(&b.seq)))
            }
        }
    }
}

/// Cost per unit, compared exactly.
///
/// ⛔ CROSS-MULTIPLIED, NEVER `cost / units`. Integer division ties lots whose
/// per-unit costs differ by less than a minor unit, and a method that ties lots
/// which are not tied gives them up in whatever order the container happened to
/// hold them. `Ratio.Lots.Methods.hifo_is_per_unit_not_per_lot`.
///
/// ⚠ `i128` because the products are `cost x units`, and both are `i64`.
///
/// ⛔ `Eq` IS HAND-WRITTEN TO AGREE WITH `Ord`, AND DERIVING IT WAS A BUG. A
/// derived `Eq` compares the FIELDS, so `100/10` and `200/20` were unequal
/// while the comparator called them equal — and `BTreeMap` requires
/// `a.cmp(b) == Equal` exactly when `a == b`. Two lots at the same per-unit cost
/// would have been two different keys or one, depending on which the map
/// happened to consult. My own totality test caught it on the first run.
#[derive(Clone, Copy, Debug)]
pub struct PerUnit {
    cost: i64,
    units: i64,
}

impl PartialEq for PerUnit {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for PerUnit {}

impl Ord for PerUnit {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.cost as i128 * other.units as i128)
            .cmp(&(other.cost as i128 * self.units as i128))
    }
}

impl PartialOrd for PerUnit {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Where a lot sits in the order its method gives up lots in.
///
/// ⛔ THE `seq` IN THE TUPLE IS WHY THIS IS A TOTAL ORDER, and it is structural
/// rather than a comparator somebody has to remember to write. HIFO once
/// silently performed FIFO because the tiebreak was written
/// `dearer(a, b) || a.seq <= b.seq`, which is true whenever the sequence
/// ascends, whatever the costs — and only Lean's `decide` caught it. A tuple
/// key cannot express that mistake.
/// ⛔ THE DIRECTION IS BAKED INTO THE KEY, AND THE `seq` TIEBREAK IS ALWAYS
/// ASCENDING. Reversing by popping from the BACK of the map reverses the whole
/// key — including the tiebreak — so lots at an equal per-unit cost came off
/// newest-first while `Method::arrange` takes them oldest-first. The
/// differential test caught it immediately; nothing else would have, because
/// the two orders only differ on ties.
///
/// A holding only ever holds one variant at a time; the enum ordering across
/// variants is never consulted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Rank {
    /// LOFO: cheapest per unit first.
    CostAsc(PerUnit, u64),
    /// HIFO: dearest per unit first.
    CostDesc(std::cmp::Reverse<PerUnit>, u64),
    /// Longest held: earliest acquisition first.
    HeldAsc(Day, u64),
    /// Shortest held: latest acquisition first.
    HeldDesc(std::cmp::Reverse<Day>, u64),
}

/// A position's open lots, kept so the next one to give up is at the front.
///
/// ⛔ THE HOLDING OWNS ITS ORDER; A RELIEF IS A POP. Relief used to copy the
/// whole holding and sort it on EVERY SALE — measured at 7.4 s of a 20.2 s cold
/// build at five hundred lots a position, and 31.1 s over the same number of
/// reliefs at two thousand. That is the term that made the cold build scale with
/// fragmentation rather than with entries.
///
/// ⭐ AND THE ORDER IS A TERM OF AN AGREEMENT, so it changes on a configuration
/// promotion and not otherwise. Ordering work belongs on that rare event, not on
/// every sale — which is the whole idea here.
///
/// ⛔ THE HUSK CHECK MOVED TO `push` FOR THE SAME REASON IT BELONGS THERE. A lot
/// holding nothing and carrying cost is refused when it is OFFERED, not
/// rediscovered by scanning the holding on every subsequent sale — the same
/// judgement `ChartRoles` makes by checking when the configuration is read.
#[derive(Clone, Debug)]
pub struct Holding {
    order: Method,
    /// FIFO and LIFO: `seq` order IS insertion order, so there is nothing to
    /// maintain and both ends are O(1).
    seq: std::collections::VecDeque<Lot>,
    /// The ranked methods: a key fixed at acquisition, so this is a priority
    /// queue rather than something to re-sort.
    ranked: std::collections::BTreeMap<Rank, Lot>,
}

impl Default for Holding {
    fn default() -> Self {
        Self {
            order: Method::Fifo,
            seq: Default::default(),
            ranked: Default::default(),
        }
    }
}

impl Holding {
    /// A holding that gives lots up under `order`.
    pub fn new(order: Method) -> Self {
        Self { order, ..Default::default() }
    }

    /// How many lots are open.
    pub fn len(&self) -> usize {
        self.seq.len() + self.ranked.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn rank(order: Method, l: &Lot) -> Option<Rank> {
        match order {
            Method::Fifo | Method::Lifo => None,
            Method::Lofo => Some(Rank::CostAsc(PerUnit { cost: l.cost, units: l.units }, l.seq)),
            Method::Hifo => Some(Rank::CostDesc(
                std::cmp::Reverse(PerUnit { cost: l.cost, units: l.units }),
                l.seq,
            )),
            Method::LongestHeldFirst => l.acquired.map(|d| Rank::HeldAsc(d, l.seq)),
            Method::ShortestHeldFirst => {
                l.acquired.map(|d| Rank::HeldDesc(std::cmp::Reverse(d), l.seq))
            }
        }
    }

    /// Whether the SEQUENCE arm takes from the back. ⚠ Only FIFO and LIFO reach
    /// this; the ranked orders carry their direction in the key, so they always
    /// take the first — which is what keeps their `seq` tiebreak ascending.
    fn reversed(order: Method) -> bool {
        matches!(order, Method::Lifo)
    }

    /// Open a lot.
    ///
    /// ⛔ REFUSES A HUSK HERE, where it is offered. `Ratio.Lots.Edges.a_husk_is_
    /// refused`: a lot holding nothing and carrying cost would be consumed by
    /// the walk and hand over its whole basis for no units, understating the
    /// gain by exactly that amount with every invariant satisfied.
    pub fn push(&mut self, lot: Lot) -> Result<()> {
        if !lot_is_sound(lot.units, lot.cost) {
            bail!(
                "lot {} holds {} units and carries {} — a holding of nothing that owes \
                 something is not a lot, and relieving it would give away its basis for \
                 no units at all",
                lot.seq,
                lot.units,
                lot.cost
            );
        }
        match Self::rank(self.order, &lot) {
            None => self.seq.push_back(lot),
            Some(r) => {
                self.ranked.insert(r, lot);
            }
        }
        Ok(())
    }

    /// Put the holding into `order`, if it is not already.
    ///
    /// ⚠ O(n log n), and paid ONCE PER CONFIGURATION CHANGE rather than once per
    /// sale. A fund that never changes method never pays it after the first lot.
    fn reorder(&mut self, order: Method) {
        if self.order == order {
            return;
        }
        let mut all: Vec<Lot> = self.drain_all();
        // Stable in `seq` so the ranked keys are built from a settled order.
        all.sort_by_key(|l| l.seq);
        self.order = order;
        for l in all {
            match Self::rank(order, &l) {
                None => self.seq.push_back(l),
                Some(r) => {
                    self.ranked.insert(r, l);
                }
            }
        }
    }

    fn drain_all(&mut self) -> Vec<Lot> {
        let mut out: Vec<Lot> = self.seq.drain(..).collect();
        out.extend(std::mem::take(&mut self.ranked).into_values());
        out
    }

    /// The lots this holding currently has, oldest acquisition first.
    ///
    /// ⚠ SORTED ON READ, not maintained. This is a screen reading one position;
    /// the fold's cost is what the structure above is arranged for.
    pub fn lots(&self) -> Vec<Lot> {
        let mut out: Vec<Lot> = self.seq.iter().chain(self.ranked.values()).cloned().collect();
        out.sort_by_key(|l| l.seq);
        out
    }

    /// Take the next lot the order gives up, or `None` when empty.
    fn pop(&mut self) -> Option<Lot> {
        if Self::reversed(self.order) {
            self.seq.pop_back()
        } else {
            self.seq.pop_front()
        }
        .or_else(|| self.ranked.pop_first().map(|(_, l)| l))
    }

    /// Put a partially-relieved lot back where it came from.
    fn put_back(&mut self, lot: Lot) {
        match Self::rank(self.order, &lot) {
            None => {
                if Self::reversed(self.order) {
                    self.seq.push_back(lot)
                } else {
                    self.seq.push_front(lot)
                }
            }
            Some(r) => {
                self.ranked.insert(r, lot);
            }
        }
    }

    /// Give up `want` units under `method`, mutating the holding.
    ///
    /// ⛔ A MUTATION, NOT A TRANSFORMATION. `relieve_by` returned a whole new
    /// `left` vector that the caller assigned over the old one — a second O(n)
    /// copy per sale, on top of the O(n) copy it made to sort.
    pub fn relieve(&mut self, method: Method, want: i64) -> Result<Relief> {
        if want < 0 {
            bail!("a relief of {want} units is not a relief; a negative sale is a purchase");
        }
        self.reorder(method);
        if method.needs_acquisition_dates() {
            // ⚠ A lot with no date cannot be RANKED by one, so it never entered
            // `ranked` — which would silently exclude it from the walk. Refusing
            // is what `Ratio.Lots.Methods.a_missing_acquisition_date_refuses`
            // asks for, and the count is how this notices.
            if !self.seq.is_empty() {
                bail!(
                    "{} lot(s) have no acquisition date, and {method:?} cannot classify \
                     them — assuming the epoch would make them long-term at the favorable \
                     rate on records that do not support the claim, and assuming today \
                     would make them short-term on holdings that may have been held for \
                     years. Neither is conservative; they are wrong in opposite directions",
                    self.seq.len()
                );
            }
        }

        // ⛔ A REFUSED RELIEF LEAVES THE HOLDING UNTOUCHED, and making relief a
        // MUTATION is what put that at risk. `relieve_by` built a new `left` and
        // the caller assigned it only on success, so a failure could not consume
        // anything; popping in place can — and did, until
        // `a_sale_that_cannot_be_relieved_is_a_break_not_a_failure` said so. A
        // sale that refuses must not have eaten half the position on its way out.
        //
        // ⚠ Bounded by what the relief CONSUMES, not by the holding: one lot in
        // the ordinary case, never the five hundred the old copy touched.
        let mut consumed: Vec<Lot> = Vec::new();
        let mut taken = Vec::new();
        let mut cost = 0i64;
        let mut remaining = want;

        macro_rules! refuse {
            ($($arg:tt)*) => {{
                for lot in consumed.into_iter().rev() {
                    self.put_back(lot);
                }
                bail!($($arg)*)
            }};
        }

        while remaining > 0 {
            let Some(lot) = self.pop() else {
                refuse!(
                    "the holding is short: {remaining} more unit(s) were sold than are held"
                );
            };
            consumed.push(lot.clone());
            if takes_whole_lot(lot.units, remaining) {
                taken.push(Taken {
                    seq: lot.seq,
                    units: lot.units,
                    cost: lot.cost,
                    acquired: lot.acquired,
                });
                cost = ratio_common::checked::add(cost, lot.cost, "the relieved cost")?;
                remaining -= lot.units;
                continue;
            }
            // Part of this lot, and none of the ones behind it.
            if ratio_common::checked::mul(lot.cost, remaining, "a pro-rata lot split").is_err() {
                refuse!("relieving lot {}'s cost of {} by {remaining} units does not fit in 64 bits", lot.seq, lot.cost);
            }
            if !partial_divides(lot.cost, remaining, lot.units) {
                refuse!(
                    // ⚠ THE SAME SENTENCE `relieve_by` USES. I wrote a fresh one
                    // here and the test caught it: two implementations with two
                    // wordings for one refusal is the drift this repo keeps
                    // finding, and the original says WHY — which way to round is
                    // somebody's decision, not arithmetic's.
                    "relieving {remaining} of lot {}'s {} units does not divide its cost of \
                     {} into whole minor units — which way to round is a term of an \
                     administration agreement, not a property of arithmetic",
                    lot.seq,
                    lot.units,
                    lot.cost
                );
            }
            let part = partial_cost(lot.cost, remaining, lot.units);
            taken.push(Taken {
                seq: lot.seq,
                units: remaining,
                cost: part,
                acquired: lot.acquired,
            });
            cost = ratio_common::checked::add(cost, part, "the relieved cost")?;
            self.put_back(Lot {
                seq: lot.seq,
                units: lot.units - remaining,
                cost: ratio_common::checked::sub(lot.cost, part, "the remaining basis")?,
                acquired: lot.acquired,
            });
            remaining = 0;
        }

        Ok(Relief { taken, cost })
    }
}

/// What a relief took. ⛔ No `left`: the holding kept it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Relief {
    pub taken: Vec<Taken>,
    pub cost: i64,
}

impl Relief {
    /// ⛔ `sub`, not `add(proceeds, -cost)` — see `Relieved::gain`.
    pub fn gain(&self, proceeds: i64) -> Result<i64> {
        ratio_common::checked::sub(proceeds, self.cost, "the realized gain")
    }
}

/// Relieve `want` units, oldest lot first.
pub fn relieve(lots: &[Lot], want: i64) -> Result<Relieved> {
    relieve_by(Method::Fifo, lots, want)
}

/// Relieve `want` units under a declared method.
///
/// ⛔ ORDERS THE HOLDING ITSELF. `Ratio.Lots.relieveFifo` takes the head of the
/// list, so it is FIFO exactly when the caller handed it acquisition order — and
/// a projection keyed by instrument does not. Naming a method and then trusting
/// the caller to have implemented it is how a fund's tax position quietly
/// becomes whatever the storage layer returned.
pub fn relieve_by(method: Method, lots: &[Lot], want: i64) -> Result<Relieved> {
    if want < 0 {
        bail!("a relief of {want} units is not a relief; a negative sale is a purchase");
    }
    if method.needs_acquisition_dates() {
        if let Some(l) = lots.iter().find(|l| l.acquired.is_none()) {
            bail!(
                "lot {} has no acquisition date, and {method:?} cannot classify it — \
                 assuming the epoch would make it long-term at the favorable rate on \
                 records that do not support the claim, and assuming today would make it \
                 short-term on a holding that may have been held for years. Neither is \
                 conservative; they are wrong in opposite directions",
                l.seq
            );
        }
    }
    for l in lots {
        // ⛔ `Ratio.Lots.Edges.a_husk_is_refused`. A lot holding nothing and
        // carrying cost would be CONSUMED by the walk below and hand over its
        // whole basis for no units, understating the gain by exactly that
        // amount — with every invariant satisfied.
        if !lot_is_sound(l.units, l.cost) {
            bail!(
                "lot {} holds {} units and carries {} — a holding of nothing that owes \
                 something is not a lot, and relieving it would give away its basis for \
                 no units at all",
                l.seq,
                l.units,
                l.cost
            );
        }
    }

    let mut ordered: Vec<Lot> = lots.to_vec();
    method.arrange(&mut ordered);

    let mut taken = Vec::new();
    let mut cost = 0i64;
    let mut remaining = want;
    let mut left: Vec<Lot> = Vec::new();
    let mut it = ordered.into_iter();

    for lot in it.by_ref() {
        if remaining == 0 {
            left.push(lot);
            break;
        }
        if takes_whole_lot(lot.units, remaining) {
            taken.push(Taken {
                seq: lot.seq,
                units: lot.units,
                cost: lot.cost,
                acquired: lot.acquired,
            });
            cost = ratio_common::checked::add(cost, lot.cost, "the relieved cost")?;
            remaining -= lot.units;
            continue;
        }
        // Part of this lot, and none of the ones behind it.
        ratio_common::checked::mul(lot.cost, remaining, "a pro-rata lot split")?;
        if !partial_divides(lot.cost, remaining, lot.units) {
            bail!(
                "relieving {remaining} of lot {}'s {} units does not divide its cost of {} \
                 into whole minor units — which way to round is a term of an administration \
                 agreement, not a property of arithmetic",
                lot.seq,
                lot.units,
                lot.cost
            );
        }
        let part = partial_cost(lot.cost, remaining, lot.units);
        taken.push(Taken {
            seq: lot.seq,
            units: remaining,
            cost: part,
            acquired: lot.acquired,
        });
        cost = ratio_common::checked::add(cost, part, "the relieved cost")?;
        left.push(Lot {
            seq: lot.seq,
            units: lot.units - remaining,
            cost: lot.cost - part,
            acquired: lot.acquired,
        });
        remaining = 0;
    }
    left.extend(it);

    if remaining > 0 {
        bail!(
            "this holding is {remaining} units short of the {want} being sold — \
             `Ratio.Lots.short_sales_are_refused`, and going negative would create \
             a position nobody opened"
        );
    }
    Ok(Relieved { taken, left, cost })
}

/// The legs of a sale. `Ratio.Lots.Posting.salePostings`.
///
/// ⛔ THE GAIN LEG IS DERIVED HERE AND NEVER SUPPLIED. It is `relieved −
/// proceeds`, computed from the figures already present, so there is no second
/// opinion for the relief to disagree with. A signature taking a gain as a
/// parameter would be the drift written down as an API — and that drift is
/// SILENT, because the gain leg absorbs whatever the other two legs leave: the
/// books tie and the taxable income is wrong.
///
/// ⚠ Three legs even when the gain is zero. An implementation that omitted the
/// leg at par would be right at par and wrong the moment it was not, and the
/// omission would be invisible because a zero leg changes no total.
pub fn sale_postings(
    roles: ratio_rules::ChartRoles,
    currency: Option<&str>,
    instrument: &str,
    units: i64,
    relieved: i64,
    proceeds: i64,
) -> Result<Vec<ratio_store::PostingRecord>> {
    roles.check()?;
    // ⛔ `sub` rather than a negation inside `add` — see `Relieved::gain`.
    let gain = ratio_common::checked::sub(relieved, proceeds, "the realized gain")?;
    Ok(vec![
        ratio_store::PostingRecord {
            dim: roles.investments,
            amount: -relieved,
            currency: currency.map(str::to_string),
            instrument: Some(instrument.to_string()),
            // ⛔ The units leave with the value. A sale that moved cost without
            // moving units would leave a position with basis and no holding —
            // the husk `Ratio.Lots.Edges` is about, created by the posting
            // rather than found in the data.
            quantity: Some(-units),
        },
        ratio_store::PostingRecord {
            dim: roles.cash,
            amount: proceeds,
            currency: currency.map(str::to_string),
            instrument: None,
            quantity: None,
        },
        // Credit-normal: a GAIN is negative here. An income account holding a
        // positive number is a loss wearing a gain's name.
        ratio_store::PostingRecord {
            dim: roles.realized_gain,
            amount: gain,
            currency: currency.map(str::to_string),
            instrument: None,
            quantity: None,
        },
    ])
}

#[cfg(test)]
mod tests {

    // ── the holding, against the walk it replaces ─────────────────────────

    /// ⛔ THE ARGUMENT THAT THE FAST PATH IS THE SAME FUNCTION. `relieve_by`
    /// copies and sorts on every call and is the shape `Ratio.Lots` is written
    /// about; `Holding` maintains the order instead. Asserting they agree on
    /// every input is worth more than asserting either one against examples,
    /// because it is the DIFFERENCE between them that a performance change can
    /// introduce — and this is the same device as
    /// `the_projection_agrees_with_a_full_fold`.
    fn agree(method: Method, lots: &[Lot], want: i64) {
        let slow = relieve_by(method, lots, want);
        let mut h = Holding::new(method);
        let pushed: Result<()> = lots.iter().try_for_each(|l| h.push(l.clone()));
        let fast = pushed.and_then(|()| h.relieve(method, want));

        match (slow, fast) {
            (Ok(s), Ok(f)) => {
                assert_eq!(s.cost, f.cost, "{method:?} cost, want {want}, {lots:?}");
                assert_eq!(s.taken, f.taken, "{method:?} taken, want {want}, {lots:?}");
                let mut left = s.left.clone();
                left.sort_by_key(|l| l.seq);
                assert_eq!(left, h.lots(), "{method:?} left, want {want}, {lots:?}");
            }
            (Err(_), Err(_)) => {
                // ⛔ AND A REFUSAL CONSUMED NOTHING. Relief is a mutation now, so
                // a failure part-way through could leave the position half
                // relieved — the old pure walk could not. A sale that refuses
                // must not have eaten anything on its way out.
                let mut before = lots.to_vec();
                before.sort_by_key(|l| l.seq);
                let intact: Vec<Lot> =
                    before.iter().filter(|l| lot_is_sound(l.units, l.cost)).cloned().collect();
                assert_eq!(
                    h.lots(),
                    intact,
                    "{method:?} refused want {want} and did not put the holding back"
                );
            }
            (s, f) => panic!(
                "{method:?} want {want}: one path answered and the other did not\n \
                 slow={s:?}\n fast={f:?}\n lots={lots:?}"
            ),
        }
    }

    #[test]
    fn the_holding_gives_up_exactly_what_the_walk_would_have() {
        // Deterministic spread rather than a random one: same reason `ratio-gen`
        // has no RNG — a failure has to be reproducible from the source alone.
        let mk = |seq, units, cost, day: Option<&str>| Lot {
            seq,
            units,
            cost,
            acquired: day.map(|d| ratio_common::days_from_iso_date(d).unwrap() as Day),
        };
        let holdings: Vec<Vec<Lot>> = vec![
            vec![],
            vec![mk(1, 10, 100, Some("2024-01-01"))],
            // Equal per-unit costs — the tie the seq tiebreak has to settle.
            vec![
                mk(1, 10, 100, Some("2024-01-01")),
                mk(2, 20, 200, Some("2024-06-01")),
                mk(3, 5, 50, Some("2025-01-01")),
            ],
            // Dearest is neither first nor last.
            vec![
                mk(1, 10, 100, Some("2024-01-01")),
                mk(2, 1, 400, Some("2023-01-01")),
                mk(3, 10, 50, Some("2026-01-01")),
            ],
            // A large cheap lot against a small dear one — HIFO by TOTAL cost
            // would take the wrong one.
            vec![
                mk(1, 1000, 1000, Some("2024-01-01")),
                mk(2, 1, 40, Some("2024-02-01")),
            ],
            // No dates at all: the holding-period methods must refuse.
            vec![mk(1, 10, 100, None), mk(2, 10, 300, None)],
            // Mixed — some dated, some not.
            vec![mk(1, 10, 100, Some("2024-01-01")), mk(2, 10, 300, None)],
        ];
        let methods = [
            Method::Fifo,
            Method::Lifo,
            Method::Hifo,
            Method::Lofo,
            Method::LongestHeldFirst,
            Method::ShortestHeldFirst,
        ];
        for lots in &holdings {
            for m in methods {
                // 0 through more than the holding, so shortfalls and partial
                // splits are both exercised.
                for want in 0..=32 {
                    agree(m, lots, want);
                }
            }
        }
    }

    #[test]
    fn per_unit_is_a_total_order() {
        // ⛔ A `BTreeMap` SILENTLY MISBEHAVES ON A COMPARATOR THAT IS NOT ONE,
        // and this codebase has been bitten there twice — the HIFO tiebreak, and
        // an eviction comparator that broke transitivity. Checked over every
        // pair and triple rather than by example.
        let vals: Vec<PerUnit> = [
            (100, 10),
            (10, 1),
            (200, 20),
            (1, 1000),
            (0, 5),
            (-50, 10),
            (i64::MAX, 1),
            (i64::MIN, 1),
        ]
        .iter()
        .map(|&(cost, units)| PerUnit { cost, units })
        .collect();

        for a in &vals {
            assert_eq!(a.cmp(a), std::cmp::Ordering::Equal, "reflexive");
            for b in &vals {
                // Antisymmetric.
                assert_eq!(a.cmp(b), b.cmp(a).reverse(), "{a:?} vs {b:?}");
                for c in &vals {
                    if a <= b && b <= c {
                        assert!(a <= c, "transitivity: {a:?} <= {b:?} <= {c:?}");
                    }
                }
            }
        }
        // And equal ratios really are equal, which is what makes the seq
        // tiebreak necessary rather than decorative.
        assert_eq!(
            PerUnit { cost: 100, units: 10 },
            PerUnit { cost: 200, units: 20 }
        );
    }

    #[test]
    fn changing_the_method_reorders_the_holding_once() {
        // ⚠ The order is a term of an agreement: it changes on a promotion, not
        // on a sale. What matters is that the holding follows it when it does.
        let mut h = Holding::new(Method::Fifo);
        for (seq, cost) in [(1u64, 100i64), (2, 400), (3, 50)] {
            h.push(Lot { seq, units: 10, cost, acquired: None }).unwrap();
        }
        assert_eq!(h.relieve(Method::Fifo, 10).unwrap().cost, 100, "oldest");
        assert_eq!(h.relieve(Method::Hifo, 10).unwrap().cost, 400, "then dearest");
        assert_eq!(h.relieve(Method::Lofo, 10).unwrap().cost, 50, "then cheapest");
        assert!(h.is_empty());
    }

    #[test]
    fn a_husk_is_refused_when_it_is_offered() {
        // ⚠ Moved from the walk to `push`: checked where it can first be wrong,
        // rather than rediscovered by scanning the holding on every sale.
        let mut h = Holding::new(Method::Fifo);
        let err = h
            .push(Lot { seq: 1, units: 0, cost: 500, acquired: None })
            .unwrap_err();
        assert!(format!("{err:#}").contains("holding of nothing"), "{err:#}");
        assert!(h.is_empty(), "and it did not enter the holding");
    }
    use super::*;

    fn l(seq: u64, units: i64, cost: i64) -> Lot {
        Lot { seq, units, cost, acquired: None }
    }

    fn dated(seq: u64, units: i64, cost: i64, day: &str) -> Lot {
        Lot { seq, units, cost, acquired: Some(ratio_common::days_from_iso_date(day).unwrap() as Day) }
    }

    const ROLES: ratio_rules::ChartRoles =
        ratio_rules::ChartRoles {
        investments: 1,
        cash: 2,
        realized_gain: 30,
        currency_conversion: None,
    };

    #[test]
    fn a_sale_posts_three_legs_and_conserves() {
        // ⭐ `Ratio.Lots.Posting.a_sale_conserves_every_currency`. Sell a lot
        // that cost 100 for 150: −100 investments, +150 cash, −50 gain.
        let ps = sale_postings(ROLES, Some("USD"), "VTI", 10, 100, 150).unwrap();
        assert_eq!(ps.len(), 3);
        assert_eq!(ps.iter().map(|p| p.amount).sum::<i64>(), 0, "it conserves");
        assert_eq!(ps[2].amount, -50, "the gain, credit-normal");
        assert_eq!(ps[0].quantity, Some(-10), "the units leave with the value");

        // ⛔ Two legs would be out by exactly the gain.
        assert_eq!(ps[..2].iter().map(|p| p.amount).sum::<i64>(), 50);
    }

    #[test]
    fn a_loss_needs_no_separate_account() {
        // One formula, both directions. A system with a `realized_loss` role
        // would have to choose between them from the sign of a number it had
        // just computed.
        let ps = sale_postings(ROLES, Some("USD"), "VTI", 10, 150, 100).unwrap();
        assert_eq!(ps[2].amount, 50, "a debit, which is what a loss is");
        assert_eq!(ps.iter().map(|p| p.amount).sum::<i64>(), 0);
    }

    #[test]
    fn a_sale_at_cost_still_posts_three_legs() {
        let ps = sale_postings(ROLES, Some("USD"), "VTI", 10, 100, 100).unwrap();
        assert_eq!(ps.len(), 3);
        assert_eq!(ps[2].amount, 0, "a zero leg, deliberately present");
    }

    #[test]
    fn a_collided_chart_is_refused_before_it_posts() {
        // ⛔ `Ratio.Lots.Posting.a_collided_chart_hides_the_gain`. Investments
        // and realized gain on one dimension: the gain would net against the
        // disposal, the entry would conserve, the trial balance would tie, and
        // the taxable income would be nowhere.
        let bad = ratio_rules::ChartRoles {
            investments: 1,
            cash: 2,
            realized_gain: 1,
            currency_conversion: None,
        };
        let err = sale_postings(bad, Some("USD"), "VTI", 10, 100, 150).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("net against the disposal"), "{msg}");
    }

    #[test]
    fn every_leg_carries_the_currency_of_the_sale() {
        // `Ratio.Lots.Posting.the_gain_is_in_the_currency_of_the_sale`, and the
        // door now requires it: a gain posted without a currency beside legs
        // that have one is two conservation groups, and refuses.
        let ps = sale_postings(ROLES, Some("EUR"), "VWRL", 5, 100, 120).unwrap();
        assert!(ps.iter().all(|p| p.currency.as_deref() == Some("EUR")));
    }

    #[test]
    fn a_sale_costs_what_it_consumes() {
        // `Ratio.Lots.relief_touches_only_what_it_takes`.
        let r = relieve(&[l(1, 10, 200), l(2, 5, 50)], 10).unwrap();
        assert_eq!(r.cost, 200);
        assert_eq!(r.left, vec![l(2, 5, 50)], "and leaves the rest alone");
    }

    #[test]
    fn what_was_taken_plus_what_remains_is_what_was_there() {
        // `Ratio.Lots.cost_is_conserved` and `units_are_conserved`.
        let lots = [l(1, 10, 200), l(2, 5, 50), l(3, 8, 400)];
        for want in 0..=23i64 {
            let r = relieve(&lots, want).unwrap();
            assert_eq!(
                r.cost + r.left.iter().map(|x| x.cost).sum::<i64>(),
                650,
                "cost, at want={want}"
            );
            assert_eq!(
                r.taken.iter().map(|t| t.units).sum::<i64>() + r.left.iter().map(|x| x.units).sum::<i64>(),
                23,
                "units, at want={want}"
            );
            assert_eq!(r.taken.iter().map(|t| t.units).sum::<i64>(), want, "sold exactly {want}");
        }
    }

    #[test]
    fn a_partial_relief_is_exactly_pro_rata() {
        // `Ratio.Lots.partial_relief_is_exactly_pro_rata`. Four of ten units at
        // 200 is 80 — not 79, not 81.
        let r = relieve(&[l(1, 10, 200)], 4).unwrap();
        assert_eq!(r.cost, 80);
        assert_eq!(r.left, vec![l(1, 6, 120)]);
    }

    #[test]
    fn a_partial_relief_that_would_round_is_refused() {
        // ⛔ Conservation holds of a ROUNDED answer, so nothing downstream would
        // catch it. Three of seven units at 100 is 42.857…
        let err = relieve(&[l(1, 7, 100)], 3).unwrap_err();
        assert!(format!("{err:#}").contains("term of an administration agreement"), "{err:#}");
    }

    #[test]
    fn a_husk_is_refused_rather_than_consumed() {
        // ⛔ `Ratio.Lots.Edges.a_husk_gives_away_its_cost`. Unguarded, this
        // relieves 200 where the answer is 100 — the husk hands over its whole
        // basis for no units — and cost is conserved, units are conserved, and
        // the realized gain is understated by exactly 100.
        let err = relieve(&[l(1, 0, 100), l(2, 10, 200)], 5).unwrap_err();
        assert!(format!("{err:#}").contains("give away its basis"), "{err:#}");

        // And the sound holding alone relieves half as much.
        assert_eq!(relieve(&[l(2, 10, 200)], 5).unwrap().cost, 100);
    }

    #[test]
    fn a_fully_relieved_lot_is_not_a_husk() {
        // Nothing held and nothing owed passes, and behaves as if absent.
        let r = relieve(&[l(1, 0, 0), l(2, 10, 200)], 5).unwrap();
        assert_eq!(r.cost, 100);
    }

    #[test]
    fn fifo_is_the_sequence_not_the_vector_order() {
        // ⛔ `Ratio.Lots.Edges.sorting_is_the_callers_job` — the Lean function
        // takes the head, so this one SORTS. A projection keyed by instrument
        // does not return acquisition order, and trusting it would make a
        // fund's tax position whatever the storage layer felt like.
        let jumbled = [l(9, 1, 90), l(1, 1, 10), l(5, 1, 50)];
        let r = relieve(&jumbled, 1).unwrap();
        assert_eq!(r.taken[0].seq, 1, "the OLDEST lot");
        assert_eq!(r.cost, 10, "not 90");
    }

    #[test]
    fn the_configured_method_reaches_the_engine() {
        // ⛔ EVERY VARIANT, AND THE GAIN IT PRODUCES. A mismatch in the mapping
        // would mean a fund declaring HIFO and being relieved FIFO — the books
        // would tie, the units would be right, and only the taxable gain would
        // be wrong, which is the figure nobody reconciles.
        use ratio_rules::LotMethod as L;
        let lots = [l(1, 1, 10), l(2, 1, 40)];
        for (declared, expect) in
            [(L::Fifo, 10i64), (L::Lifo, 40), (L::Hifo, 40), (L::Lofo, 10)]
        {
            let m: Method = declared.into();
            assert_eq!(
                relieve_by(m, &lots, 1).unwrap().cost,
                expect,
                "{declared:?} did not reach the engine as itself"
            );
        }

        // And the default a fund gets when it declares nothing.
        assert_eq!(Method::from(L::default()), Method::Fifo);

        // ⛔ EVERY VARIANT MAPS, including the two that need dates. A method
        // declared in a configuration and silently dropped on the way to the
        // engine would relieve FIFO while the agreement said otherwise.
        let dated_lots = [dated(1, 1, 10, "2026-01-01"), dated(2, 1, 40, "2024-01-01")];
        for (declared, expect) in
            [(L::LongestHeldFirst, 40i64), (L::ShortestHeldFirst, 10)]
        {
            let m: Method = declared.into();
            assert!(m.needs_acquisition_dates(), "{declared:?} needs dates");
            assert_eq!(relieve_by(m, &dated_lots, 1).unwrap().cost, expect, "{declared:?}");
        }
    }

    #[test]
    fn the_method_decides_the_taxable_gain() {
        // ⭐ `Ratio.Lots.Methods.the_method_decides_the_taxable_gain`. Two
        // one-unit lots at 10 and 40, a sale of one at 50. Four times the
        // taxable income between HIFO and LOFO, and nothing on the balance
        // sheet moves.
        let lots = [l(1, 1, 10), l(2, 1, 40)];
        let g = |m| relieve_by(m, &lots, 1).unwrap().gain(50).unwrap();
        assert_eq!(g(Method::Fifo), 40, "gives up the OLD lot");
        assert_eq!(g(Method::Lifo), 10, "gives up the NEW lot");
        assert_eq!(g(Method::Hifo), 10, "gives up the DEAR lot — shelters the gain");
        assert_eq!(g(Method::Lofo), 40, "gives up the CHEAP lot — realizes it");
    }

    #[test]
    fn hifo_is_per_unit_not_per_lot() {
        // ⛔ `Ratio.Lots.Methods.hifo_is_per_unit_not_per_lot`. A lot of 100
        // units costing 1,000 is 10 each; a lot of 1 unit costing 50 is 50.
        // By TOTAL cost the first is dearer; per UNIT the second is, by five
        // times, and it is the one that shelters the most gain.
        let lots = [l(1, 100, 1_000), l(2, 1, 50)];
        let r = relieve_by(Method::Hifo, &lots, 1).unwrap();
        assert_eq!(r.cost, 50, "the small DEAR lot, not the large cheap one");
        assert_eq!(r.taken[0].seq, 2);
    }

    #[test]
    fn the_tiebreak_does_not_override_the_method() {
        // ⛔ THE BUG LEAN CAUGHT. `dearer(a,b) || a.seq <= b.seq` is true
        // whenever the sequence ascends, so the tiebreak overrode the method
        // and HIFO performed FIFO.
        //
        // ⚠ A test asserting "HIFO differs from LOFO" would have PASSED — both
        // were broken the same way in opposite directions. This asserts the
        // VALUE, which is the only thing that catches it.
        let lots = [l(1, 1, 10), l(2, 1, 40)];
        assert_eq!(relieve_by(Method::Hifo, &lots, 1).unwrap().cost, 40, "the dear lot");
        assert_eq!(relieve_by(Method::Fifo, &lots, 1).unwrap().cost, 10, "the old lot");
    }

    #[test]
    fn every_method_conserves_whatever_it_chooses() {
        // `Ratio.Lots.Methods.every_ordering_method_conserves`. The invariant a
        // new method must preserve — and the one whoever adds it will not think
        // to check, because the gain is what the method is FOR.
        let lots = [l(1, 10, 200), l(2, 5, 50), l(3, 8, 400)];
        for m in [Method::Fifo, Method::Lifo, Method::Hifo, Method::Lofo] {
            for want in 0..=23i64 {
                let r = relieve_by(m, &lots, want).unwrap();
                assert_eq!(
                    r.cost + r.left.iter().map(|x| x.cost).sum::<i64>(),
                    650,
                    "{m:?} at want={want}"
                );
                assert_eq!(r.taken.iter().map(|t| t.units).sum::<i64>(), want, "{m:?}");
            }
        }
    }

    #[test]
    fn a_holding_period_method_refuses_a_lot_with_no_date() {
        // ⛔ `Ratio.Lots.Methods.a_missing_acquisition_date_refuses`. Not
        // "assume long", not "assume short" — refuse. A tax rate is not a thing
        // to guess at from an absence, and the two fallbacks are wrong in
        // OPPOSITE directions, so neither is the conservative one.
        let lots = [dated(1, 1, 10, "2024-01-01"), l(2, 1, 40)];
        let err = relieve_by(Method::LongestHeldFirst, &lots, 1).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("no acquisition date"), "{msg}");
        assert!(msg.contains("wrong in opposite directions"), "names why: {msg}");

        // ⚠ And the methods that do NOT need dates still work on the same
        // holding — the refusal is the method's, not the data's.
        assert!(relieve_by(Method::Fifo, &lots, 1).is_ok());
        assert!(relieve_by(Method::Hifo, &lots, 1).is_ok());
    }

    #[test]
    fn holding_period_orders_by_date_not_by_sequence() {
        // `Ratio.Lots.Methods.the_period_orders_it_not_the_sequence`. Lot 2 was
        // acquired earlier despite the higher ordinal — a fund that migrated its
        // records, or one that back-loaded a position, has exactly this shape.
        let lots = [dated(1, 1, 10, "2026-01-01"), dated(2, 1, 40, "2024-01-01")];
        let long = relieve_by(Method::LongestHeldFirst, &lots, 1).unwrap();
        assert_eq!(long.taken[0].seq, 2, "the one held longest, not the first ordinal");
        let short = relieve_by(Method::ShortestHeldFirst, &lots, 1).unwrap();
        assert_eq!(short.taken[0].seq, 1);
    }

    #[test]
    fn equal_dates_fall_back_to_acquisition_order() {
        // ⚠ `Ratio.Lots.Methods.equal_periods_fall_back_to_acquisition_order`.
        // Without a stable tiebreak, two runs of the same fund could produce two
        // different tax figures from identical data.
        let lots = [dated(2, 1, 40, "2025-06-01"), dated(1, 1, 10, "2025-06-01")];
        let r = relieve_by(Method::LongestHeldFirst, &lots, 1).unwrap();
        assert_eq!(r.taken[0].seq, 1);
    }

    #[test]
    fn the_gain_is_proceeds_less_what_it_cost() {
        // `Ratio.Lots.Relief.the_gain_and_the_relieved_cost_are_the_proceeds`.
        let r = relieve(&[l(1, 1, 10), l(2, 1, 40)], 1).unwrap();
        assert_eq!(r.gain(50).unwrap(), 40, "FIFO gives up the cheap lot");
        assert_eq!(r.gain(50).unwrap() + r.cost, 50, "and the two are the proceeds");
    }

    #[test]
    fn selling_more_than_the_holding_is_refused() {
        // `Ratio.Lots.short_sales_are_refused`. Not "relieve what there is and
        // report a smaller sale" — that would silently fill part of an order.
        let err = relieve(&[l(1, 10, 200)], 11).unwrap_err();
        assert!(format!("{err:#}").contains("1 units short"), "{err:#}");
        assert!(relieve(&[l(1, 10, 200)], -1).is_err(), "and a negative sale is not a sale");
    }

    #[test]
    fn selling_everything_leaves_nothing() {
        // `Ratio.Lots.Edges.selling_everything_leaves_nothing` — an empty list,
        // not a list of husks, which the NEXT sale would have consumed.
        let r = relieve(&[l(1, 10, 200), l(2, 5, 50)], 15).unwrap();
        assert!(r.left.is_empty());
        assert_eq!(r.cost, 250);
    }

    #[test]
    fn a_sale_of_nothing_disturbs_nothing() {
        let lots = [l(1, 10, 200), l(2, 5, 50)];
        let r = relieve(&lots, 0).unwrap();
        assert!(r.taken.is_empty());
        assert_eq!(r.left, lots.to_vec());
        assert_eq!(r.gain(0).unwrap(), 0);
    }

    #[test]
    fn an_overflowing_pro_rata_split_is_refused() {
        // ⛔ `Ratio.Bounded`. `cost * want` at a large basis wraps, and the
        // wrapped product PASSES `partial_divides` — the proof cannot see it
        // because in `Int` the multiplication simply happened.
        let err = relieve(&[l(1, 1_000_000, i64::MAX / 2)], 999_999).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }
}
