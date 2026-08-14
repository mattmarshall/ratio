//! ratio-gen — a fund with realistic shape, generated the same way every time.
//!
//! `Ratio.Closure.Dials` names what a period end's cost turns on: securities,
//! currencies, lots per security, open corporate actions, capital transactions.
//! This builds a book with those dials set, so the model can be checked against
//! a thing that runs rather than argued about.
//!
//! # ⛔ Deterministic, and not by a random seed that happens to be fixed
//!
//! There is no RNG crate here. Every quantity is a pure function of the dials
//! and a seed, through one integer hash, so the same dials give byte-identical
//! books on any machine and in CI. A generator whose output drifted would make
//! every measurement taken against it unreproducible — which is a strange thing
//! to accept in a system whose entire claim is reproducibility.
//!
//! # ⛔ Open lots reach a STEADY STATE. The journal does not.
//!
//! The first version of this bought and never sold, so open lots grew without
//! bound and "twenty years of fragmentation" meant twenty years of accumulation.
//! That is not how a fund behaves. Positions turn over: new lots open, old ones
//! are relieved, and the OPEN count settles at roughly turnover × holding
//! period rather than climbing forever.
//!
//! Which separates three quantities this crate had collapsed into one:
//!
//!   journal entries   MONOTONIC. Every buy and every sale, forever — an
//!                     append-only log does not forget a closed lot. This is
//!                     what a cold projection build costs, and it is O(history).
//!   open lots         STEADY STATE. What a tax report scans and what
//!                     `Ratio.Closure.lots` counts. Bounded by turnover, not by
//!                     age.
//!   the NAV           `Ratio.Closure.navCost` — neither of the above. Prices,
//!                     rates, and the maintained totals.
//!
//! Conflating the first two is the easy overclaim: "twenty million lots are
//! free" is true of the NAV and false of a cold rebuild, and the demo has to
//! show both curves.
//!
//! # ⚠ What is realistic here and what is not
//!
//! REALISTIC: buys and sells reaching a steady state of open lots; lot counts
//! and sizes drawn from a spread; several currencies with rates; a price per
//! security; corporate actions announced and left outstanding.
//!
//! NOT REALISTIC, and worth saying plainly: prices are a single observation per
//! security rather than a series, and the sale schedule is regular rather than
//! driven by anything. Those change what a NAV is WORTH. They do not change
//! what it COSTS to strike, which is what this exists to measure.

use std::fmt::Write as _;

use anyhow::{bail, Context, Result};
use ratio_store::{
    Account, AccountTypeRecord, AnnouncementRecord, ConfigStore, FileBook, Journal, JournalEntry,
    PostingRecord,
};

/// The shape of the fund to build. Mirrors `Ratio.Closure.Dials`.
///
/// ⛔ SERIALIZED INTO THE BOOK, because a generated book otherwise carries no
/// record of what generated it. `ratio bench --fold` measures a period end using
/// the shape — one mark per security, one rate per currency — so a caller who
/// had to RESTATE the dials could describe a fund that is not the one on disk
/// and get a confident report about it. `SHAPE` is read back instead.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Shape {
    pub securities: i64,
    /// Currencies the chart is denominated in. `Ratio.Closure.fxCost` is one
    /// rate per CURRENCY, not per position — three currencies is three
    /// translations at five names or five hundred.
    pub currencies: i64,
    /// Open tax lots per security AT STEADY STATE — what remains after
    /// relieving, not what was ever bought.
    pub lots_per: i64,
    /// How many lots are opened for each one that stays open.
    ///
    /// ⛔ THE DIAL THAT SEPARATES THE JOURNAL FROM THE LOTS. At 1 nothing is
    /// ever sold and the two grow together, which is the unrealistic case the
    /// first version of this crate had. At 4, three lots in four are relieved:
    /// open lots stay flat while the journal grows four times as fast.
    pub turnover: i64,
    /// Corporate actions announced and left OUTSTANDING — never rewritten, so
    /// the factor read path carries them.
    pub open_actions: i64,
    pub capital_txns: i64,
    pub seed: u64,
    /// Which lots a sale gives up.
    ///
    /// ⛔ A DIAL, BECAUSE THE DEMO COULD NOT SHOW THE ONE THING THIS DECIDES.
    /// The generator wrote `lot_method = "fifo"` as a literal, so every fund on
    /// the demo declared the same method and no screen demonstrated that the
    /// declaration reaches the engine at all — which is the state the engine
    /// wiring fix was made to leave behind. Two funds from ONE seed differing
    /// only in this produce different realized gains from identical holdings
    /// and identical trades, which is the claim.
    pub method: ratio_rules::LotMethod,
}

impl Default for Shape {
    /// An S&P tracker in its twentieth year.
    fn default() -> Self {
        Self {
            securities: 500,
            currencies: 3,
            lots_per: 40,
            turnover: 4,
            open_actions: 0,
            capital_txns: 4,
            seed: 1,
            method: ratio_rules::LotMethod::Fifo,
        }
    }
}

impl Shape {
    pub fn lots(&self) -> i64 {
        self.securities * self.lots_per
    }
}

/// One integer hash, and the only source of variation in this crate.
///
/// splitmix64. ⛔ NOT `rand`: a dependency whose version could change the bytes
/// this produces would make every measurement taken against a generated book
/// unreproducible.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// A value in `lo..=hi`, determined by the inputs.
fn between(seed: u64, a: u64, b: u64, lo: i64, hi: i64) -> i64 {
    if hi <= lo {
        return lo;
    }
    let span = (hi - lo + 1) as u64;
    lo + (mix(seed ^ mix(a).wrapping_add(b)) % span) as i64
}

/// The day the generated fund starts trading, as days since 1970-01-01.
///
/// 2006-01-03 — twenty years before the June 2026 valuation date every
/// measurement here strikes at, which is what "an S&P tracker in its twentieth
/// year" means when the lots have to carry dates.
///
/// ⚠ A CONSTANT, NOT `today`. A generator whose output depended on the day it
/// ran would produce a different book every morning, and every measurement
/// taken against one would be unreproducible — the same reason there is no RNG
/// in this crate.
const FIRST_TRADE_DAY: i64 = 13_151;

/// When the generated corporate actions were announced, in unix seconds.
///
/// ⛔ ONE CONSTANT FOR TWO FIELDS. The announcement record carries it and so
/// does the entry's trade date, and two spellings of one instant would let a
/// view place the entry on a day the announcement itself disagreed with.
const ANNOUNCED_AT: i64 = 1_767_225_600;

/// One book of record a generated fund keeps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenView {
    pub id: String,
    /// `None` recognises on the trade date. `Some(n)` settles `n` open days
    /// after it, over the calendar this generator declares.
    pub settles_in: Option<i64>,
}

