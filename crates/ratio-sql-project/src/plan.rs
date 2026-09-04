//! Stage E read plans, denoted the way `Pg.Rel.Semantics` denotes them.
//!
//! # Why this module exists
//!
//! #234 applied the schema. #235 wired console/API reads. Neither had a
//! rewrite that was a theorem: `lots_of` built a SQL string. This is the
//! store's planner surface — the same `Plan` / `Pred` / `denote` / `≡` as
//! `lean/Pg/Rel/Semantics.lean`, plus the Stage E catalog
//! `lean/Ratio/Sql/Pushdown.lean` instantiates.
//!
//! # ⛔ Two scans, not one join that can return empty
//!
//! A missing watermark is a refuse. An INNER JOIN of watermark ⋉ lots
//! that yields no rows looks like a fund that sold everything.
//! `an_empty_pin_is_not_an_empty_holding`. [`sql_of`] will emit a
//! filter-over-scan and nothing else.
//!
//! # ⛔ Relief is not a rewrite
//!
//! `ORDER BY seq` is display order. HIFO is `relieve_by`.
//! [`push_below_outer_join`] refuses — that is
//! `pushdown_below_an_outer_join_is_unsound`, and the Stage E witness is
//! a null `acquired` on an unmatched watermark row.
//!
//! # What this is not
//!
//! The measured 20M-lot claim. That stays #159. `Ratio.Exec` still holds:
//! a database does not change the IO floor.

use anyhow::{bail, Result};

/// A value. `null` is not a number and is not comparable to one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Val {
    Null,
    Num(i64),
}

/// A row is positional; a table is a BAG of rows.
pub type Row = Vec<Val>;
pub type Table = Vec<Row>;

/// SQL's three-valued logic. `WHERE` keeps only [`Three::Yes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Three {
    Yes,
    No,
    Unknown,
}

/// A predicate over a row, by column position.
///
/// `EqStr` is the Rust door for text keys (`book_id`, `view_id`,
/// `instrument`, digest). The Lean denotation encodes those as `eqNum`.
/// Both use the same three-valued rule: compare with null is unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pred {
    IsNull { col: usize },
    EqNum { col: usize, n: i64 },
    EqStr { col: usize, s: String },
    And(Box<Pred>, Box<Pred>),
    Not(Box<Pred>),
}

impl Pred {
    pub fn is_null(col: usize) -> Self {
        Self::IsNull { col }
    }

    pub fn eq_num(col: usize, n: i64) -> Self {
        Self::EqNum { col, n }
    }

    pub fn eq_str(col: usize, s: impl Into<String>) -> Self {
        Self::EqStr { col, s: s.into() }
    }

    pub fn and(a: Pred, b: Pred) -> Self {
        Self::And(Box::new(a), Box::new(b))
    }

    pub fn not(a: Pred) -> Self {
        Self::Not(Box::new(a))
    }

    /// Greatest column this predicate reads. Used as the left-only test.
    pub fn max_col(&self) -> usize {
        match self {
            Self::IsNull { col } | Self::EqNum { col, .. } | Self::EqStr { col, .. } => *col,
            Self::And(a, b) => a.max_col().max(b.max_col()),
            Self::Not(a) => a.max_col(),
        }
    }

    /// `hp` of `pushdown_into_the_preserved_side_is_sound`: every column
    /// this predicate reads sits in the left prefix.
    pub fn reads_only_left(&self, left_width: usize) -> bool {
        self.max_col() < left_width
    }
}

/// Plan nodes. Same four as `Pg.Rel.Plan`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    Scan(String),
    Filter { p: Pred, child: Box<Plan> },
    Union { a: Box<Plan>, b: Box<Plan> },
    LeftJoin {
        a: Box<Plan>,
        b: Box<Plan>,
        right_width: usize,
    },
}

impl Plan {
    pub fn scan(t: impl Into<String>) -> Self {
        Self::Scan(t.into())
    }

