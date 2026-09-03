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
/// ⚠ AND THE SPACE IS NOT ALL ORDERINGS. These sort and walk. SPECIFIC
/// IDENTIFICATION is a selection the taxpayer names, AVERAGE COST pools the
/// holding, and MINTAX ranks at a SALE PRICE — none belongs in this enum.
/// Adding average cost here as a variant is the mistake
/// `Ratio.Lots.AverageCost` exists to prevent.
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

    /// The lot this order would give up next, without giving it up.
    ///
    /// ⛔ FOR A WRITER, NOT A READER. `ratio-gen` builds a book by selling a
    /// lot and posting the basis it gave up, and it chose that lot by popping
    /// the OLDEST — hardcoded, whatever the configuration it had just written
    /// declared. A book generated `--method hifo` therefore declared HIFO and
    /// carried FIFO-computed gains, and the engine reading it back relieved
    /// HIFO, disagreed with every posted sale, and reported 242 lot breaks with
    /// 75% of the gain unclassifiable.
    ///
    /// ⚠ SELLING EXACTLY THIS LOT'S UNITS IS WHAT KEEPS RELIEF WHOLE. A partial
    /// relief divides a lot's cost pro rata and REFUSES when that does not land
    /// on a whole minor unit, so a writer that picks an arbitrary quantity
    /// generates books that break for reasons that have nothing to do with what
    /// is being demonstrated.
    pub fn peek(&self) -> Option<&Lot> {
        let next = if Self::reversed(self.order) { self.seq.back() } else { self.seq.front() };
        next.or_else(|| self.ranked.first_key_value().map(|(_, l)| l))
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

    /// The open lot with this acquisition ordinal, if the holding still has it.
    pub fn get(&self, seq: u64) -> Option<&Lot> {
        self.seq.iter().chain(self.ranked.values()).find(|l| l.seq == seq)
    }

    /// Take one open lot out by ordinal. ⛔ A `Taken` lot is already gone, so
    /// this is how `attach` refuses to write one: the search is over what
    /// remains, not a check somebody has to remember.
    fn take_seq(&mut self, seq: u64) -> Option<Lot> {
        if let Some(i) = self.seq.iter().position(|l| l.seq == seq) {
            return self.seq.remove(i);
        }
        let key = self.ranked.iter().find(|(_, l)| l.seq == seq).map(|(k, _)| *k)?;
        self.ranked.remove(&key)
    }

    /// Put an open lot back, keeping FIFO/LIFO in `seq` order and re-ranking
    /// the methods whose key includes cost or date.
    ///
    /// ⛔ NOT `push`. `push` appends; a wash write that pulled a lot from the
    /// middle and pushed it on the end would make FIFO give up the wrong
    /// remainder on the next sale. Ranked methods must re-key: attaching a
    /// deferral changes per-unit cost, and
    /// `Ratio.Lots.Wash.a_wash_write_changes_what_a_later_method_gives_up`.
    fn insert_open(&mut self, lot: Lot) -> Result<()> {
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
            None => {
                let pos = self.seq.iter().position(|l| l.seq > lot.seq).unwrap_or(self.seq.len());
                self.seq.insert(pos, lot);
            }
            Some(r) => {
                self.ranked.insert(r, lot);
            }
        }
        Ok(())
    }

    /// Attach a deferred loss to one open lot. `Ratio.Lots.Wash.attachTo`.
    ///
    /// ⛔ A WRITE TO A LOT THE ENGINE DID NOT RELIEVE. The search is over the
    /// holding — the remainder — so a lot the sale took is not a candidate.
    /// A negative `d` is refused: that would reduce basis, which is washing
    /// a gain, which `a_gain_is_never_washed` already forbids.
    ///
    /// ⚠ `acquired` IS THE US HOLDING-PERIOD TRANSFER. The replacement's
    /// acquisition date for the period becomes the original lot's, not the
    /// repurchase's. `Ratio.Lots.Wash.replacementAcquired`. `None` leaves
    /// the repurchase date alone.
    pub fn attach(&mut self, seq: u64, d: i64, acquired: Option<Day>) -> Result<()> {
        if d < 0 {
            bail!(
                "a negative deferral of {d} is not a wash — that would reduce basis, \
                 which is washing a gain, and Ratio.Lots.Wash.a_gain_is_never_washed \
                 forbids it"
            );
        }
        let mut lot = self.take_seq(seq).ok_or_else(|| {
            anyhow::anyhow!(
                "lot {seq} is not open — the wash write searches the remainder, and a \
                 lot the sale took is not a candidate"
            )
        })?;
        lot.cost = ratio_common::checked::add(lot.cost, d, "the replacement lot's basis")?;
        if let Some(day) = acquired {
            // `Ratio.Lots.Wash.replacementAcquired`: the original's date, not
            // the repurchase's. Getting this wrong moves a later disposal
            // between two tax rates and changes no total.
            lot.acquired = Some(day);
        }
        self.insert_open(lot)
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

    /// Give up `want` units under MinTax: rank at `price`, then walk.
    ///
    /// ⛔ NOT `relieve`. The ranking takes the sale PRICE, so it cannot be a
    /// `Method` and cannot live in the pre-indexed order. Each sale re-ranks.
    /// `Ratio.Lots.MinTax`. `//tla:sort_and_walk_mintax_check` is the engine
    /// that pretends otherwise.
    pub fn relieve_min_tax(
        &mut self,
        want: i64,
        price: i64,
        short_weight: i64,
        threshold: i64,
        as_of: Day,
    ) -> Result<Relief> {
        let lots = self.drain_all();
        match relieve_min_tax(&lots, want, price, short_weight, threshold, as_of) {
            Ok(r) => {
                for lot in r.left {
                    self.insert_open(lot)?;
                }
                Ok(Relief { taken: r.taken, cost: r.cost })
            }
            Err(e) => {
                for lot in lots {
                    self.insert_open(lot)?;
                }
                Err(e)
            }
        }
    }

    /// Give up `want` units under SpecID: take the named lots, in the
    /// order named.
    ///
    /// ⛔ NOT `relieve`. The walk takes the taxpayer's NAMES, so it cannot
    /// be a `Method` and cannot live in the pre-indexed order.
    /// `Ratio.Lots.SpecId`. `//tla:sort_and_walk_specid_check` is the
    /// engine that pretends otherwise.
    pub fn relieve_spec_id(&mut self, want: i64, named: &[u64]) -> Result<Relief> {
        let lots = self.drain_all();
        match relieve_spec_id(&lots, want, named) {
            Ok(r) => {
                for lot in r.left {
                    self.insert_open(lot)?;
                }
                Ok(Relief { taken: r.taken, cost: r.cost })
            }
            Err(e) => {
                for lot in lots {
                    self.insert_open(lot)?;
                }
                Err(e)
            }
        }
    }

    /// Give up `want` units under average cost: pool the holding, then
    /// slice the pool.
    ///
    /// ⛔ NOT `relieve`. Pooling is not a sort, so it cannot be a
    /// `Method` and cannot live in the pre-indexed order.
    /// `Ratio.Lots.AverageCost`. `//tla:sort_and_walk_average_cost_check`
    /// is the engine that pretends otherwise.
    pub fn relieve_average_cost(&mut self, want: i64) -> Result<Relief> {
        let lots = self.drain_all();
        match relieve_average_cost(&lots, want) {
            Ok(r) => {
                for lot in r.left {
                    self.insert_open(lot)?;
                }
                Ok(Relief { taken: r.taken, cost: r.cost })
            }
            Err(e) => {
                for lot in lots {
                    self.insert_open(lot)?;
                }
                Err(e)
            }
        }
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

/// Per-unit proceeds of a sale. `Ratio.Lots.MinTax.unitPrice`.
///
/// ⛔ REFUSES RATHER THAN ROUNDS. A sale of three units for 100 has no
/// whole-minor-unit price, and which way to round would move the ranking of
/// every lot. `a_price_that_does_not_divide_is_refused`.
pub fn unit_price(proceeds: i64, want: i64) -> Result<i64> {
    if want <= 0 {
        bail!(
            "a unit price over {want} units is not a price — a sale of nothing, or a \
             negative one, has no per-unit proceeds"
        );
    }
    if proceeds % want != 0 {
        bail!(
            "a sale of {want} units for {proceeds} does not divide into a whole-minor-unit \
             price — which way to round would move every lot's tax rank, and \
             Ratio.Lots.MinTax.a_price_that_does_not_divide_is_refused refuses rather \
             than guesses"
        );
    }
    Ok(proceeds / want)
}

/// Whether a holding period is long-term. `Ratio.Lots.Methods.isLongTerm`.
///
/// ⚠ THE THRESHOLD DAY IS LONG-TERM. Off by one here moves a lot between
/// tax rates, and the resulting figure looks entirely ordinary.
pub fn is_long_term(threshold: i64, acquired: Day, as_of: Day) -> bool {
    i64::from(as_of) - i64::from(acquired) >= threshold
}

/// Tax of giving up a lot at a per-unit sale price. `Ratio.Lots.MinTax.taxAt`.
///
/// ⛔ EVERY MULTIPLY IS CHECKED. The proof is over unbounded `Int`; this
/// runs on `i64`. Asking a wrapped number which lot costs less is asking
/// about a product that never happened.
fn tax_at(short_weight: i64, price: i64, lot: &Lot, short: bool) -> Result<i64> {
    let proceeds = ratio_common::checked::mul(price, lot.units, "min-tax proceeds at this lot")?;
    let gain = ratio_common::checked::sub(proceeds, lot.cost, "min-tax gain at this lot")?;
    if short {
        ratio_common::checked::mul(gain, short_weight, "min-tax short-term weight")
    } else {
        Ok(gain)
    }
}

/// Whether lot `a` is cheaper in tax per unit than lot `b` at this price.
///
/// ⛔ CROSS-MULTIPLIED, NEVER `tax / units`. `Ratio.Lots.MinTax.cheaperTax`.
/// Compared in `i128` so the ranking itself is not a 64-bit product.
fn cheaper_tax(
    short_weight: i64,
    price: i64,
    a: &Lot,
    a_short: bool,
    b: &Lot,
    b_short: bool,
) -> Result<bool> {
    let ta = tax_at(short_weight, price, a, a_short)?;
    let tb = tax_at(short_weight, price, b, b_short)?;
    Ok((ta as i128) * (b.units as i128) < (tb as i128) * (a.units as i128))
}

fn lot_is_short(threshold: i64, as_of: Day, lot: &Lot) -> Result<bool> {
    let Some(acquired) = lot.acquired else {
        bail!(
            "lot {} has no acquisition date, and min-tax cannot classify it — \
             assuming the epoch would make it long-term at the favorable rate on \
             records that do not support the claim, and assuming today would make it \
             short-term on a holding that may have been held for years. Neither is \
             conservative; they are wrong in opposite directions",
            lot.seq
        );
    };
    Ok(!is_long_term(threshold, acquired, as_of))
}

/// Rank a holding by tax at a sale price, cheapest-tax first.
///
/// `Ratio.Lots.MinTax.arrangeMinTax`. Missing dates refuse. Equal tax falls
/// back to acquisition order.
pub fn arrange_min_tax(
    lots: &mut [Lot],
    price: i64,
    short_weight: i64,
    threshold: i64,
    as_of: Day,
) -> Result<()> {
    for l in lots.iter() {
        let short = lot_is_short(threshold, as_of, l)?;
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
        // Fail here, not inside `sort_by`: a comparator that returns Result
        // cannot refuse, and `expect` would panic on a product that does not
        // fit.
        tax_at(short_weight, price, l, short)?;
    }
    // Insertion-style via sort_by. The comparator is the Lean `thenBySeq`
    // over `cheaperTax`: cheaper first, and ONLY a tie falls to seq.
    lots.sort_by(|a, b| {
        let a_short = lot_is_short(threshold, as_of, a).expect("dates checked");
        let b_short = lot_is_short(threshold, as_of, b).expect("dates checked");
        let a_cheaper = cheaper_tax(short_weight, price, a, a_short, b, b_short)
            .expect("tax ranking");
        let b_cheaper = cheaper_tax(short_weight, price, b, b_short, a, a_short)
            .expect("tax ranking");
        if a_cheaper {
            std::cmp::Ordering::Less
        } else if b_cheaper {
            std::cmp::Ordering::Greater
        } else {
            a.seq.cmp(&b.seq)
        }
    });
    Ok(())
}

/// Relieve under MinTax: rank at the price, then walk.
///
/// `Ratio.Lots.MinTax.relieveMinTax`. Conservation is the walk's; the
/// ranking decides the gain.
pub fn relieve_min_tax(
    lots: &[Lot],
    want: i64,
    price: i64,
    short_weight: i64,
    threshold: i64,
    as_of: Day,
) -> Result<Relieved> {
    if want < 0 {
        bail!("a relief of {want} units is not a relief; a negative sale is a purchase");
    }
    if short_weight <= 0 {
        bail!(
            "min-tax short-term weight is {short_weight}, and a non-positive weight is \
             not a weight"
        );
    }
    let mut ordered = lots.to_vec();
    arrange_min_tax(&mut ordered, price, short_weight, threshold, as_of)?;
    // The walk is FIFO over the ranked list. ⛔ NOT `Method::Fifo.arrange` —
    // that would re-sort by sequence and undo the price ranking.
    walk_ordered(ordered, want)
}

/// Relieve under SpecID: take the named lots, in the order named.
///
/// `Ratio.Lots.SpecId.relieveSpecId`. Conservation is the walk's; the
/// name decides the gain. An empty name list is SpecID elected and lots
/// unnamed — refuse, do not walk FIFO.
pub fn relieve_spec_id(lots: &[Lot], want: i64, named: &[u64]) -> Result<Relieved> {
    if want < 0 {
        bail!("a relief of {want} units is not a relief; a negative sale is a purchase");
    }
    if named.is_empty() {
        bail!(
            "specific identification is elected and no lots were named — walking FIFO \
             would relieve under a method this sale did not elect, and FIFO is a method \
             real funds elect. Ratio.Lots.SpecId.an_unnamed_selection_is_refused"
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut picked = Vec::with_capacity(named.len());
    for seq in named {
        if !seen.insert(*seq) {
            bail!(
                "lot {seq} is named twice on one sale — two instructions about one \
                 remainder. Ratio.Lots.SpecId.a_duplicate_name_is_refused"
            );
        }
        let lot = lots.iter().find(|l| l.seq == *seq).ok_or_else(|| {
            anyhow::anyhow!(
                "lot {seq} is not in this holding — a name the book does not have is \
                 not a lot the taxpayer can identify. \
                 Ratio.Lots.SpecId.an_unknown_lot_is_refused"
            )
        })?;
        picked.push(lot.clone());
    }
    // ⛔ OVERSPECIFIED: a proper prefix already covers the sale. Naming
    // lots the walk will not reach contradicts the quantity sold.
    // `selectFirst` cannot say so.
    if overspecified(&picked, want) {
        bail!(
            "this selection names more lots than the sale of {want} unit(s) will \
             reach — a client instruction that contradicts itself, not a walk. \
             Ratio.Lots.SpecId.an_overspecified_selection_is_refused"
        );
    }
    let unnamed: Vec<Lot> = lots
        .iter()
        .filter(|l| !seen.contains(&l.seq))
        .cloned()
        .collect();
    let mut r = walk_ordered(picked, want)?;
    r.left.extend(unnamed);
    Ok(r)
}

/// Per-unit pooled basis of a holding, or nothing if the figure will
/// not divide.
///
/// `Ratio.Lots.Methods.pooled`. Euclidean remainder, to match Lean's
/// `Int` — a toward-zero `%` on a negative cost would accept a product
/// the proof refused, or refuse one it accepted.
pub fn pooled_basis(lots: &[Lot]) -> Result<i64> {
    let mut units = 0i64;
    let mut cost = 0i64;
    for lot in lots {
        units = ratio_common::checked::add(units, lot.units, "the holding's units")?;
        cost = ratio_common::checked::add(cost, lot.cost, "the holding's cost")?;
    }
    if units <= 0 {
        anyhow::bail!(
            "a holding of {units} unit(s) has no pooled basis — average cost pools \
             what is held, and nothing is held. \
             Ratio.Lots.AverageCost.a_zero_unit_holding_has_no_pooled_basis"
        );
    }
    if cost.rem_euclid(units) != 0 {
        anyhow::bail!(
            "pooling {units} unit(s) costing {cost} does not divide into a whole \
             minor unit — which way to round is a term of an administration \
             agreement, not a property of arithmetic. \
             Ratio.Lots.Methods.an_average_that_does_not_divide_is_refused"
        );
    }
    Ok(cost.div_euclid(units))
}

/// The acquisition date the pool carries, if every lot agrees.
///
/// ⛔ DOES NOT INVENT A DATE. Mixed or missing dates stay unset. A
/// holding-period split of a pool is a leftover on #9, not a guess.
fn pool_acquired(lots: &[Lot]) -> Option<Day> {
    let mut dates = lots.iter().map(|l| l.acquired);
    let first = dates.next()??;
    for d in dates {
        if d != Some(first) {
            return None;
        }
    }
    Some(first)
}

/// Relieve under average cost: pool the holding, slice `want` units.
///
/// `Ratio.Lots.AverageCost.relieveAverageCost`. The remainder is one
/// pooled lot (sequence 0 — the holding, not a surviving lot). A
/// figure that will not divide is refused.
pub fn relieve_average_cost(lots: &[Lot], want: i64) -> Result<Relieved> {
    if want < 0 {
        bail!("a relief of {want} units is not a relief; a negative sale is a purchase");
    }
    if want == 0 {
        return Ok(Relieved { taken: Vec::new(), left: lots.to_vec(), cost: 0 });
    }
    let unit = pooled_basis(lots)?;
    let mut units = 0i64;
    let mut cost = 0i64;
    for lot in lots {
        units = ratio_common::checked::add(units, lot.units, "the holding's units")?;
        cost = ratio_common::checked::add(cost, lot.cost, "the holding's cost")?;
    }
    if want > units {
        anyhow::bail!(
            "the holding is short: {want} unit(s) were sold and {units} are held. \
             Ratio.Lots.AverageCost.a_sale_bigger_than_the_pool_is_refused"
        );
    }
    let taken_cost = ratio_common::checked::mul(unit, want, "the pooled basis given up")?;
    let left_units = ratio_common::checked::sub(units, want, "the pooled remainder")?;
    let left_cost = ratio_common::checked::sub(cost, taken_cost, "the pooled leftover cost")?;
    let acquired = pool_acquired(lots);
    let taken = vec![Taken {
        seq: 0,
        units: want,
        cost: taken_cost,
        acquired,
    }];
    let left = if left_units == 0 {
        Vec::new()
    } else {
        vec![Lot { seq: 0, units: left_units, cost: left_cost, acquired }]
    };
    Ok(Relieved { taken, left, cost: taken_cost })
}

/// Whether a proper prefix of the named lots already covers `want`.
///
/// `Ratio.Lots.SpecId.overspecified`. A husk (0 units) at the front does
/// not cover; a single named lot may be taken in part.
fn overspecified(picked: &[Lot], want: i64) -> bool {
    if picked.len() < 2 {
        return false;
    }
    let mut remaining = want;
    for (i, lot) in picked.iter().enumerate() {
        let last = i + 1 == picked.len();
        if last {
            return false;
        }
        if remaining <= lot.units {
            return true;
        }
        remaining -= lot.units;
    }
    false
}

/// Walk an already-ranked holding. Shared by the Method walk's shape; the
/// caller owns the order.
fn walk_ordered(ordered: Vec<Lot>, want: i64) -> Result<Relieved> {
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

/// Whether a repurchase falls inside the disallowance window of a sale.
///
/// `Ratio.Lots.Wash.inWashWindow`. ⛔ BOTH SIDES, and the window is a
/// parameter — a jurisdiction's number, never a constant in the arithmetic.
/// `the_window_reaches_backwards_too`, `the_window_is_a_jurisdiction_number`.
pub fn in_wash_window(window: i64, sale_day: Day, buy_day: Day) -> bool {
    let delta = i64::from(buy_day) - i64::from(sale_day);
    -window <= delta && delta <= window
}

/// A citeable identity of a struck figure: the journal prefix it folded.
///
/// ⛔ NOT THE FIGURE. Two strikes can report the same number from different
/// prefixes; the identity is which prefix was read. Rewriting the number
/// while keeping this id is the silent defect.
/// `Ratio.Lots.WashRestatement`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StrikeId {
    pub prefix: u64,
}

/// A realized gain that was struck — the number somebody was paid on.
///
/// `qualified` is written at strike time: the wash window was still open,
/// so this figure can still move. Adding the flag later is a restatement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StruckGain {
    pub id: StrikeId,
    pub sold_on: Day,
    pub figure: i64,
    pub qualified: bool,
}

/// A restatement: a new record that cites the strike it supersedes.
///
/// ⭐ `Ratio.Period` forbids a second value occupying the same day. This is
/// that new kind of thing, for the one rule that genuinely reaches
/// backwards. Putting `moved_to` on the strike is [`rewrite_in_place`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Restatement {
    pub cites: StrikeId,
    pub original: i64,
    pub moved_to: i64,
}

/// Whether a wash window is still open on this day.
///
/// ⭐ WRITTEN AT STRIKE TIME. A sale on day 100 with a 30-day window is
/// still open on day 130 and closed on day 131. Flagging a closed window
/// trains a reader to ignore the flag.
/// `Ratio.Lots.WashRestatement.a_closed_window_is_not_qualified`.
///
/// ⛔ THE SUM IS CHECKED. Lean's `day ≤ soldOn + window` is over `Int`;
/// a wrapped close would qualify a strike whose window had closed, or
/// refuse one whose window was open.
pub fn window_still_open(window: i64, sold_on: Day, day: Day) -> Result<bool> {
    let close = ratio_common::checked::add(
        i64::from(sold_on),
        window,
        "the day the wash window closes",
    )?;
    Ok(i64::from(day) <= close)
}

/// Strike a realized gain. Qualifies iff the window is still open.
///
/// `Ratio.Lots.WashRestatement.strikeGain`.
pub fn strike_gain(
    id: StrikeId,
    window: i64,
    sold_on: Day,
    day: Day,
    figure: i64,
) -> Result<StruckGain> {
    Ok(StruckGain {
        id,
        sold_on,
        figure,
        qualified: window_still_open(window, sold_on, day)?,
    })
}

/// A later repurchase that washes a struck sale.
///
/// Returns a restatement citing the strike, or `None` if the repurchase
/// does not move this figure. Never returns a mutated [`StruckGain`].
/// `Ratio.Lots.WashRestatement.restate`.
pub fn restate(s: &StruckGain, window: i64, buy_day: Day, new_figure: i64) -> Option<Restatement> {
    if in_wash_window(window, s.sold_on, buy_day) && new_figure != s.figure {
        Some(Restatement {
            cites: s.id.clone(),
            original: s.figure,
            moved_to: new_figure,
        })
    } else {
        None
    }
}

/// The forbidden operation: overwrite the struck figure and keep the id.
///
/// Defined so tests can name it. An engine that "updated the strike" is
/// this, not [`restate`].
/// `Ratio.Lots.WashRestatement.rewriteInPlace`.
pub fn rewrite_in_place(s: StruckGain, new_figure: i64) -> StruckGain {
    StruckGain {
        figure: new_figure,
        ..s
    }
}

/// Whether the record said the figure can move, or said it did.
///
/// The third case — struck clean, changed quietly — is `false`.
/// `Ratio.Lots.WashRestatement.saysSo`.
pub fn says_so(s: &StruckGain, r: Option<&Restatement>) -> bool {
    s.qualified || r.is_some()
}

/// The disallowed portion of a loss, as a POSITIVE magnitude.
///
/// `Ratio.Lots.Wash.disallowed`. `loss` is signed the way [`Relief::gain`]
/// is: negative when money was lost. A gain is never washed. The match is
/// capped at what was sold. A split that will not divide is refused rather
/// than rounded — `partial_relief_is_exactly_pro_rata`'s discipline.
pub fn disallowed(loss: i64, sold_units: i64, bought_units: i64) -> Result<i64> {
    if loss >= 0 {
        return Ok(0);
    }
    if sold_units <= 0 {
        bail!(
            "washing a sale of {sold_units} units is not a wash — there is nothing \
             to match against"
        );
    }
    let matched = bought_units.min(sold_units);
    if matched <= 0 {
        return Ok(0);
    }
    // ⛔ NEGATE THEN MULTIPLY, EACH CHECKED. `-loss * matched` as a bare
    // expression wraps before anything looks, and the wrapped product is
    // what a remainder test would then bless. `Ratio.Bounded`.
    let magnitude = ratio_common::checked::neg(loss, "the loss being washed")?;
    let product = ratio_common::checked::mul(magnitude, matched, "a pro-rata wash split")?;
    if product.rem_euclid(sold_units) != 0 {
        bail!(
            "washing {matched} of {sold_units} sold units does not divide the {magnitude} \
             loss into whole minor units — which way to round is a term of an \
             administration agreement, not a property of arithmetic"
        );
    }
    Ok(product.div_euclid(sold_units))
}

/// What the sale recognizes once the disallowance is applied.
///
/// `Ratio.Lots.Wash.recognizedNow`. `loss` is negative and `d` is the
/// positive amount deferred, so this moves the figure towards zero.
pub fn recognized_now(loss: i64, d: i64) -> Result<i64> {
    ratio_common::checked::add(loss, d, "the recognized loss after wash")
}

/// The replacement lot's basis, with the deferred loss attached.
///
/// `Ratio.Lots.Wash.replacementBasis`.
pub fn replacement_basis(cost: i64, d: i64) -> Result<i64> {
    ratio_common::checked::add(cost, d, "the replacement lot's basis")
}

/// The replacement's acquisition date for holding-period purposes: the
/// original lot's, not the repurchase's.
///
/// `Ratio.Lots.Wash.replacementAcquired`. The US rule. Getting this wrong
/// changes no total and moves a disposal between two tax rates.
pub fn replacement_acquired(original_acquired: Option<Day>, _repurchased_on: Option<Day>) -> Option<Day> {
    original_acquired
}

/// The replacement's acquisition date under the elected holding-period rule.
///
/// `keep` is the non-US variant: the repurchase's own date.
/// Otherwise the US transfer already named as [`replacement_acquired`].
/// `Ratio.Lots.WashHolding.acquiredFor`.
///
/// ⛔ CHOOSING THE WRONG RULE FLIPS THE RATE. Same units, same basis,
/// same proceeds. Conservation cannot see it.
pub fn acquired_for(
    keep: bool,
    original_acquired: Option<Day>,
    repurchase_on: Option<Day>,
) -> Option<Day> {
    if keep {
        repurchase_on
    } else {
        replacement_acquired(original_acquired, repurchase_on)
    }
}

/// What [`Holding::attach`] is handed for the date write.
///
/// `keep` leaves the repurchase's own date (`None` — attach does not
/// overwrite). Otherwise the original's date is written. This is not
/// unset: the election was already read. `Ratio.Lots.WashHolding`.
pub fn acquired_write(keep: bool, original_acquired: Option<Day>) -> Option<Day> {
    if keep {
        None
    } else {
        original_acquired
    }
}

/// Attach a deferred loss to one open lot in a remainder list.
///
/// `Ratio.Lots.Wash.attachTo`. ⛔ SEARCHES THE LIST IT WAS HANDED, which is
/// the remainder after relief. A lot the sale took is not in it, so writing
/// a `Taken` lot is unrepresentable rather than a check.
pub fn attach_to(lots: &[Lot], seq: u64, d: i64) -> Result<Vec<Lot>> {
    if d < 0 {
        bail!(
            "a negative deferral of {d} is not a wash — that would reduce basis, \
             which is washing a gain, and Ratio.Lots.Wash.a_gain_is_never_washed \
             forbids it"
        );
    }
    let mut found = false;
    let mut out = Vec::with_capacity(lots.len());
    for l in lots {
        if l.seq == seq {
            found = true;
            out.push(Lot {
                seq: l.seq,
                units: l.units,
                cost: replacement_basis(l.cost, d)?,
                acquired: l.acquired,
            });
        } else {
            out.push(l.clone());
        }
    }
    if !found {
        bail!(
            "lot {seq} is not open — the wash write searches the remainder, and a \
             lot the sale took is not a candidate"
        );
    }
    Ok(out)
}

/// What matching a sale against open replacements produced.
///
/// `remaining_*` is what no open lot could take — a later repurchase inside
/// the window still can. `//tla:wash_engine_check` is the sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WashMatch {
    /// Total deferral written onto open replacements.
    pub attached: i64,
    /// Sold units not yet matched to a replacement.
    pub remaining_units: i64,
    /// The unmatched loss, still signed as [`Relief::gain`].
    pub remaining_loss: i64,
}

/// Plan, then write, a wash against the lots still open in `held`.
///
/// ⛔ THE HOLDING IS THE REMAINDER. Lots the sale just took are gone, so
/// attaching to one of them refuses. Candidates are open lots whose
/// acquisition day sits inside `window` either side of `sale_day`.
///
/// ⚠ MATCHES OLDEST REPLACEMENT FIRST, and attaches each lot's share of
/// the disallowance. A split that will not divide refuses before any write
/// lands — the same all-or-nothing as a refused relief.
pub fn wash_open(
    held: &mut Holding,
    loss: i64,
    sold_units: i64,
    sale_day: Day,
    window: i64,
    original_acquired: Option<Day>,
    taken: &[Taken],
) -> Result<WashMatch> {
    if loss >= 0 || sold_units <= 0 {
        return Ok(WashMatch { attached: 0, remaining_units: sold_units.max(0), remaining_loss: loss });
    }

    // ⛔ A PARTIAL REMAINDER OF A SOLD LOT IS NOT A REPURCHASE. It is still
    // open, still in the window if the lot was bought recently, and attaching
    // to it would write the deferral onto shares the sale did not replace.
    // `attaching_cannot_write_a_lot_the_sale_took` — the seq the sale took
    // is not a candidate, even when some of it remains.
    let taken_seqs: std::collections::BTreeSet<u64> = taken.iter().map(|t| t.seq).collect();
    let mut candidates: Vec<(u64, i64)> = held
        .lots()
        .into_iter()
        .filter(|l| !taken_seqs.contains(&l.seq))
        .filter(|l| l.acquired.is_some_and(|b| in_wash_window(window, sale_day, b)))
        .map(|l| (l.seq, l.units))
        .collect();
    // Oldest replacement first — a stable match, not whatever the method
    // would give up next. The method already decided which lots were sold.
    candidates.sort_by_key(|(seq, _)| *seq);

    let mut remaining_sold = sold_units;
    let mut remaining_loss = loss;
    let mut plan: Vec<(u64, i64)> = Vec::new();
    for (seq, units) in candidates {
        if remaining_sold <= 0 || remaining_loss >= 0 {
            break;
        }
        let bought = units.min(remaining_sold);
        let d = disallowed(remaining_loss, remaining_sold, bought)?;
        if d > 0 {
            plan.push((seq, d));
            remaining_loss = recognized_now(remaining_loss, d)?;
        }
        remaining_sold -= bought;
    }

    let mut attached = 0i64;
    for (seq, d) in plan {
        held.attach(seq, d, original_acquired)?;
        attached = ratio_common::checked::add(attached, d, "the attached deferral")?;
    }
    Ok(WashMatch { attached, remaining_units: remaining_sold, remaining_loss })
}

/// Match one newly opened lot against a leftover wash from an earlier sale.
///
/// The forward half of the window: the sale happened, no replacement was
/// open, and this purchase landed inside it. Same arithmetic as
/// [`wash_open`]; the write still has to land on an open lot.
pub fn wash_purchase(
    held: &mut Holding,
    replacement_seq: u64,
    bought_units: i64,
    remaining_loss: i64,
    remaining_sold: i64,
    original_acquired: Option<Day>,
) -> Result<(i64, i64, i64)> {
    if remaining_loss >= 0 || remaining_sold <= 0 || bought_units <= 0 {
        return Ok((0, remaining_sold.max(0), remaining_loss));
    }
    let bought = bought_units.min(remaining_sold);
    let d = disallowed(remaining_loss, remaining_sold, bought)?;
    if d > 0 {
        held.attach(replacement_seq, d, original_acquired)?;
    }
    Ok((d, remaining_sold - bought, recognized_now(remaining_loss, d)?))
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

    // ── wash sales — `Ratio.Lots.Wash` ────────────────────────────────────

    #[test]
    fn the_window_reaches_backwards_too() {
        // ⛔ A FORWARD-ONLY ENGINE IS GREEN ON EVERY OTHER THEOREM. A
        // repurchase five days BEFORE the sale is inside a thirty-day window.
        assert!(in_wash_window(30, 100, 95));
        assert!(in_wash_window(30, 100, 105));
        assert!(!in_wash_window(30, 100, 69));
        assert!(!in_wash_window(30, 100, 131));
        // The threshold is configuration: same two dates, two windows.
        assert!(in_wash_window(30, 0, 25));
        assert!(!in_wash_window(10, 0, 25));
    }

    #[test]
    fn a_gain_is_never_washed() {
        assert_eq!(disallowed(40, 100, 100).unwrap(), 0);
        assert_eq!(disallowed(40, 1, 1_000).unwrap(), 0);
    }

    #[test]
    fn repurchasing_everything_defers_the_whole_loss() {
        assert_eq!(disallowed(-1000, 100, 100).unwrap(), 1000);
    }

    #[test]
    fn repurchasing_more_than_was_sold_defers_no_more() {
        assert_eq!(disallowed(-1000, 100, 250).unwrap(), 1000);
    }

    #[test]
    fn repurchasing_part_defers_exactly_that_part() {
        // Forty of a hundred: 400 of the 1000 loss is deferred.
        assert_eq!(disallowed(-1000, 100, 40).unwrap(), 400);
    }

    #[test]
    fn a_wash_split_that_does_not_divide_is_refused() {
        let err = disallowed(-1000, 3, 1).unwrap_err();
        assert!(format!("{err:#}").contains("administration agreement"), "{err:#}");
    }

    #[test]
    fn the_wash_rule_moves_a_loss_it_does_not_remove_it() {
        // ⭐ Recognize the reduced loss now, sell the replacement later against
        // its adjusted basis: the two disposals sum to the unwashed total.
        let loss = -1000i64;
        let d = 1000i64;
        let replacement_cost = 5000i64;
        let later_proceeds = 5000i64;
        let now = recognized_now(loss, d).unwrap();
        let later = later_proceeds - replacement_basis(replacement_cost, d).unwrap();
        assert_eq!(now + later, loss + (later_proceeds - replacement_cost));
        // And the first-half-only engine destroys the loss.
        assert_ne!(now + (later_proceeds - replacement_cost), loss + (later_proceeds - replacement_cost));
    }

    #[test]
    fn attaching_cannot_write_a_lot_the_sale_took() {
        // ⛔ THE SEARCH IS OVER THE REMAINDER. After relieving seq 1, attaching
        // to 1 refuses; attaching to the open replacement writes.
        let r = relieve(&[l(1, 1, 10), l(2, 1, 40)], 1).unwrap();
        let err = attach_to(&r.left, 1, 100).unwrap_err();
        assert!(format!("{err:#}").contains("not open"), "{err:#}");
        assert_eq!(attach_to(&r.left, 2, 100).unwrap(), vec![l(2, 1, 140)]);

        let mut h = Holding::new(Method::Fifo);
        h.push(l(1, 1, 10)).unwrap();
        h.push(l(2, 1, 40)).unwrap();
        h.relieve(Method::Fifo, 1).unwrap();
        let err = h.attach(1, 100, None).unwrap_err();
        assert!(format!("{err:#}").contains("not open"), "{err:#}");
        h.attach(2, 100, None).unwrap();
        assert_eq!(h.get(2).unwrap().cost, 140);
    }

    #[test]
    fn a_negative_deferral_is_refused() {
        assert!(attach_to(&[l(1, 1, 10)], 1, -1).is_err());
        let mut h = Holding::new(Method::Fifo);
        h.push(l(1, 1, 10)).unwrap();
        let err = h.attach(1, -1, None).unwrap_err();
        assert!(format!("{err:#}").contains("washing a gain"), "{err:#}");
        assert_eq!(h.lots()[0].cost, 10, "and it did not write");
    }

    #[test]
    fn a_later_sale_of_the_replacement_takes_the_adjusted_basis() {
        let held = attach_to(&[l(2, 1, 40)], 2, 1000).unwrap();
        let r = relieve(&held, 1).unwrap();
        assert_eq!(r.cost, 1040);
    }

    #[test]
    fn a_wash_write_changes_what_a_later_method_gives_up() {
        // ⛔ THE OTHER DIRECTION OF `the_method_decides_whether_there_is_a_
        // loss_to_wash`. After attaching 20, HIFO picks the replacement it
        // previously ignored.
        let lots = [l(1, 1, 25), l(2, 1, 10)];
        assert_eq!(relieve_by(Method::Hifo, &lots, 1).unwrap().cost, 25);
        let washed = attach_to(&lots, 2, 20).unwrap();
        assert_eq!(relieve_by(Method::Hifo, &washed, 1).unwrap().cost, 30);

        // And the holding re-ranks: same numbers, mutating the open lot.
        let mut h = Holding::new(Method::Hifo);
        h.push(l(1, 1, 25)).unwrap();
        h.push(l(2, 1, 10)).unwrap();
        h.attach(2, 20, None).unwrap();
        assert_eq!(h.relieve(Method::Hifo, 1).unwrap().cost, 30);
    }

    #[test]
    fn the_method_decides_whether_there_is_a_loss_to_wash() {
        // Same holding, same trade at 20, same repurchase of 1. FIFO gives up
        // the cheap lot and realizes a GAIN — nothing to wash. LIFO gives up
        // the dear one and realizes a LOSS the repurchase defers entirely.
        let lots = [l(1, 1, 10), l(2, 1, 40)];
        let fifo = relieve_by(Method::Fifo, &lots, 1).unwrap();
        let lifo = relieve_by(Method::Lifo, &lots, 1).unwrap();
        assert_eq!(disallowed(fifo.gain(20).unwrap(), 1, 1).unwrap(), 0);
        assert_eq!(disallowed(lifo.gain(20).unwrap(), 1, 1).unwrap(), 20);
    }

    #[test]
    fn the_transferred_period_decides_the_rate() {
        // Acquired day 0, washed by a repurchase on day 300, disposed day 400.
        // From the original: 400 days, long-term. From the repurchase: 100,
        // short. Same units, same basis, same proceeds, different rate.
        let transferred = replacement_acquired(Some(0), Some(300));
        assert_eq!(transferred, Some(0));
        assert!(400 - transferred.unwrap() as i64 >= 365);
        assert!(400 - 300 < 365);
    }

    #[test]
    fn choosing_the_wrong_rule_flips_the_rate() {
        // `Ratio.Lots.WashHolding.choosing_the_wrong_rule_flips_the_rate`.
        // Transfer: 400 days, long. Keep: 100 days, short. Same basis.
        let transfer = acquired_for(false, Some(0), Some(300)).unwrap();
        let keep = acquired_for(true, Some(0), Some(300)).unwrap();
        assert_eq!(transfer, 0);
        assert_eq!(keep, 300);
        assert!(is_long_term(365, transfer, 400));
        assert!(!is_long_term(365, keep, 400));
        assert_eq!(replacement_basis(500, 1000).unwrap(), 1500);
    }

    #[test]
    fn assuming_us_transfer_when_the_election_is_keep_is_the_wrong_rate() {
        // ⛔ THE NAMED DEFECT. replacementAcquired on a keep book classifies
        // long; the elected date classifies short. The books still tie.
        let usurped = replacement_acquired(Some(0), Some(300)).unwrap();
        let elected = acquired_for(true, Some(0), Some(300)).unwrap();
        assert!(is_long_term(365, usurped, 400));
        assert!(!is_long_term(365, elected, 400));
    }

    #[test]
    fn a_keep_write_leaves_the_repurchase_date() {
        // Attach with None does not overwrite. That is keep, not unset —
        // the election was already read. `acquired_write(true, …)` is None.
        assert_eq!(acquired_write(true, Some(0)), None);
        assert_eq!(acquired_write(false, Some(0)), Some(0));

        let mut h = Holding::new(Method::Fifo);
        h.push(dated(1, 100, 2000, "2026-01-01")).unwrap();
        h.push(dated(2, 40, 500, "2026-06-10")).unwrap();
        let taken = h.relieve(Method::Fifo, 100).unwrap();
        let loss = taken.gain(1000).unwrap();
        let sale = ratio_common::days_from_iso_date("2026-06-15").unwrap() as Day;
        let write = acquired_write(true, taken.taken[0].acquired);
        let w = wash_open(&mut h, loss, 100, sale, 30, write, &taken.taken).unwrap();
        assert_eq!(w.attached, 400);
        assert_eq!(h.get(2).unwrap().cost, 900, "the write is still the write");
        assert_eq!(
            h.get(2).unwrap().acquired,
            ratio_common::days_from_iso_date("2026-06-10").ok().map(|d| d as Day),
            "keep leaves the repurchase's own date"
        );
        assert_eq!(h.relieve(Method::Fifo, 40).unwrap().cost, 900);
    }

    #[test]
    fn an_in_window_repurchase_raises_the_replacement_basis() {
        // Buy the replacement five days before the sale (the harvest shape).
        let mut h = Holding::new(Method::Fifo);
        h.push(dated(1, 100, 2000, "2026-01-01")).unwrap();
        h.push(dated(2, 40, 500, "2026-06-10")).unwrap();
        let taken = h.relieve(Method::Fifo, 100).unwrap();
        assert_eq!(taken.cost, 2000);
        let loss = taken.gain(1000).unwrap(); // −1000
        let sale = ratio_common::days_from_iso_date("2026-06-15").unwrap() as Day;
        let w = wash_open(&mut h, loss, 100, sale, 30, taken.taken[0].acquired, &taken.taken).unwrap();
        assert_eq!(w.attached, 400, "forty of a hundred, pro rata");
        assert_eq!(h.get(2).unwrap().cost, 900, "500 + 400");
        assert_eq!(
            h.get(2).unwrap().acquired,
            taken.taken[0].acquired,
            "the period transfers with the basis"
        );
        // And a later relief of the replacement takes the adjusted basis.
        assert_eq!(h.relieve(Method::Fifo, 40).unwrap().cost, 900);
    }

    #[test]
    fn an_out_of_window_repurchase_does_not_raise_the_replacement_basis() {
        let mut h = Holding::new(Method::Fifo);
        h.push(dated(1, 100, 2000, "2026-01-01")).unwrap();
        h.push(dated(2, 40, 500, "2026-04-01")).unwrap(); // well before
        let taken = h.relieve(Method::Fifo, 100).unwrap();
        let loss = taken.gain(1000).unwrap();
        let sale = ratio_common::days_from_iso_date("2026-06-15").unwrap() as Day;
        let w = wash_open(&mut h, loss, 100, sale, 30, taken.taken[0].acquired, &taken.taken).unwrap();
        assert_eq!(w.attached, 0);
        assert_eq!(h.get(2).unwrap().cost, 500, "untouched");
        assert_eq!(h.get(2).unwrap().acquired, dated(2, 40, 500, "2026-04-01").acquired);
    }

    #[test]
    fn a_partial_remainder_of_the_sold_lot_is_not_a_replacement() {
        // Buy 100 and sell 40 inside the window. The 60 left were never
        // repurchased — they are the same lot the sale took.
        let mut h = Holding::new(Method::Fifo);
        h.push(dated(1, 100, 2000, "2026-06-01")).unwrap();
        let taken = h.relieve(Method::Fifo, 40).unwrap();
        assert_eq!(taken.cost, 800);
        let loss = taken.gain(200).unwrap();
        let sale = ratio_common::days_from_iso_date("2026-06-15").unwrap() as Day;
        let w = wash_open(&mut h, loss, 40, sale, 30, taken.taken[0].acquired, &taken.taken)
            .unwrap();
        assert_eq!(w.attached, 0);
        assert_eq!(h.get(1).unwrap().cost, 1200, "the remainder is untouched");
    }

    #[test]
    fn a_forward_repurchase_attaches_when_the_replacement_opens() {
        // Sale first, then the buy — the natural implementation, and the
        // other half of the window.
        let mut h = Holding::new(Method::Fifo);
        h.push(dated(1, 100, 2000, "2026-01-01")).unwrap();
        let taken = h.relieve(Method::Fifo, 100).unwrap();
        let loss = taken.gain(1000).unwrap();
        h.push(dated(2, 40, 500, "2026-06-20")).unwrap();
        let (d, remaining_units, remaining_loss) =
            wash_purchase(&mut h, 2, 40, loss, 100, taken.taken[0].acquired).unwrap();
        assert_eq!(d, 400);
        assert_eq!(remaining_units, 60);
        assert_eq!(remaining_loss, -600);
        assert_eq!(h.get(2).unwrap().cost, 900);
    }

    #[test]
    fn an_overflowing_wash_split_is_refused() {
        let err = disallowed(i64::MIN, 2, 1).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }

    // ── wash restatement — `Ratio.Lots.WashRestatement` ───────────────────

    fn struck(prefix: u64, sold_on: Day, day: Day, figure: i64) -> StruckGain {
        strike_gain(StrikeId { prefix }, 30, sold_on, day, figure).unwrap()
    }

    #[test]
    fn a_closed_window_is_not_qualified() {
        // `Ratio.Lots.WashRestatement.a_closed_window_is_not_qualified`.
        assert!(!struck(7, 100, 131, -1000).qualified);
        assert!(struck(7, 100, 105, -1000).qualified);
        assert!(window_still_open(30, 100, 130).unwrap());
        assert!(!window_still_open(30, 100, 131).unwrap());
    }

    #[test]
    fn restatement_cites_the_strike_it_supersedes() {
        // ⭐ THE STRIKE STILL SAYS −1000. The restatement names prefix 7
        // and the original number. `Ratio.Lots.WashRestatement.
        // restatement_cites_the_strike_it_supersedes`.
        let s = struck(7, 100, 105, -1000);
        let r = restate(&s, 30, 110, -600).expect("an in-window move restates");
        assert_eq!(s.figure, -1000, "the strike was not rewritten");
        assert_eq!(r.cites, StrikeId { prefix: 7 });
        assert_eq!(r.original, -1000);
        assert_eq!(r.moved_to, -600);
    }

    #[test]
    fn a_wash_that_does_not_change_the_figure_does_not_restate() {
        let s = struck(7, 100, 105, -1000);
        assert!(restate(&s, 30, 110, -1000).is_none());
        assert!(restate(&s, 30, 200, -600).is_none(), "outside the window");
    }

    #[test]
    fn a_silent_strike_that_was_restated_says_so() {
        let s = StruckGain {
            id: StrikeId { prefix: 7 },
            sold_on: 100,
            figure: -1000,
            qualified: false,
        };
        let r = restate(&s, 30, 110, -600);
        assert!(says_so(&s, r.as_ref()));
        assert!(says_so(&struck(7, 100, 105, -1000), None));
        assert!(!says_so(&s, None), "the third case: struck clean, nothing said");
    }

    #[test]
    fn rewriting_in_place_keeps_the_id_and_changes_the_figure() {
        // ⛔ THE DEFECT, NAMED. Prefix 7 still cites; the number is now
        // −600; nothing is qualified.
        // `Ratio.Lots.WashRestatement.rewriting_in_place_keeps_the_id_
        // and_changes_the_figure`.
        let s = StruckGain {
            id: StrikeId { prefix: 7 },
            sold_on: 100,
            figure: -1000,
            qualified: false,
        };
        let s = rewrite_in_place(s, -600);
        assert_eq!(s.id, StrikeId { prefix: 7 });
        assert_eq!(s.figure, -600);
        assert!(!s.qualified);
        assert_ne!(
            s,
            StruckGain {
                id: StrikeId { prefix: 7 },
                sold_on: 100,
                figure: -1000,
                qualified: false,
            }
        );
    }

    #[test]
    fn restatement_and_rewrite_are_not_the_same_operation() {
        let s = StruckGain {
            id: StrikeId { prefix: 7 },
            sold_on: 100,
            figure: -1000,
            qualified: false,
        };
        assert_eq!(
            restate(&s, 30, 110, -600),
            Some(Restatement {
                cites: StrikeId { prefix: 7 },
                original: -1000,
                moved_to: -600,
            })
        );
        assert_eq!(
            rewrite_in_place(s, -600),
            StruckGain {
                id: StrikeId { prefix: 7 },
                sold_on: 100,
                figure: -600,
                qualified: false,
            }
        );
    }

    #[test]
    fn an_overflowing_window_close_is_refused() {
        // ⛔ `Ratio.Bounded`. soldOn + window at the i64 edge wraps, and a
        // wrapped close would qualify a strike whose window had closed.
        let err = window_still_open(i64::MAX, 1, 0).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }

    // ── min-tax — `Ratio.Lots.MinTax` ─────────────────────────────────────

    fn close_bases() -> [Lot; 2] {
        // A short (basis 10), B long (basis 12). Threshold 365.
        [
            dated(1, 1, 10, "2025-10-01"),
            dated(2, 1, 12, "2023-01-01"),
        ]
    }

    const AS_OF: &str = "2026-01-01";

    fn as_of() -> Day {
        ratio_common::days_from_iso_date(AS_OF).unwrap() as Day
    }

    #[test]
    fn a_price_that_does_not_divide_is_refused() {
        let err = unit_price(100, 3).unwrap_err();
        assert!(format!("{err:#}").contains("does not divide"), "{err:#}");
        assert_eq!(unit_price(150, 3).unwrap(), 50);
    }

    #[test]
    fn mintax_takes_different_lots_at_the_two_prices() {
        // ⭐ `Ratio.Lots.MinTax.mintax_takes_different_lots_at_the_two_prices`.
        let lots = close_bases();
        let at_50 = relieve_min_tax(&lots, 1, 50, 2, 365, as_of()).unwrap();
        let at_5 = relieve_min_tax(&lots, 1, 5, 2, 365, as_of()).unwrap();
        assert_eq!(at_50.cost, 12, "at a gain the long lot costs less");
        assert_eq!(at_50.taken[0].seq, 2);
        assert_eq!(at_5.cost, 10, "at a loss the short lot is worth more");
        assert_eq!(at_5.taken[0].seq, 1);
    }

    #[test]
    fn far_bases_do_not_flip() {
        // ⚠ `Ratio.Lots.MinTax.far_bases_do_not_flip`. Basis 40 on the long
        // lot wins at both prices; a test that used this holding would stay
        // green on an engine that never saw the price.
        let lots = [
            dated(1, 1, 10, "2025-10-01"),
            dated(2, 1, 40, "2023-01-01"),
        ];
        assert_eq!(relieve_min_tax(&lots, 1, 50, 2, 365, as_of()).unwrap().taken[0].seq, 2);
        assert_eq!(relieve_min_tax(&lots, 1, 5, 2, 365, as_of()).unwrap().taken[0].seq, 2);
    }

    #[test]
    fn no_ordering_reproduces_both_mintax_answers() {
        // `Ratio.Lots.MinTax.no_ordering_reproduces_both_mintax_answers`.
        // Each Order produces one basis from this holding. MinTax produces
        // two. FIFO/LOFO give 10; LIFO/HIFO give 12. None give both.
        let lots = close_bases();
        let mintax_gain = relieve_min_tax(&lots, 1, 50, 2, 365, as_of()).unwrap().cost;
        let mintax_loss = relieve_min_tax(&lots, 1, 5, 2, 365, as_of()).unwrap().cost;
        assert_eq!(mintax_gain, 12);
        assert_eq!(mintax_loss, 10);
        for m in [Method::Fifo, Method::Lifo, Method::Hifo, Method::Lofo] {
            let cost = relieve_by(m, &lots, 1).unwrap().cost;
            assert!(
                cost != mintax_gain || cost != mintax_loss,
                "{m:?} reproduced both MinTax answers from lots alone"
            );
        }
    }

    #[test]
    fn preferring_long_term_is_not_minimising_tax() {
        let lots = close_bases();
        let long = relieve_by(Method::LongestHeldFirst, &lots, 1).unwrap();
        let min_at_5 = relieve_min_tax(&lots, 1, 5, 2, 365, as_of()).unwrap();
        assert_eq!(long.taken[0].seq, 2, "prefer-long always takes B");
        assert_eq!(min_at_5.taken[0].seq, 1, "min-tax at 5 takes A");
    }

    #[test]
    fn a_missing_acquisition_date_refuses_mintax() {
        let lots = [l(1, 1, 10)];
        let err = relieve_min_tax(&lots, 1, 50, 2, 365, as_of()).unwrap_err();
        assert!(format!("{err:#}").contains("no acquisition date"), "{err:#}");
    }

    #[test]
    fn mintax_partial_relief_is_exactly_pro_rata() {
        let lots = [dated(1, 7, 100, "2023-01-01")];
        let err = relieve_min_tax(&lots, 3, 50, 2, 365, as_of()).unwrap_err();
        assert!(format!("{err:#}").contains("does not divide"), "{err:#}");
    }

    #[test]
    fn equal_tax_falls_back_to_acquisition_order() {
        let lots = [
            dated(2, 1, 10, "2025-10-01"),
            dated(1, 1, 10, "2025-10-01"),
        ];
        let r = relieve_min_tax(&lots, 1, 50, 2, 365, as_of()).unwrap();
        assert_eq!(r.taken[0].seq, 1);
    }

    #[test]
    fn mintax_is_not_a_method_variant() {
        // ⛔ THE TRAP. `Method` has no MinTax. Compiles only if it stays that
        // way — a variant would make this match exhaustive and this test
        // would have to name it.
        match Method::Fifo {
            Method::Fifo
            | Method::Lifo
            | Method::Hifo
            | Method::Lofo
            | Method::LongestHeldFirst
            | Method::ShortestHeldFirst => {}
        }
    }

    #[test]
    fn holding_relieve_min_tax_takes_the_long_lot_at_a_gain() {
        let mut h = Holding::new(Method::Fifo);
        h.push(dated(1, 1, 10, "2025-10-01")).unwrap();
        h.push(dated(2, 1, 12, "2023-01-01")).unwrap();
        let r = h.relieve_min_tax(1, 50, 2, 365, as_of()).unwrap();
        assert_eq!(r.cost, 12);
        assert_eq!(h.lots().len(), 1);
        assert_eq!(h.lots()[0].seq, 1);
    }

    #[test]
    fn an_overflowing_mintax_rank_is_refused() {
        let lots = [dated(1, i64::MAX, i64::MAX / 2, "2023-01-01")];
        let err = relieve_min_tax(&lots, 1, i64::MAX, 2, 365, as_of()).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }

    // ── specific identification — `Ratio.Lots.SpecId` ─────────────────────

    fn spec_holding() -> [Lot; 3] {
        [l(1, 1, 10), l(2, 1, 40), l(3, 1, 70)]
    }

    #[test]
    fn specid_takes_from_the_middle() {
        // ⭐ `Ratio.Lots.SpecId.specid_takes_from_the_middle`.
        let r = relieve_spec_id(&spec_holding(), 1, &[2]).unwrap();
        assert_eq!(r.cost, 40);
        assert_eq!(r.taken[0].seq, 2);
        let left: Vec<u64> = r.left.iter().map(|l| l.seq).collect();
        assert_eq!(left, vec![1, 3]);
    }

    #[test]
    fn no_ordering_takes_the_middle_lot() {
        // `Ratio.Lots.SpecId.no_ordering_takes_the_middle`.
        let lots = spec_holding();
        let named = relieve_spec_id(&lots, 1, &[2]).unwrap().cost;
        assert_eq!(named, 40);
        for m in [Method::Fifo, Method::Lifo, Method::Hifo, Method::Lofo] {
            assert_ne!(
                relieve_by(m, &lots, 1).unwrap().cost,
                named,
                "{m:?} took the middle lot from the holding alone"
            );
        }
    }

    #[test]
    fn an_unknown_lot_is_refused() {
        let err = relieve_spec_id(&spec_holding(), 1, &[9]).unwrap_err();
        assert!(format!("{err:#}").contains("not in this holding"), "{err:#}");
    }

    #[test]
    fn an_overspecified_selection_is_refused() {
        let err = relieve_spec_id(&spec_holding(), 1, &[2, 3]).unwrap_err();
        assert!(format!("{err:#}").contains("more lots than the sale"), "{err:#}");
    }

    #[test]
    fn an_insufficient_selection_is_refused() {
        let err = relieve_spec_id(&spec_holding(), 2, &[2]).unwrap_err();
        assert!(
            format!("{err:#}").contains("short") || format!("{err:#}").contains("units short"),
            "{err:#}"
        );
    }

    #[test]
    fn an_unnamed_selection_is_refused() {
        let err = relieve_spec_id(&spec_holding(), 1, &[]).unwrap_err();
        assert!(format!("{err:#}").contains("no lots were named"), "{err:#}");
    }

    #[test]
    fn a_duplicate_name_is_refused() {
        let err = relieve_spec_id(&spec_holding(), 1, &[2, 2]).unwrap_err();
        assert!(format!("{err:#}").contains("named twice"), "{err:#}");
    }

    #[test]
    fn specid_partial_relief_is_exactly_pro_rata() {
        let err = relieve_spec_id(&[l(1, 7, 100)], 3, &[1]).unwrap_err();
        assert!(format!("{err:#}").contains("does not divide"), "{err:#}");
    }

    #[test]
    fn specid_inherits_the_husk() {
        let r = relieve_spec_id(&[l(1, 0, 40), l(2, 1, 10)], 1, &[1, 2]).unwrap();
        assert_eq!(r.cost, 50);
    }

    #[test]
    fn a_husk_that_was_not_named_stays() {
        let r = relieve_spec_id(&[l(1, 0, 40), l(2, 1, 10)], 1, &[2]).unwrap();
        assert_eq!(r.cost, 10);
        assert_eq!(r.left.iter().map(|l| l.seq).collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn specid_is_not_a_method_variant() {
        // ⛔ THE TRAP. `Method` has no SpecId. Compiles only if it stays that
        // way — a variant would make this match exhaustive and this test
        // would have to name it.
        match Method::Fifo {
            Method::Fifo
            | Method::Lifo
            | Method::Hifo
            | Method::Lofo
            | Method::LongestHeldFirst
            | Method::ShortestHeldFirst => {}
        }
    }

    #[test]
    fn holding_relieve_spec_id_takes_the_named_lot() {
        let mut h = Holding::new(Method::Fifo);
        h.push(l(1, 1, 10)).unwrap();
        h.push(l(2, 1, 40)).unwrap();
        h.push(l(3, 1, 70)).unwrap();
        let r = h.relieve_spec_id(1, &[2]).unwrap();
        assert_eq!(r.cost, 40);
        let left: Vec<u64> = h.lots().iter().map(|l| l.seq).collect();
        assert_eq!(left, vec![1, 3]);
    }

    // ── average cost — `Ratio.Lots.AverageCost` ───────────────────────────

    fn pool_holding() -> [Lot; 3] {
        // 10 / 20 / 60. The pool is 30, and no lot carries 30.
        [l(1, 1, 10), l(2, 1, 20), l(3, 1, 60)]
    }

    #[test]
    fn the_pooled_basis_is_not_any_lots_basis() {
        // ⭐ `Ratio.Lots.AverageCost.the_pooled_basis_is_not_any_lots_basis`.
        assert_eq!(pooled_basis(&pool_holding()).unwrap(), 30);
        let r = relieve_average_cost(&pool_holding(), 1).unwrap();
        assert_eq!(r.cost, 30);
        assert_eq!(r.taken[0].seq, 0, "the pool is the holding, not a lot");
        assert_eq!(r.left.len(), 1);
        assert_eq!(r.left[0].seq, 0);
        assert_eq!(r.left[0].units, 2);
        assert_eq!(r.left[0].cost, 60);
    }

    #[test]
    fn no_ordering_gives_up_the_pooled_basis() {
        // `Ratio.Lots.AverageCost.no_ordering_gives_up_the_pooled_basis`.
        let lots = pool_holding();
        let pooled = relieve_average_cost(&lots, 1).unwrap().cost;
        assert_eq!(pooled, 30);
        for m in [Method::Fifo, Method::Lifo, Method::Hifo, Method::Lofo] {
            assert_ne!(
                relieve_by(m, &lots, 1).unwrap().cost,
                pooled,
                "{m:?} gave up the pooled basis from the lots alone"
            );
        }
    }

    #[test]
    fn an_ordering_leaves_the_other_lots() {
        // `Ratio.Lots.AverageCost.an_ordering_leaves_the_other_lots`.
        let r = relieve_by(Method::Fifo, &pool_holding(), 1).unwrap();
        assert_eq!(r.cost, 10);
        let left: Vec<i64> = r.left.iter().map(|l| l.cost).collect();
        assert_eq!(left, vec![20, 60]);
    }

    #[test]
    fn the_pooled_remainder_is_not_the_unnamed_lots() {
        // Same taken cost as SpecID of lot 2 on 10 / 40 / 70; different remainder.
        let lots = [l(1, 1, 10), l(2, 1, 40), l(3, 1, 70)];
        let pooled = relieve_average_cost(&lots, 1).unwrap();
        let named = relieve_spec_id(&lots, 1, &[2]).unwrap();
        assert_eq!(pooled.cost, 40);
        assert_eq!(named.cost, 40);
        assert_eq!(pooled.left.len(), 1);
        assert_eq!(pooled.left[0].units, 2);
        assert_eq!(pooled.left[0].cost, 80);
        let named_left: Vec<i64> = named.left.iter().map(|l| l.cost).collect();
        assert_eq!(named_left, vec![10, 70]);
    }

    #[test]
    fn an_average_that_does_not_divide_is_refused() {
        let err = relieve_average_cost(&[l(1, 1, 12), l(2, 1, 13)], 1).unwrap_err();
        assert!(format!("{err:#}").contains("does not divide"), "{err:#}");
    }

    #[test]
    fn a_sale_bigger_than_the_pool_is_refused() {
        let err = relieve_average_cost(&pool_holding(), 4).unwrap_err();
        assert!(format!("{err:#}").contains("short"), "{err:#}");
    }

    #[test]
    fn a_zero_unit_holding_has_no_pooled_basis() {
        let err = relieve_average_cost(&[], 1).unwrap_err();
        assert!(format!("{err:#}").contains("no pooled basis"), "{err:#}");
    }

    #[test]
    fn a_partial_pool_is_still_the_unit_basis() {
        let r = relieve_average_cost(&pool_holding(), 2).unwrap();
        assert_eq!(r.cost, 60);
        assert_eq!(r.left[0].units, 1);
        assert_eq!(r.left[0].cost, 30);
    }

    #[test]
    fn average_cost_absorbs_the_husk() {
        // ⚠ `Ratio.Lots.AverageCost.average_cost_absorbs_the_husk`.
        let r = relieve_average_cost(&[l(1, 0, 40), l(2, 1, 10)], 1).unwrap();
        assert_eq!(r.cost, 50);
        assert!(r.left.is_empty());
    }

    #[test]
    fn average_cost_is_not_a_method_variant() {
        // ⛔ THE TRAP. `Method` has no AverageCost. Compiles only if it
        // stays that way — a variant would make this match exhaustive
        // and this test would have to name it.
        match Method::Fifo {
            Method::Fifo
            | Method::Lifo
            | Method::Hifo
            | Method::Lofo
            | Method::LongestHeldFirst
            | Method::ShortestHeldFirst => {}
        }
    }

    #[test]
    fn holding_relieve_average_cost_leaves_a_pool() {
        let mut h = Holding::new(Method::Fifo);
        h.push(l(1, 1, 10)).unwrap();
        h.push(l(2, 1, 20)).unwrap();
        h.push(l(3, 1, 60)).unwrap();
        let r = h.relieve_average_cost(1).unwrap();
        assert_eq!(r.cost, 30);
        assert_eq!(h.lots().len(), 1);
        assert_eq!(h.lots()[0].seq, 0);
        assert_eq!(h.lots()[0].units, 2);
        assert_eq!(h.lots()[0].cost, 60);
    }

    #[test]
    fn an_overflowing_pool_sum_is_refused() {
        // Two lots whose costs sum past i64. Asking the wrapped
        // total a divisibility question would answer about a pool
        // that never happened. `ratio_common::checked`.
        let lots = [l(1, 1, i64::MAX), l(2, 1, 1)];
        let err = relieve_average_cost(&lots, 1).unwrap_err();
        assert!(format!("{err:#}").contains("64 bits"), "{err:#}");
    }
}