/// The books of record a generated fund keeps, and the tail that makes them
/// disagree.
///
/// ⛔ NOT PART OF `Shape`, WHICH MIRRORS `Ratio.Closure.Dials`. A view is not a
/// dimension of a fund's SIZE; it is a term of its administration. Putting it in
/// `Shape` would put it in `ratio bench`'s cost table, where it would sit beside
/// `securities` and `lots_per` meaning nothing.
#[derive(Clone, Debug, Default)]
pub struct Books {
    /// An empty list declares no `[[view]]` section at all, which is what every
    /// fund generated before this looked like and must go on looking like.
    pub views: Vec<GenView>,

    /// The last trading day of the tail, as days since the epoch.
    ///
    /// ⛔ PASSED IN, NOT READ FROM A CLOCK. This crate has no `now` and is not
    /// getting one: it exists to produce the same book from the same dials, and
    /// the seam where the wall clock enters belongs at the CLI — the same place
    /// `strike` takes its valuation point.
    ///
    /// ⚠ AND A CALLER WANTING THE TAIL TO STRADDLE A VALUATION POINT MUST PASS
    /// THAT POINT'S DAY. `ratio strike` values at NOW, so `deploy` passes today.
    /// Folded to the end of history every view agrees, because everything
    /// eventually settles — `Ratio.Views.a_fold_with_no_cut_hides_the_
    /// settlement_gap` — so a tail that has already settled demonstrates
    /// nothing at all.
    pub tail_ends: i64,

    /// Trading days at the end of the journal the tail is spread over.
    pub settle_tail: i64,

    /// How many entries in that tail are SUBSCRIPTIONS.
    ///
    /// ⛔ SUBSCRIPTIONS, BECAUSE A PURCHASE MOVES A NAV BY ZERO. Cash and
    /// investments are both assets, so recognising a trade or not leaves the
    /// figure identical — two views would agree while every line of the engine
    /// ran. A subscription credits equity, which the NAV filter excludes, so the
    /// asset side is left holding the balance and the figure MOVES. HANDOFF.md
    /// records the multi-currency version of this being written vacuously twice.
    pub subscriptions_in_tail: i64,
}

impl Books {
    /// Whether this fund declares any views at all.
    pub fn declared(&self) -> bool {
        !self.views.is_empty()
    }

    /// The `[[calendar]]` and `[[view]]` sections, appended to the generated
    /// configuration. Empty when nothing is declared, so a fund generated
    /// without views produces byte-identical TOML to one generated before views
    /// existed.
    fn toml(&self) -> String {
        if !self.declared() {
            return String::new();
        }
        // ⚠ ONE CALENDAR, NAMED, AND EVERY SETTLEMENT VIEW POINTS AT IT.
        // `View::check` refuses a settlement view naming a calendar nobody
        // declared, at READ time — so emitting the view without the calendar
        // would produce a configuration this generator could not read back.
        let mut s = String::from("\n[[calendar]]\nid = \"settlement\"\n");
        for v in &self.views {
            s.push_str("\n[[view]]\n");
            s.push_str(&format!("id = \"{}\"\n", v.id));
            match v.settles_in {
                None => s.push_str("basis = \"trade\"\n"),
                Some(n) => {
                    s.push_str("basis = \"settlement\"\n");
                    s.push_str(&format!("settles_in = {n}\n"));
                    s.push_str("calendar = \"settlement\"\n");
                }
            }
        }
        s
    }
}

/// Ticker for security `i`. Deterministic, and shaped like a real one.
pub fn ticker(i: i64) -> String {
    let letters = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mut s = String::new();
    let mut n = i as usize;
    for _ in 0..3 {
        s.push(letters[n % 26] as char);
        n /= 26;
    }
    s
}

/// Build the book. Returns how many journal entries were written.
///
/// ⚠ EVERY LOT IS A JOURNAL ENTRY, which is what makes the cold-build cost
/// O(lots) and is the honest shape of the problem. `Ratio.Closure` proves a NAV
/// does not READ the lots; it says nothing about the one-time cost of folding a
/// journal that contains them, and conflating the two would be the easiest
/// overclaim available here.
pub fn generate(path: &std::path::Path, shape: Shape) -> Result<usize> {
    generate_books(path, shape, &Books::default())
}