    pub fn filter(p: Pred, child: Plan) -> Self {
        Self::Filter {
            p,
            child: Box::new(child),
        }
    }

    pub fn union(a: Plan, b: Plan) -> Self {
        Self::Union {
            a: Box::new(a),
            b: Box::new(b),
        }
    }

    pub fn left_join(a: Plan, b: Plan, right_width: usize) -> Self {
        Self::LeftJoin {
            a: Box::new(a),
            b: Box::new(b),
            right_width,
        }
    }
}

/// Stage E table names. One watermark, three children.
pub const WATERMARK: &str = "projection_watermark";
pub const LOTS: &str = "lots";
pub const POSITIONS: &str = "positions";
pub const AGGREGATES: &str = "aggregates";

/// `lots` row width. `acquired` is column 7 and may be null (unset).
pub const LOTS_WIDTH: usize = 8;
/// `projection_watermark` row width: book, prefix, digest.
pub const WATERMARK_WIDTH: usize = 3;

fn cell(r: &Row, i: usize) -> Val {
    r.get(i).cloned().unwrap_or(Val::Null)
}

/// ⛔ COMPARING WITH NULL IS UNKNOWN, NOT FALSE.
pub fn eval_pred(p: &Pred, r: &Row) -> Three {
    match p {
        Pred::IsNull { col } => match cell(r, *col) {
            Val::Null => Three::Yes,
            Val::Num(_) => Three::No,
        },
        Pred::EqNum { col, n } => match cell(r, *col) {
            Val::Null => Three::Unknown,
            Val::Num(m) => {
                if m == *n {
                    Three::Yes
                } else {
                    Three::No
                }
            }
        },
        Pred::EqStr { col, s: _ } => match cell(r, *col) {
            // ⚠ THE LEAN DENOTATION HAS NO STRINGS. A text key is `eqNum`
            // there. Here a null is still unknown; a number is not a
            // string, so the comparison is no — never a silent yes.
            Val::Null => Three::Unknown,
            Val::Num(_) => Three::No,
        },
        Pred::And(a, b) => match (eval_pred(a, r), eval_pred(b, r)) {
            (Three::No, _) | (_, Three::No) => Three::No,
            (Three::Yes, Three::Yes) => Three::Yes,
            _ => Three::Unknown,
        },
        Pred::Not(a) => match eval_pred(a, r) {
            Three::Yes => Three::No,
            Three::No => Three::Yes,
            Three::Unknown => Three::Unknown,
        },
    }
}

fn joins(r: &Row, s: &Row) -> bool {
    match (cell(r, 0), cell(s, 0)) {
        (Val::Num(a), Val::Num(b)) => a == b,
        _ => false,
    }
}

/// What a plan MEANS. `WHERE` keeps only [`Three::Yes`].
pub fn denote(db: &dyn Fn(&str) -> Table, plan: &Plan) -> Table {
    match plan {
        Plan::Scan(t) => db(t),
        Plan::Filter { p, child } => denote(db, child)
            .into_iter()
            .filter(|r| eval_pred(p, r) == Three::Yes)
            .collect(),
        Plan::Union { a, b } => {
            let mut out = denote(db, a);
            out.extend(denote(db, b));
            out
        }
        Plan::LeftJoin { a, b, right_width } => {
            let right = denote(db, b);
            denote(db, a)
                .into_iter()
                .flat_map(|r| {
                    let matched: Table = right.iter().filter(|s| joins(&r, s)).cloned().collect();
                    let sides: Table = if matched.is_empty() {
                        vec![vec![Val::Null; *right_width]]
                    } else {
                        matched
                    };
                    sides.into_iter().map(move |s| {
                        let mut row = r.clone();
                        row.extend(s);
                        row
                    })
                })
                .collect()
        }
    }
}