/// The same fund, keeping the books of record `books` declares.
///
/// ⚠ `generate` IS THIS WITH NOTHING DECLARED, and that case must stay
/// byte-identical to what this crate produced before views existed — every
/// measurement in HANDOFF.md was taken against one.
pub fn generate_books(path: &std::path::Path, shape: Shape, books: &Books) -> Result<usize> {
    if books.subscriptions_in_tail > 0 {
        if books.settle_tail <= 0 {
            bail!(
                "a settlement tail of {} subscriptions needs days to spread over — pass \
                 --settle-tail",
                books.subscriptions_in_tail
            );
        }
        if books.tail_ends <= 0 {
            bail!("a settlement tail has to end on a day — `Books::tail_ends` was not set");
        }
        // ⛔ REFUSED, NOT WRAPPED. Spreading more subscriptions than there are
        // days would put two on one day and leave the last day of the tail
        // empty, which is the day most likely to straddle the valuation point.
        // A demo that quietly generated a fund the views agree about is the one
        // outcome this whole fund exists to avoid.
        if books.subscriptions_in_tail > books.settle_tail {
            bail!(
                "{} subscriptions will not fit in a {}-day tail — one a day, so the last day \
                 of the tail is never the empty one",
                books.subscriptions_in_tail,
                books.settle_tail
            );
        }
    }

    let _ = std::fs::remove_dir_all(path);
    let mut b = FileBook::open(path).context("creating the book")?;

    b.put_accounts(&[
        Account { dim: 1, display_name: "Investments at fair value".into(), account_type: AccountTypeRecord::Asset },
        Account { dim: 2, display_name: "Cash and equivalents".into(), account_type: AccountTypeRecord::Asset },
        Account { dim: 20, display_name: "Capital".into(), account_type: AccountTypeRecord::Equity },
        Account { dim: 30, display_name: "Realized gain on investments".into(), account_type: AccountTypeRecord::Income },
        Account { dim: 40, display_name: "Currency conversion".into(), account_type: AccountTypeRecord::Asset },
    ])?;
    // ⛔ THE CHART NAMES ITS ROLES. The engine cannot guess which dimension is
    // realized gain, and `Ratio.Lots.Posting.a_collided_chart_hides_the_gain`
    // is what happens if two roles collide — so the configuration says so, and
    // the rule set refuses a collision when it is read.
    let owned = format!(
        "lot_method = \"{}\"\nrules = []\n\n[chart_roles]\n\
         investments = 1\ncash = 2\nrealized_gain = 30\ncurrency_conversion = 40\n{}",
        shape.method.as_declared(),
        // ⛔ THE VIEWS GO IN THE SAME CONFIGURATION EVERY ENTRY PINS, which is
        // what makes this one journal rather than two books. A per-view file
        // would leave the views unpinned while the rules were pinned, and a
        // calendar amended later would silently move a NAV already struck —
        // `//tla:calendar_in_side_file_check` is exactly that.
        books.toml()
    );
    let config: &str = &owned;
    let cfg = b.put(config.as_bytes())?;
    // ⛔ READ BACK OUT OF THE CONFIGURATION, NOT WRITTEN TWICE. This used to be
    // a Rust literal standing beside the TOML above with nothing checking the
    // two agreed — so the generator could post through one chart while
    // declaring another, and every figure would have tied.
    //
    // ⚠ And going through `from_toml` is what runs `ChartRoles::check`, which
    // the literal skipped entirely.
    let roles = ratio_rules::RuleSet::from_toml(config)?
        .chart_roles
        .expect("the configuration above declares chart roles");
    b.set_active(&cfg)?;

    let mut written = 0usize;

    let base = currency_code(0);

    // Capital first: the fund has to be funded before it can buy anything.
    // Subscribed in the base currency, because that is what an investor sends.
    for k in 0..shape.capital_txns {
        let amount = between(shape.seed, 7, k as u64, 10_000_000_00, 50_000_000_00);
        b.append(&JournalEntry {
            id: format!("cap-{k}"),
            memo: "subscription".into(),
            config: cfg.clone(),
            postings: vec![
                PostingRecord::of_currency(2, amount, base),
                PostingRecord::of_currency(20, -amount, base),
            ],
            // ⛔ DATED, LIKE EVERY OTHER ENTRY HERE, AND IT USED NOT TO BE. A
            // settlement view refuses an entry it cannot date — rightly, since
            // there is nothing to roll forward from — so a generator that dated
            // only its trades produced a fund every settlement view refused
            // outright. Funding lands before the first trade, which is the only
            // thing about the day that matters.
            trade_date: Some(ratio_common::iso_date_from_days(FIRST_TRADE_DAY - 30 + k)),
            announcement: None,
        })?;
        written += 1;
    }

    // ⛔ AND THEN THE FUND BUYS ITS FOREIGN CURRENCY, IN FOUR LEGS. A fund
    // holding EUR securities needs EUR cash, and getting it is an EXCHANGE:
    //
    //     [USD −100.00, EUR +92.00]     sum 0, USD out by 100, EUR out by 92
    //
    // Two legs conserve neither currency — `Ratio.Chart.Dimensions.a_two_leg_
    // exchange_conserves_neither_currency` — and the door refuses them, which
    // is the whole point of the law. Four legs through the conversion account
    // conserve both and leave the rate behind as the pair of figures sitting in
    // that account: `an_exchange_conserves_and_leaves_the_rate_behind`.
    let conversion = roles
        .currency_conversion
        .expect("the configuration above declares a conversion account");
    for c in 1..shape.currencies {
        let code = currency_code(c);
        let rate = fx_rate(shape, c);
        // Enough of each currency to buy in it, in the base currency's terms.
        let spend = between(shape.seed, 19, c as u64, 5_000_000_00, 20_000_000_00);
        let got = spend * rate / 100;
        b.append(&JournalEntry {
            id: format!("fx-{code}"),
            memo: format!("buy {code}"),
            config: cfg.clone(),
            postings: vec![
                PostingRecord::of_currency(2, -spend, base),
                PostingRecord::of_currency(conversion, spend, base),
                PostingRecord::of_currency(conversion, -got, code),
                PostingRecord::of_currency(2, got, code),
            ],
            // After the funding and before the first trade — the fund buys the
            // currency it is about to trade in. See `cap-{k}` above for why an
            // undated entry is not an option.
            trade_date: Some(ratio_common::iso_date_from_days(FIRST_TRADE_DAY - 20 + c)),
            announcement: None,
        })?;
        written += 1;
    }

    // Then trading, to a STEADY STATE.
    //
    // ⛔ `turnover` LOTS ARE OPENED FOR EACH ONE LEFT OPEN, and the rest are
    // relieved. The journal grows by all of them; the open count settles at
    // `lots_per`. That separation is the whole point — a cold projection build
    // is O(journal) and a tax scan is O(open lots), and they are different
    // numbers by a factor of `turnover`.
    //
    // ⚠ Counts VARY per security. Uniform lots would make every partition the
    // same size and hide `Ratio.Exec.the_slowest_partition_sets_the_pace`,
    // which is the tail a planner has to survive.
    // ⛔ BATCHED. `FileBook::append` opens and closes the journal every call —
    // ~4 ms an entry, 549 seconds for 140,000, twenty-one hours for the twenty
    // million this system's central claim is about. Measured, not assumed.
    // `append_all` runs the same per-entry checks and shares one file handle.
    // ⛔ FLUSHED IN CHUNKS, NOT ACCUMULATED. This held every trading entry in
    // memory before writing any of them — the same "materialize what you are
    // folding" shape as the read path, on the write side. At the twenty-million
    // lot book issue #6 asks for it is the generator, not the fold, that would
    // run out of memory first.
    //
    // ⚠ `append_all` was already all-or-nothing on VALIDATION rather than on
    // writing — "a mid-write failure leaves what was appended" — so chunking
    // does not weaken a guarantee that was there. What it keeps is the one file
    // handle per chunk, which is why `append_all` exists at all.
    const FLUSH_EVERY: usize = 65_536;
    let mut batch: Vec<JournalEntry> = Vec::with_capacity(FLUSH_EVERY);
    let mut memo = String::new();
    for i in 0..shape.securities {
        let t = ticker(i);
        let keep =
            between(shape.seed, 1, i as u64, (shape.lots_per / 2).max(1), shape.lots_per * 3 / 2);
        let opened = keep * shape.turnover.max(1);
        // A ring of open lots: buy, and once there are more than `keep`, sell
        // the oldest. `Ratio.Lots.relief_touches_only_what_it_takes` is the FIFO
        // rule; here it is only the shape that matters.
        // ⛔ TRADES ARE DATED, AND THE SPACING VARIES BY SECURITY. Without a
        // trade date a lot has no acquisition date, the holding-period methods
        // refuse it, and every realized gain lands in `unclassified` — so the
        // short/long split would be built, shipped and exercised by nothing.
        // That is the same failure as selling at cost: the engine looks correct
        // because nothing asks it the question.
        //
        // ⚠ THE SPACING HAS TO STRADDLE THE THRESHOLD. A lot is relieved `keep`
        // buys after it opens, so its holding period is `keep × step` days.
        // A step that made every security's holding over a year would produce a
        // book that is entirely long-term, which tests the split no better than
        // no dates at all. Between 3 and 20 days against these ring sizes puts
        // securities on both sides of 365.
        let step = between(shape.seed, 17, i as u64, 3, 20);
        let ccy = currency_of(i, shape.currencies);
        // ⛔ THE HOLDING IS RELIEVED BY THE METHOD THE CONFIGURATION DECLARES,
        // and this was a `VecDeque` popped from the FRONT — hardcoded
        // oldest-first, whatever had just been written to the config. A book
        // generated `--method hifo` declared HIFO and carried FIFO-computed
        // gains; the engine reading it back relieved HIFO, disagreed with every
        // posted sale, and reported 242 lot breaks with 75% of the realized
        // gain unclassifiable. Exactly the defect the engine wiring fixed,
        // alive on the other side of the same seam.
        let mut open = ratio_project::relief::Holding::new(shape.method.into());
        let mut next_seq: u64 = 0;
        for l in 0..opened {
            // Twenty years of history, ending before the June 2026 valuation
            // date every measurement here strikes at.
            let day = ratio_common::iso_date_from_days(FIRST_TRADE_DAY + l * step);
            let units = between(shape.seed, 2, (i * 4096 + l) as u64, 2, 500) * 2;
            let cost = units * between(shape.seed, 3, (i * 4096 + l) as u64, 10_00, 400_00);
            memo.clear();
            let _ = write!(memo, "buy {t}");
            batch.push(JournalEntry {
                id: format!("t-{t}-{l}"),
                memo: memo.clone(),
                config: cfg.clone(),
                postings: vec![
                    PostingRecord {
                        dim: 1,
                        amount: cost,
                        // ⛔ THE SECURITY'S OWN CURRENCY, from the master, which
                        // is the one authority on it. Both legs carry it, so the
                        // entry conserves within that currency and no lot ever
                        // changes denomination.
                        currency: Some(ccy.to_string()),
                        instrument: Some(t.clone()),
                        quantity: Some(units),
                    },
                    PostingRecord::of_currency(2, -cost, ccy),
                ],
                trade_date: Some(day.clone()),
                announcement: None,
            });
            written += 1;
            open.push(ratio_project::relief::Lot {
                seq: next_seq,
                units,
                cost,
                // ⚠ A DAY NUMBER, which the generator already has — the ISO
                // string above is derived FROM it. Parsing that string back
                // would be a round trip through the one representation that
                // can fail.
                acquired: Some((FIRST_TRADE_DAY + l * step) as i32),
            })?;
            next_seq += 1;
            if batch.len() >= FLUSH_EVERY {
                b.append_all(&batch)?;
                batch.clear();
            }

            if open.len() as i64 > keep {
                // ⚠ SELL EXACTLY THE LOT THE METHOD WOULD GIVE UP, WHOLE. A
                // partial relief divides a lot's cost pro rata and REFUSES when
                // that does not land on a whole minor unit, so picking any
                // other quantity generates books that break for reasons with
                // nothing to do with the method being demonstrated.
                let want = open.peek().expect("len > keep").units;
                let (u, c) = (want, open.relieve(shape.method.into(), want)?.cost);
                memo.clear();
                let _ = write!(memo, "sell {t}");
                // ⛔ THREE LEGS, AND THE PROCEEDS ARE NOT THE BASIS. This sold
                // at cost until now, so every disposal realized nothing and the
                // gain account never moved — which made the lot engine look
                // correct while exercising none of it. A sale at a markup or a
                // markdown is the case that tests it.
                //
                // ⛔ AND THE PRICE IS PER UNIT AND INDEPENDENT OF THE LOT. The
                // proceeds were `basis + swing`, derived from the cost of
                // whichever lot was given up — so choosing a DEARER lot also
                // fetched more, and HIFO came out with the LARGEST taxable gain
                // of the four methods. Backwards, and backwards in the
                // direction that would have been quoted: HIFO exists to reduce
                // a gain. The market does not know your lot method.
                //
                // ⚠ THE BAND SITS ABOVE THE COST BAND, because twenty years of
                // equity history that ends flat is not the fund anyone is
                // demonstrating. Costs are drawn from 10.00–400.00, prices from
                // 12.00–480.00 — a fifth higher, so the book carries a gain
                // under most methods and a loss under the one chosen to harvest
                // them.
                let px = between(shape.seed, 13, (i * 4096 + l) as u64, 12_00, 480_00);
                let proceeds = u * px;
                batch.push(JournalEntry {
                    id: format!("s-{t}-{l}"),
                    memo: memo.clone(),
                    config: cfg.clone(),
                    postings: ratio_project::relief::sale_postings(
                        roles,
                        Some(ccy),
                        &t,
                        u,
                        c,
                        proceeds,
                    )?,
                    // The same day the buy that displaced it happened, so the
                    // holding period is exactly `keep` steps.
                    trade_date: Some(day.clone()),
                    announcement: None,
                });
                written += 1;
            }
        }
    }

    b.append_all(&batch)?;
    batch.clear();

    // Corporate actions, announced and left OUTSTANDING.
    //
    // ⛔ ANNOUNCED ONLY — no `action-{id}` entry, so nothing is rewritten and
    // the factor read path carries them. That is the case the whole redesign is
    // about: `Ratio.Closure.an_open_action_makes_the_nav_read_the_lots` is what
    // a rewrite would cost here, and it is 40,000 reads per action.
    //
    // ⛔ N-FOR-1 ONLY, AND A TEST CAUGHT ME GETTING THIS WRONG. The first
    // version used 2-for-1 and 1-for-2, on the reasoning that both are "clean"
    // ratios. A 1-for-2 REVERSE split halves the holding, and half of an odd
    // number of units is not a number of units — `every_generated_holding_can_
    // actually_be_valued` failed on `CAA` at 1,853 units.
    //
    // That refusal is correct: the holder is paid cash for the fraction, which
    // realizes a gain and is a posting no configuration here declares. But a
    // generator that produces books which cannot be valued measures nothing, so
    // this only emits ratios that always divide. Reverse splits are a real case
    // and belong in a book that declares how it handles cash in lieu.
    for k in 0..shape.open_actions {
        let i = between(shape.seed, 5, k as u64, 0, (shape.securities - 1).max(0));
        let (num, den) = if k % 2 == 0 { (2, 1) } else { (3, 1) };
        b.append(&JournalEntry {
            id: format!("announce-ca-{k}"),
            memo: format!("announced ca-{k}"),
            config: cfg.clone(),
            postings: Vec::new(),
            // ⚠ THE DAY IT WAS ANNOUNCED, NOT THE EX-DATE. This entry carries no
            // postings, so what a view does with the date changes no figure —
            // but a view that cannot PLACE it refuses the whole book, and an
            // announcement's own day is the day somebody was told.
            trade_date: Some(ratio_common::iso_date_from_days(ANNOUNCED_AT / 86_400)),
            announcement: Some(AnnouncementRecord {
                id: format!("ca-{k}"),
                instrument: ticker(i),
                numerator: num,
                denominator: den,
                ex_date: "2026-01-15".into(),
                announced: ANNOUNCED_AT,
            }),
        })?;
        written += 1;
    }

    // ⛔ AND THEN THE TAIL: THE ONLY ENTRIES IN THIS FILE ANCHORED TO A DAY THE
    // CALLER CHOSE, AND THE ONLY REASON TWO VIEWS OF THIS FUND DISAGREE.
    //
    // ⭐ FOLDED TO THE END OF HISTORY EVERY VIEW AGREES, because everything
    // eventually settles — `Ratio.Views.a_fold_with_no_cut_hides_the_settlement_
    // gap`. The difference between two books of record lives entirely at a CUT,
    // so these have to be traded ON OR BEFORE the valuation point and settle
    // AFTER it. `ratio strike` values at now, which is why `tail_ends` is passed
    // in rather than derived from `FIRST_TRADE_DAY` like everything above.
    //
    // ⛔ SUBSCRIPTIONS, AND A PURCHASE WOULD NOT DO. Cash and investments are
    // both assets, so recognising a trade or not moves the NAV by ZERO and the
    // two views would agree while every line of the engine ran. Capital is
    // equity, the NAV filter excludes it, and the asset side is left holding the
    // balance — so the figure MOVES. HANDOFF.md records the multi-currency
    // version of this being written vacuously twice before anybody noticed.
    for j in 0..books.subscriptions_in_tail {
        // One a day, most recent first, so the last day of the tail is never
        // the empty one — it is the day most likely to straddle the cut.
        let day = books.tail_ends - j;
        let amount = between(shape.seed, 23, j as u64, 5_000_000_00, 30_000_000_00);
        b.append(&JournalEntry {
            id: format!("sub-tail-{j}"),
            memo: format!("subscription {}", j + 1),
            config: cfg.clone(),
            postings: vec![
                PostingRecord::of_currency(2, amount, base),
                PostingRecord::of_currency(20, -amount, base),
            ],
            trade_date: Some(ratio_common::iso_date_from_days(day)),
            announcement: None,
        })?;
        written += 1;
    }

    // ⛔ AND THE THINGS A NAV ACTUALLY READS. `Ratio.Closure.navCost` is
    // `markCost + fxCost + actionCost + capitalCost` — one price per SECURITY
    // and one rate per CURRENCY. A book with lots and no prices can be folded
    // into a trial balance and cannot be VALUED, so a benchmark against one
    // would measure the wrong thing entirely.
    let master: Vec<ratio_ingest::Entity> = (0..shape.securities)
        .map(|i| ratio_ingest::Entity {
            id: ticker(i),
            kind: ratio_ingest::EntityKind::Instrument,
            display_name: format!("{} Corp", ticker(i)),
            attributes: [
                ("ticker".to_string(), ticker(i)),
                ("currency".to_string(), currency_of(i, shape.currencies).to_string()),
            ]
            .into_iter()
            .collect(),
        })
        .collect();
    for e in &master {
        b.append_record(ratio_store::Plane::Entities, e)?;
    }

    // One price per security, and one rate per currency other than the base.
    // `fx_does_not_grow_with_the_chart`: three currencies is three rows at five
    // names or five hundred.
    for i in 0..shape.securities {
        let t = ticker(i);
        let px = between(shape.seed, 11, i as u64, 5_00, 800_00);
        // ⛔ IN THE SECURITY'S OWN CURRENCY. Every price said "USD" while the
        // entity master called one security in three EUR and one in three GBP —
        // three statements of a fact, two of them disagreeing, and nothing
        // reading both.
        b.append_record(
            ratio_store::Plane::Facts,
            &price_fact(&t, px, currency_of(i, shape.currencies), shape),
        )?;
    }
    for c in 1..shape.currencies {
        b.append_record(
            ratio_store::Plane::Facts,
            &rate_fact(currency_code(c), fx_rate(shape, c), shape),
        )?;
    }

    // ⛔ LAST, SO IT IS ONLY THERE IF EVERYTHING ABOVE SUCCEEDED. A book that
    // failed halfway through generation must not read back as a complete fund of
    // a declared shape; a missing `SHAPE` is the honest signal that this is not a
    // generated book, and `shape_of` says so in those words.
    put_shape(path, shape)?;

    Ok(written)
}

/// Where a generated book records the dials it was generated from.
const SHAPE: &str = "SHAPE";

fn put_shape(path: &std::path::Path, shape: Shape) -> Result<()> {
    let json = serde_json::to_string_pretty(&shape).context("serializing the shape")?;
    std::fs::write(path.join(SHAPE), json)
        .with_context(|| format!("writing {}", path.join(SHAPE).display()))
}

/// The dials a generated book was generated from.
///
/// ⛔ THE ONLY WAY TO LEARN A BOOK'S SHAPE, and it refuses rather than guessing.
/// A period end measured against restated dials is a report about a fund that
/// may not be the one on disk — one mark per security and one rate per currency
/// are read off this, so a wrong `securities` marks names that were never
/// generated and a wrong `currencies` translates at rates nothing traded at.
/// Both would tie, and both would be somebody else's fund.
pub fn shape_of(path: &std::path::Path) -> Result<Shape> {
    let p = path.join(SHAPE);
    let raw = std::fs::read_to_string(&p).with_context(|| {
        format!(
            "{} has no {SHAPE}, so it did not come from `ratio gen` — there is nothing here \
             that says how many securities or currencies it holds, and a period end measured \
             against a shape somebody restated would be a report about a different fund",
            path.display()
        )
    })?;
    serde_json::from_str(&raw).with_context(|| format!("reading {}", p.display()))
}