/// Sound rewrite: filter on the preserved side moves below the join.
///
/// `pushdown_into_the_preserved_side_is_sound`. Returns [`None`] when the
/// predicate reads a right-side column — that is the other theorem, and
/// this function will not pretend it is this one.
pub fn push_into_preserved(plan: &Plan, left_width: usize) -> Option<Plan> {
    let Plan::Filter { p, child } = plan else {
        return None;
    };
    let Plan::LeftJoin { a, b, right_width } = child.as_ref() else {
        return None;
    };
    if !p.reads_only_left(left_width) {
        return None;
    }
    Some(Plan::left_join(
        Plan::filter(p.clone(), a.as_ref().clone()),
        b.as_ref().clone(),
        *right_width,
    ))
}

/// Unsound rewrite: filter on the NULL-extended side moved below the join.
///
/// ⛔ ALWAYS A REFUSE. `pushdown_below_an_outer_join_is_unsound` is a
/// counterexample, not a rewrite that sometimes works. Stage E's witness
/// is `acquired` on watermark ⋉ lots.
pub fn push_below_outer_join(_plan: &Plan) -> Result<Plan> {
    bail!(
        "pushdown_below_an_outer_join_is_unsound — a filter on the NULL-extended \
         side of a LEFT JOIN cannot move below it. Stage E acquired / instrument \
         predicates stay above the join, or on an inner lots scan after the pin \
         has already refused. `Pg.Rel.Semantics`"
    )
}

/// Pin check: filter the watermark scan. Empty is a refuse, not lots.
pub fn pin_plan(book: &str, prefix: i64, digest: &str) -> Plan {
    Plan::filter(
        Pred::and(
            Pred::eq_str(0, book),
            Pred::and(Pred::eq_num(1, prefix), Pred::eq_str(2, digest)),
        ),
        Plan::scan(WATERMARK),
    )
}

/// Lots of one holding, already pushed into the lots scan.
pub fn lots_plan(book: &str, view: &str, dim: i64, instrument: &str) -> Plan {
    Plan::filter(
        Pred::and(
            Pred::eq_str(0, book),
            Pred::and(
                Pred::eq_str(1, view),
                Pred::and(Pred::eq_num(2, dim), Pred::eq_str(3, instrument)),
            ),
        ),
        Plan::scan(LOTS),
    )
}

/// Positions of one view, already pushed into the positions scan.
pub fn positions_plan(book: &str, view: &str) -> Plan {
    Plan::filter(
        Pred::and(Pred::eq_str(0, book), Pred::eq_str(1, view)),
        Plan::scan(POSITIONS),
    )
}

/// Aggregates of one view, already pushed into the aggregates scan.
pub fn aggregates_plan(book: &str, view: &str) -> Plan {
    Plan::filter(
        Pred::and(Pred::eq_str(0, book), Pred::eq_str(1, view)),
        Plan::scan(AGGREGATES),
    )
}

fn col_name(cols: &[&str], i: usize) -> Result<String> {
    cols.get(i)
        .map(|c| (*c).to_string())
        .ok_or_else(|| {
            anyhow::anyhow!("plan column {i} is past the table width {}", cols.len())
        })
}

const WATERMARK_COLS: [&str; 3] = ["book_id", "journal_prefix", "journal_digest"];
const LOTS_COLS: [&str; 8] = [
    "book_id",
    "view_id",
    "dim",
    "instrument",
    "seq",
    "units",
    "cost",
    "acquired",
];
const POSITIONS_COLS: [&str; 6] = [
    "book_id",
    "view_id",
    "dim",
    "instrument",
    "cost",
    "quantity",
];
const AGGREGATES_COLS: [&str; 7] = [
    "book_id",
    "view_id",
    "dim",
    "currency",
    "debit",
    "credit",
    "postings",
];

fn cols_for(table: &str) -> Result<&'static [&'static str]> {
    Ok(match table {
        WATERMARK => &WATERMARK_COLS,
        LOTS => &LOTS_COLS,
        POSITIONS => &POSITIONS_COLS,
        AGGREGATES => &AGGREGATES_COLS,
        other => bail!(
            "plan scans {other:?}, which is not a Stage E table. The catalog \
             is projection_watermark / lots / positions / aggregates"
        ),
    })
}