/// What one unit of currency `c` is worth in the base, in hundredths.
///
/// ⛔ ONE FUNCTION, BECAUSE THE RATE IS ONE FACT. The exchange that buys the
/// currency and the rate fact the valuation reads must be the same number — two
/// expressions of it drifting apart would leave a fund whose books were bought
/// at one rate and valued at another, with both figures internally consistent.
fn fx_rate(shape: Shape, c: i64) -> i64 {
    // ⛔ HUNDREDTHS — `ratio_project::RATE_SCALE`. 0.70 to 1.50 of the base per
    // unit of the foreign currency, which is what an FX rate between developed
    // currencies looks like.
    //
    // ⚠ THIS SAID `70_00, 150_00` AND NOTHING NOTICED. Read at a scale of 100
    // that is 70.00 to 150.00 base per unit — seventy dollars to the euro — and
    // it produced a fund whose realized gain was seventy times its basis. It
    // survived because the only consumer was `ratio bench`'s FX phase, which
    // multiplied by it and threw the answer away to time the multiplication:
    // `let _ = gross.saturating_mul(rate) / 100`. A number nothing reads is a
    // number nothing checks.
    between(shape.seed, 12, c as u64, 70, 150)
}

/// The rates a book of this shape was generated at.
///
/// ⛔ EXPORTED SO A READER DOES NOT RE-DERIVE THEM. A NAV over a multi-currency
/// book is meaningless without saying at what rate, and a caller inventing its
/// own would value the fund at a rate it was never traded at — the books would
/// still tie. In a real deployment these come from the rate FACTS in the data
/// plane; here the generator and the reader share one function.
pub fn rates(shape: Shape) -> ratio_project::Rates {
    ratio_project::Rates::of(
        currency_code(0),
        (1..shape.currencies).map(|c| (currency_code(c).to_string(), fx_rate(shape, c))),
    )
}

/// Which currency security `i` is denominated in.
fn currency_of(i: i64, currencies: i64) -> &'static str {
    currency_code(if currencies <= 1 { 0 } else { i % currencies })
}

/// The ISO code for currency index `c`, for a caller that needs to name one.
pub fn currency_code_of(c: i64) -> &'static str {
    currency_code(c)
}

fn currency_code(c: i64) -> &'static str {
    match c {
        0 => "USD",
        1 => "EUR",
        2 => "GBP",
        3 => "JPY",
        _ => "CHF",
    }
}

fn provenance(shape: Shape) -> ratio_ingest::Provenance {
    ratio_ingest::Provenance {
        delivery: format!("generated-{}", shape.seed),
        row: 1,
        template: "generated".into(),
        template_id: "ratio-gen".into(),
        received: 1_767_225_600,
    }
}

fn price_fact(ticker: &str, minor: i64, currency: &str, shape: Shape) -> ratio_ingest::Fact {
    ratio_ingest::Fact {
        id: format!("px-{ticker}"),
        kind: "price".into(),
        reference: ticker.into(),
        entities: [(
            "instrument".to_string(),
            ratio_ingest::EntityRef {
                kind: ratio_ingest::EntityKind::Instrument,
                // One rung: match on the ticker. `Ratio.Ingest.empty_rung_
                // matches_nothing` — a rung with no claims would match EVERY
                // instrument in the master and book against an arbitrary one.
                rungs: vec![vec![ratio_ingest::Claim {
                    attr: "ticker".into(),
                    value: ticker.to_string(),
                }]],
            },
        )]
        .into_iter()
        .collect(),
        values: [
            ("asOf".to_string(), ratio_ingest::Value::Date { iso: "2026-06-30".into() }),
            (
                "price".to_string(),
                ratio_ingest::Value::Money { minor, currency: currency.into() },
            ),
        ]
        .into_iter()
        .collect(),
        provenance: provenance(shape),
    }
}