fn pred_sql(p: &Pred, cols: &[&str]) -> Result<String> {
    match p {
        Pred::IsNull { col } => Ok(format!("{} IS NULL", col_name(cols, *col)?)),
        Pred::EqNum { col, n } => Ok(format!("{} = {n}", col_name(cols, *col)?)),
        Pred::EqStr { col, s } => Ok(format!("{} = {}", col_name(cols, *col)?, lit(s))),
        Pred::And(a, b) => Ok(format!(
            "{} AND {}",
            pred_sql(a, cols)?,
            pred_sql(b, cols)?
        )),
        Pred::Not(a) => Ok(format!("NOT ({})", pred_sql(a, cols)?)),
    }
}

fn lit(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Peel `filter*` off a `scan`. Anything else is a join or a union, and
/// those are denote/rewrite surface — not a SQL string the store will run.
fn scan_filters<'a>(plan: &'a Plan) -> Result<(&'a str, Vec<&'a Pred>)> {
    let mut p = plan;
    let mut preds = Vec::new();
    loop {
        match p {
            Plan::Filter { p: pred, child } => {
                preds.push(pred);
                p = child;
            }
            Plan::Scan(t) => {
                preds.reverse();
                return Ok((t.as_str(), preds));
            }
            Plan::LeftJoin { .. } => {
                bail!(
                    "Stage E reads are two scans. A LEFT JOIN that can return \
                     empty lots when the watermark is missing is the silent \
                     empty-fund. `an_empty_pin_is_not_an_empty_holding`. \
                     `//tla:unpinned_projection_check`"
                )
            }
            Plan::Union { .. } => {
                bail!("Stage E reads are not a UNION of snapshots")
            }
        }
    }
}

/// SQL for a filter-over-scan plan. No join. No relief `ORDER BY seq`.
pub fn sql_of(plan: &Plan) -> Result<String> {
    let (table, preds) = scan_filters(plan)?;
    let cols = cols_for(table)?;
    let mut sql = format!("SELECT * FROM {table}");
    if !preds.is_empty() {
        let wheres: Result<Vec<String>> = preds.iter().map(|p| pred_sql(p, cols)).collect();
        sql.push_str(" WHERE ");
        sql.push_str(&wheres?.join(" AND "));
    }
    Ok(sql)
}

/// Lots read the store already runs: projected columns, pin is a
/// separate plan, `ORDER BY seq` is display — not FIFO relief.
pub fn lots_select_sql(book: &str, view: &str, dim: i64, instrument: &str) -> Result<String> {
    let plan = lots_plan(book, view, dim, instrument);
    let from = sql_of(&plan)?;
    // ⚠ ORDER BY seq IS DISPLAY ORDER, matching `SqlProjection::lots_of`.
    // Relief does not trust this. `seq_scan_is_not_hifo`.
    Ok(format!(
        "SELECT seq, units, cost, COALESCE(acquired::text, '') FROM ({from}) AS lots_read \
         ORDER BY seq"
    ))
}

pub fn positions_select_sql(book: &str, view: &str) -> Result<String> {
    let from = sql_of(&positions_plan(book, view))?;
    Ok(format!(
        "SELECT dim, instrument, cost, quantity FROM ({from}) AS positions_read"
    ))
}

pub fn aggregates_select_sql(book: &str, view: &str) -> Result<String> {
    let from = sql_of(&aggregates_plan(book, view))?;
    Ok(format!(
        "SELECT dim, currency, debit, credit, postings FROM ({from}) AS aggregates_read"
    ))
}