fn rate_fact(code: &str, minor: i64, shape: Shape) -> ratio_ingest::Fact {
    ratio_ingest::Fact {
        id: format!("fx-{code}"),
        kind: "rate".into(),
        reference: code.into(),
        entities: Default::default(),
        values: [
            ("asOf".to_string(), ratio_ingest::Value::Date { iso: "2026-06-30".into() }),
            ("currency".to_string(), ratio_ingest::Value::Text { text: code.into() }),
            ("rate".to_string(), ratio_ingest::Value::Decimal { minor }),
        ]
        .into_iter()
        .collect(),
        provenance: provenance(shape),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("ratio-gen-{name}"))
    }

    #[test]
    fn the_same_shape_gives_the_same_book_byte_for_byte() {
        // ⛔ The property every measurement taken against this depends on. A
        // generator that drifted would make its own benchmarks unreproducible.
        let s = Shape { securities: 12, currencies: 2, turnover: 3, lots_per: 6, open_actions: 2, capital_txns: 2, seed: 7, method: ratio_rules::LotMethod::Fifo };
        let a = tmp("det-a");
        let b = tmp("det-b");
        generate(&a, s).unwrap();
        generate(&b, s).unwrap();
        assert_eq!(
            std::fs::read(a.join("journal.jsonl")).unwrap(),
            std::fs::read(b.join("journal.jsonl")).unwrap(),
        );
    }

    #[test]
    fn every_entry_a_generated_fund_writes_carries_a_trade_date() {
        // ⛔ THE PROPERTY THAT WAS BROKEN, AND IT WAS INVISIBLE. Trades were
        // dated and capital, FX and announcements were not — which no test
        // noticed, because the only view any generated fund had recognised in
        // journal order and consults no date at all. The moment one declared a
        // settlement basis it refused the whole book.
        //
        // ⚠ ASSERTED OVER A SHAPE THAT WRITES ALL FOUR KINDS. `open_actions` and
        // `currencies > 1` are what put an announcement and an exchange in the
        // journal; at the defaults for either, this test would pass over a book
        // that contains neither.
        let d = tmp("dated");
        let n = generate(
            &d,
            Shape {
                securities: 4,
                currencies: 3,
                lots_per: 3,
                turnover: 2,
                open_actions: 2,
                capital_txns: 2,
                seed: 5,
                method: ratio_rules::LotMethod::Fifo,
            },
        )
        .unwrap();
        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        assert_eq!(entries.len(), n);
        assert!(entries.iter().any(|e| e.id.starts_with("cap-")), "no capital entry");
        assert!(entries.iter().any(|e| e.id.starts_with("fx-")), "no exchange");
        assert!(entries.iter().any(|e| e.announcement.is_some()), "no announcement");
        let undated: Vec<&str> =
            entries.iter().filter(|e| e.trade_date.is_none()).map(|e| e.id.as_str()).collect();
        assert!(undated.is_empty(), "a settlement view would refuse these: {undated:?}");
    }

    #[test]
    fn two_declared_views_land_in_the_configuration_every_entry_pins() {
        // ⭐ ONE JOURNAL, AND THE VIEWS TRAVEL IN THE SAME CONFIGURATION THE
        // RULES DO. A per-view side file would leave the views unpinned while
        // the rules were pinned, and a calendar amended later would move a NAV
        // already struck — `//tla:calendar_in_side_file_check`.
        let d = tmp("twoviews");
        let books = Books {
            views: vec![
                GenView { id: "abor".into(), settles_in: None },
                GenView { id: "ibor".into(), settles_in: Some(2) },
            ],
            tail_ends: 20_600,
            settle_tail: 3,
            subscriptions_in_tail: 2,
        };
        generate_books(&d, Shape { securities: 3, lots_per: 2, ..Shape::default() }, &books)
            .unwrap();

        let b = FileBook::open(&d).unwrap();
        let digest = b.active().unwrap().expect("a generated book has an active configuration");
        let set =
            ratio_rules::RuleSet::from_toml(&String::from_utf8_lossy(&b.get(&digest).unwrap()))
                .expect("the generated configuration reads back");
        let views = set.effective_views();
        assert_eq!(views.len(), 2, "{views:?}");
        assert!(set.views_declared(), "these were elected, and the fund should say so");
        assert_eq!(views[0].id, "abor");
        assert_eq!(views[0].basis, ratio_rules::Basis::Trade);
        assert_eq!(views[1].basis, ratio_rules::Basis::Settlement);
        assert_eq!(views[1].settles_in, Some(2));
        // ⛔ AND THE CALENDAR IT NAMES IS DECLARED. `View::check` refuses a
        // settlement view pointing at one nobody wrote down, at READ time — so
        // a generator emitting the view without the calendar would produce a
        // configuration it could not read back.
        assert!(set.calendar(views[1].calendar.as_deref().unwrap()).is_some());

        // The tail is where the two views disagree, so it has to be there.
        let entries = b.entries().unwrap();
        let tail: Vec<_> = entries.iter().filter(|e| e.id.starts_with("sub-tail-")).collect();
        assert_eq!(tail.len(), 2, "the settlement tail is missing");
        for e in &tail {
            let day = ratio_common::days_from_iso_date(e.trade_date.as_deref().unwrap()).unwrap();
            assert!(day <= 20_600, "a tail entry traded after the day the tail ends");
            assert!(day > 20_600 - 3, "a tail entry fell outside the tail");
        }
    }

    #[test]
    fn a_tail_longer_than_its_days_is_refused_rather_than_doubled_up() {
        // ⛔ Doubling up would leave the LAST day of the tail empty, and that is
        // the day most likely to straddle the valuation point — a demo that
        // quietly generated a fund the two views agree about.
        let e = generate_books(
            &tmp("tail-overflow"),
            Shape::default(),
            &Books { tail_ends: 20_600, settle_tail: 2, subscriptions_in_tail: 5, ..Default::default() },
        )
        .unwrap_err()
        .to_string();
        assert!(e.contains("will not fit"), "{e}");
    }

    #[test]
    fn a_different_seed_gives_a_different_book() {
        // Otherwise the previous test passes on a generator that ignores its
        // inputs entirely.
        let a = tmp("seed-a");
        let b = tmp("seed-b");
        generate(&a, Shape { securities: 8, lots_per: 4, seed: 1, ..Shape::default() }).unwrap();
        generate(&b, Shape { securities: 8, lots_per: 4, seed: 2, ..Shape::default() }).unwrap();
        assert_ne!(
            std::fs::read(a.join("journal.jsonl")).unwrap(),
            std::fs::read(b.join("journal.jsonl")).unwrap(),
        );
    }

    #[test]
    fn the_book_it_writes_balances() {
        // Every entry passed `FileBook::append`'s check at the door, so this is
        // really asserting that the generator produced entries at all — but a
        // generator that wrote nothing would also produce a balanced book, so:
        let d = tmp("balanced");
        let n = generate(&d, Shape { securities: 5, currencies: 2, turnover: 3, lots_per: 4, open_actions: 1, capital_txns: 2, seed: 3, method: ratio_rules::LotMethod::Fifo }).unwrap();
        let b = FileBook::open(&d).unwrap();
        let entries = b.entries().unwrap();
        assert_eq!(entries.len(), n);
        assert!(n > 20, "only {n} entries");
        assert!(entries.iter().all(|e| e.is_balanced()));
    }

    #[test]
    fn lot_counts_vary_between_securities() {
        // ⚠ Uniform lot counts would make every partition the same size and
        // hide `Ratio.Exec.the_slowest_partition_sets_the_pace` — the tail a
        // real planner has to survive.
        let d = tmp("varies");
        generate(&d, Shape { securities: 30, currencies: 2, turnover: 3, lots_per: 20, open_actions: 0, capital_txns: 1, seed: 4, method: ratio_rules::LotMethod::Fifo })
            .unwrap();
        let b = FileBook::open(&d).unwrap();
        let mut counts = std::collections::BTreeMap::<String, usize>::new();
        for e in b.entries().unwrap() {
            for p in e.postings {
                if let Some(i) = p.instrument {
                    *counts.entry(i).or_default() += 1;
                }
            }
        }
        let lo = counts.values().min().copied().unwrap();
        let hi = counts.values().max().copied().unwrap();
        assert!(hi > lo, "every security got {lo} lots — the spread is not being applied");
    }

    #[test]
    fn open_actions_are_announced_and_never_rewritten() {
        // ⛔ The case the redesign is about. An `action-{id}` entry here would
        // mean the lots had been walked, which is the cost being avoided.
        let d = tmp("open");
        generate(&d, Shape { securities: 6, currencies: 2, turnover: 3, lots_per: 4, open_actions: 3, capital_txns: 1, seed: 5, method: ratio_rules::LotMethod::Fifo })
            .unwrap();
        let entries = FileBook::open(&d).unwrap().entries().unwrap();
        assert_eq!(entries.iter().filter(|e| e.announcement.is_some()).count(), 3);
        assert!(
            !entries.iter().any(|e| e.id.starts_with("action-")),
            "nothing was rewritten"
        );
    }

    #[test]
    fn the_generated_book_realizes_gains_on_both_sides_of_the_threshold() {
        // ⛔ THE GENERATOR HAS TO EXERCISE WHAT IT CLAIMS TO. It sold at cost
        // until the gain posting landed, so every disposal realized nothing and
        // six lot methods ran against a book where the answer was always zero.
        // The same shape of mistake was live again: undated trades gave every
        // lot no acquisition date, so the whole realized gain fell into
        // `unclassified` and the short/long split was exercised by nothing.
        //
        // ⚠ BOTH SIDES, NOT JUST NON-ZERO. A book that is entirely long-term
        // tests the classification no better than one that is entirely
        // unclassified — it passes whatever the threshold comparison does.
        let d = tmp("dated");
        generate(&d, Shape { securities: 20, lots_per: 40, ..Shape::default() }).unwrap();
        let shape = Shape { securities: 20, lots_per: 40, ..Shape::default() };
        let roles = ratio_rules::RuleSet::from_toml(
            &String::from_utf8_lossy(&FileBook::open(&d).unwrap().get(
                &FileBook::open(&d).unwrap().active().unwrap().unwrap()).unwrap()))
            .unwrap()
            .chart_roles
            .unwrap();
        let p = ratio_project::Projection::of_book(&d).unwrap();
        let r = p.realized(ratio_rules::UNDECLARED_VIEW, Some(roles), &rates(shape)).unwrap().value.unwrap();

        assert!(r.short_term < 0, "no short-term gains: {r:?}");
        assert!(r.long_term < 0, "no long-term gains: {r:?}");
        assert_eq!(
            r.short_term + r.long_term + r.unclassified(),
            r.gain,
            "the split must partition the total"
        );
        // ⚠ NOT EXACTLY ZERO, AND THE BOUND IS THE POINT. Every generated trade
        // carries a date, so nothing is unclassified for want of a holding
        // period. What is left is the translation residue: the total and the
        // two parts are each translated from three currencies into the base,
        // and integer division does not distribute over a sum. At most one
        // minor unit per currency per bucket.
        assert!(
            r.unclassified().abs() <= 2 * shape.currencies as i128,
            "every generated trade carries a date, so this should be a rounding \
             residue and nothing more: {r:?}"
        );
    }

    #[test]
    fn the_generated_book_exercises_the_multi_currency_law() {
        // ⛔ THE LAW WAS PROVED, ENFORCED AT THE DOOR, AND EXERCISED BY NOTHING.
        // Every posting this generator wrote carried `currency: None`, so a book
        // with three currencies in its entity master formed exactly ONE
        // conservation group and behaved as it always had.
        let d = tmp("multiccy");
        generate(&d, Shape { securities: 6, lots_per: 4, ..Shape::default() }).unwrap();
        let entries = FileBook::open(&d).unwrap().entries().unwrap();

        let seen: std::collections::BTreeSet<Option<String>> = entries
            .iter()
            .flat_map(|e| e.postings.iter())
            .map(|p| p.currency.clone())
            .collect();
        assert!(seen.len() >= 3, "every currency should appear: {seen:?}");
        assert!(!seen.contains(&None), "no leg should be untyped: {seen:?}");

        // ⛔ AND THE EXCHANGE HAS FOUR LEGS. Two would conserve neither currency
        // — `Ratio.Chart.Dimensions.a_two_leg_exchange_conserves_neither_
        // currency` — and the door would refuse the entry, which is the law
        // doing its job.
        let fx: Vec<_> = entries.iter().filter(|e| e.id.starts_with("fx-")).collect();
        assert_eq!(fx.len(), 2, "one exchange per non-base currency");
        for e in &fx {
            assert_eq!(e.postings.len(), 4, "{}: {:?}", e.id, e.postings);
            assert!(e.is_balanced(), "{} does not conserve", e.id);
        }
    }

    #[test]
    fn a_two_leg_exchange_is_refused_at_the_door() {
        // The hole this closed, tried against a real book: numerically zero,
        // and out by the whole amount in both currencies.
        let d = tmp("twoleg");
        generate(&d, Shape { securities: 2, lots_per: 2, ..Shape::default() }).unwrap();
        let mut b = FileBook::open(&d).unwrap();
        let c = b.active().unwrap().unwrap();
        let err = b
            .append(&JournalEntry {
                id: "bad-fx".into(),
                memo: "an exchange that is not one".into(),
                config: c,
                postings: vec![
                    PostingRecord::of_currency(2, -10_000, "USD"),
                    PostingRecord::of_currency(2, 10_000, "EUR"),
                ],
                trade_date: None,
                announcement: None,
            })
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("does not conserve"), "{msg}");
    }

    #[test]
    fn every_generated_holding_can_actually_be_valued() {
        // ⚠ THE TEST THAT KEEPS THE GENERATOR HONEST. A 3-for-2 against a lot
        // holding an odd number of units refuses — cash in lieu, which realizes
        // a gain and is a posting no configuration here declares. Generating
        // books that cannot be valued would measure nothing at all.
        let d = tmp("valuable");
        generate(&d, Shape { securities: 20, currencies: 2, turnover: 3, lots_per: 10, open_actions: 6, capital_txns: 2, seed: 9, method: ratio_rules::LotMethod::Fifo })
            .unwrap();
        // ⚠ Through `of_book` rather than `rebuild`, so the lots are relieved
        // under the method the generated configuration DECLARES rather than one
        // this test picked. A generator that writes `lot_method` and a test that
        // folds under some other method measure two different books.
        let p = ratio_project::Projection::of_book(&d).unwrap();
        for i in 0..20i64 {
            p.units_as_of(ratio_rules::UNDECLARED_VIEW, 1, &ticker(i), "2026-06-30")
                .unwrap_or_else(|e| panic!("{} cannot be valued: {e:#}", ticker(i)));
        }
    }

    #[test]
    fn a_generated_book_says_what_generated_it() {
        // ⛔ EVERY DIAL, NOT JUST THE TWO A HEADLINE QUOTES. `ratio bench --fold`
        // marks one security at a time and translates one currency at a time off
        // this record, and `method` decides the realized gain — so a field that
        // failed to round-trip would produce a confident report about a fund
        // nobody generated.
        let s = Shape {
            securities: 7,
            currencies: 2,
            turnover: 3,
            lots_per: 5,
            open_actions: 1,
            capital_txns: 2,
            seed: 42,
            method: ratio_rules::LotMethod::Hifo,
        };
        let d = tmp("shape-roundtrip");
        generate(&d, s).unwrap();
        let back = shape_of(&d).unwrap();

        assert_eq!(back.securities, s.securities);
        assert_eq!(back.currencies, s.currencies);
        assert_eq!(back.lots_per, s.lots_per);
        assert_eq!(back.turnover, s.turnover);
        assert_eq!(back.open_actions, s.open_actions);
        assert_eq!(back.capital_txns, s.capital_txns);
        assert_eq!(back.seed, s.seed);
        // ⭐ The dial the demo exists to show: two funds from one seed differing
        // only here realize different gains. A method that did not survive the
        // round trip would fold the book back under FIFO and say so silently.
        assert_eq!(back.method, s.method);
    }

    #[test]
    fn a_book_nobody_generated_refuses_to_state_a_shape() {
        // ⛔ REFUSES RATHER THAN DEFAULTING. `Shape::default()` is a real fund —
        // 500 securities, three currencies — so a `unwrap_or_default` here would
        // hand `bench --fold` a shape for a book that has none, and it would mark
        // five hundred tickers that do not exist and report the result.
        let d = tmp("no-shape");
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let err = shape_of(&d).unwrap_err();
        let said = format!("{err:#}");
        assert!(said.contains("did not come from `ratio gen`"), "{said}");
    }
}