pub fn watermark_select_sql(book: &str) -> Result<String> {
    let plan = Plan::filter(Pred::eq_str(0, book), Plan::scan(WATERMARK));
    let from = sql_of(&plan)?;
    Ok(format!(
        "SELECT journal_prefix, journal_digest FROM ({from}) AS pin_read"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lots_db(rows: Table) -> impl Fn(&str) -> Table {
        move |t: &str| {
            if t == LOTS {
                rows.clone()
            } else {
                Vec::new()
            }
        }
    }

    fn cheap_then_dear() -> Table {
        vec![
            vec![
                Val::Num(1),
                Val::Num(0),
                Val::Num(1),
                Val::Num(1),
                Val::Num(0),
                Val::Num(10),
                Val::Num(1000),
                Val::Null,
            ],
            vec![
                Val::Num(1),
                Val::Num(0),
                Val::Num(1),
                Val::Num(1),
                Val::Num(1),
                Val::Num(10),
                Val::Num(10000),
                Val::Null,
            ],
        ]
    }

    #[test]
    fn push_into_preserved_agrees_with_denote_on_a_typed_pin() {
        // ⭐ THE SOUND REWRITE. Pin on cols 0,1,2 of a width-3 watermark
        // row. `pushdown_into_the_preserved_side_is_sound`.
        let pin = Pred::and(
            Pred::eq_num(0, 1),
            Pred::and(Pred::eq_num(1, 2), Pred::eq_num(2, 3)),
        );
        let above = Plan::filter(
            pin.clone(),
            Plan::left_join(Plan::scan(WATERMARK), Plan::scan(LOTS), LOTS_WIDTH),
        );
        let pushed = push_into_preserved(&above, WATERMARK_WIDTH)
            .expect("a pin reads only the watermark prefix");
        assert_eq!(
            pushed,
            Plan::left_join(
                Plan::filter(pin, Plan::scan(WATERMARK)),
                Plan::scan(LOTS),
                LOTS_WIDTH
            )
        );

        let db = |t: &str| match t {
            WATERMARK => vec![
                vec![Val::Num(1), Val::Num(2), Val::Num(3)],
                vec![Val::Num(9), Val::Num(9), Val::Num(9)],
            ],
            LOTS => cheap_then_dear(),
            _ => Vec::new(),
        };
        assert_eq!(denote(&db, &above), denote(&db, &pushed));
        assert_eq!(denote(&db, &above).len(), 2, "book 1's two lots survive");
    }

    #[test]
    fn acquired_below_an_outer_join_disagrees_with_the_filter_above() {
        // ⭐ THE COUNTEREXAMPLE. Watermark row, no lots partner, filter
        // `acquired = 5` (joined col 10). Above drops it (UNKNOWN).
        // Below keeps the padded row. `stage_e_acquired_below_join_is_unsound`.
        let p = Pred::eq_num(WATERMARK_WIDTH + 7, 5);
        let above = Plan::filter(
            p.clone(),
            Plan::left_join(Plan::scan(WATERMARK), Plan::scan(LOTS), LOTS_WIDTH),
        );
        let below = Plan::left_join(
            Plan::scan(WATERMARK),
            Plan::filter(p, Plan::scan(LOTS)),
            LOTS_WIDTH,
        );
        let db = |t: &str| {
            if t == WATERMARK {
                vec![vec![Val::Num(1), Val::Num(0), Val::Num(0)]]
            } else {
                Vec::new()
            }
        };
        assert_ne!(
            denote(&db, &above),
            denote(&db, &below),
            "the two plans must differ — that is what says the rewrite is unsound"
        );
        assert!(denote(&db, &above).is_empty(), "UNKNOWN acquired is dropped");
        assert_eq!(
            denote(&db, &below).len(),
            1,
            "pushing below preserves the unmatched watermark"
        );
        assert!(
            push_into_preserved(&above, WATERMARK_WIDTH).is_none(),
            "acquired is not left-only — the sound rewriter must refuse it"
        );
    }

    #[test]
    fn push_below_outer_join_is_a_refuse_not_a_rewrite() {
        let plan = Plan::filter(
            Pred::eq_num(10, 5),
            Plan::left_join(Plan::scan(WATERMARK), Plan::scan(LOTS), LOTS_WIDTH),
        );
        let err = push_below_outer_join(&plan).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("pushdown_below_an_outer_join_is_unsound"), "{msg}");
        assert!(msg.contains("Pg.Rel.Semantics"), "{msg}");
    }

    #[test]
    fn seq_scan_is_not_hifo() {
        // ⭐ SAME WITNESS AS THE LEAN THEOREM. Cheap seq 0, dear seq 1.
        // A saboteur that treated ORDER BY seq as HIFO would equate these.
        let db = lots_db(cheap_then_dear());
        let fifo = denote(&db, &Plan::filter(Pred::eq_num(4, 0), Plan::scan(LOTS)));
        let hifo = denote(&db, &Plan::filter(Pred::eq_num(6, 10000), Plan::scan(LOTS)));
        assert_eq!(fifo[0][6], Val::Num(1000));
        assert_eq!(hifo[0][6], Val::Num(10000));
        assert_ne!(fifo, hifo);
    }

    #[test]
    fn an_empty_pin_is_not_an_empty_holding() {
        let db = |t: &str| {
            if t == LOTS {
                cheap_then_dear()
            } else {
                Vec::new()
            }
        };
        let pin = Plan::filter(
            Pred::and(
                Pred::eq_num(0, 1),
                Pred::and(Pred::eq_num(1, 2), Pred::eq_num(2, 3)),
            ),
            Plan::scan(WATERMARK),
        );
        let holding = Plan::filter(
            Pred::and(
                Pred::eq_num(0, 1),
                Pred::and(
                    Pred::eq_num(1, 0),
                    Pred::and(Pred::eq_num(2, 1), Pred::eq_num(3, 1)),
                ),
            ),
            Plan::scan(LOTS),
        );
        assert!(denote(&db, &pin).is_empty());
        assert_eq!(denote(&db, &holding).len(), 2);
    }

    #[test]
    fn sql_of_a_lots_plan_is_a_filter_over_the_lots_scan() {
        let sql = lots_select_sql("fund", "book", 1, "vti").unwrap();
        assert!(sql.contains("FROM lots"), "{sql}");
        assert!(sql.contains("book_id = 'fund'"), "{sql}");
        assert!(sql.contains("view_id = 'book'"), "{sql}");
        assert!(sql.contains("dim = 1"), "{sql}");
        assert!(sql.contains("instrument = 'vti'"), "{sql}");
        assert!(
            !sql.to_ascii_lowercase().contains("join"),
            "two scans, not a join that can return empty: {sql}"
        );
        assert!(
            sql.contains("ORDER BY seq"),
            "display order is named — and the next assert is why it is not relief: {sql}"
        );
        assert!(
            !sql.to_ascii_lowercase().contains("lot_method"),
            "the plan must not invent a Method column: {sql}"
        );
    }

    #[test]
    fn sql_of_a_join_is_a_refuse() {
        let plan = Plan::left_join(Plan::scan(WATERMARK), Plan::scan(LOTS), LOTS_WIDTH);
        let err = sql_of(&plan).unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("two scans"), "{msg}");
        assert!(msg.contains("an_empty_pin_is_not_an_empty_holding"), "{msg}");
    }

    #[test]
    fn a_predicate_does_not_partition_a_null() {
        // The Lean theorem's witness, so a denote that collapsed UNKNOWN
        // into false would reassemble the table and this would go green
        // for the wrong reason.
        let db = |_t: &str| vec![vec![Val::Null]];
        let p = Pred::eq_num(0, 1);
        let halves = Plan::union(
            Plan::filter(p.clone(), Plan::scan("t")),
            Plan::filter(Pred::not(p), Plan::scan("t")),
        );
        assert_ne!(denote(&db, &halves), denote(&db, &Plan::scan("t")));
        assert!(denote(&db, &halves).is_empty());
    }
}
