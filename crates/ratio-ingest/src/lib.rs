//! ratio-ingest — the data plane: a counterparty's file becomes canonical facts.
//!
//! Four steps, and only the first one touches the world:
//!
//! ```text
//!   Delivery   bytes + origin, hashed on arrival        (I/O)
//!   extract    bytes → rows                             (pure)
//!   project    row → claims + values, via a template     (pure)
//!   resolve    claims → entities, against the master     (pure)
//! ```
//!
//! ⛔ THE ONE DESIGN DECISION EVERYTHING ELSE FOLLOWS FROM: a fact stores its
//! CLAIMS, not the entity they resolved to. Resolution is a fold over the fact
//! log and the entity master — the same relationship the trial balance has to
//! the journal, and NAV has to both.
//!
//! That is what makes a pending fact clear on its own. When the missing
//! instrument is finally added, nothing is notified, no queue is drained and no
//! file is re-ingested: the next derivation simply resolves. Storing the
//! resolved id instead would be faster and would be wrong — it goes stale the
//! moment the master changes, and it makes "why did this resolve?"
//! unanswerable afterwards.
//!
//! A template is authored as TOML and RENDERED for reading, exactly like a
//! posting rule. There is no bespoke parser: the readable form in the
//! architecture note is what `render` produces, not a second syntax to keep in
//! step with the first.

mod generated;
/// The Lean-authored core: `ResolutionKind`, `resolution_of_count`,
/// `rung_satisfied`, `claim_holds`, `scale_for`, `minor_units`.
///
/// ⛔ These are NOT a Rust copy of the Lean. They are emitted from
/// `//lean:Ratio/Ingest/Emit.lean` into this crate's `src/generated.rs` as one
/// Bazel edge, so there is nothing to keep in step. The folds below establish
/// the facts those functions decide on, and `Ratio.Ingest` proves the decisions
/// depend on nothing else.
pub use generated::*;

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

pub use ratio_common::parse_minor;

// ─── the master ────────────────────────────────────────────────────────────

/// What kind of thing an entity is. Deliberately small: the slice needs the two
/// that a trade file refers to, and the closure beyond it (issuers, custodians,
/// agents) is more of the same shape rather than a different one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Instrument,
    Counterparty,
}

impl EntityKind {
    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Instrument => "instrument",
            EntityKind::Counterparty => "counterparty",
        }
    }
}

/// One entity in the master.
///
/// Identifiers are PLURAL and none of them is authoritative — an instrument may
/// carry an ISIN, a CUSIP, a ticker and an internal code, and different
/// counterparties will send different ones. So attributes are a map rather than
/// a struct with an `isin` field, and matching is over whatever is present.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub kind: EntityKind,
    pub display_name: String,
    /// `isin` → `GB00B03MLX29`, `ticker` → `AAPL`, `exchange` → `XNAS`, …
    #[serde(default)]
    pub attributes: BTreeMap<String, String>,
}

impl Entity {
    /// Whether every claim in the set holds against this entity.
    ///
    /// An attribute the entity does not carry is NOT a match. Absence is not
    /// agreement: an instrument with no CUSIP recorded does not match a row
    /// claiming a CUSIP, it simply says nothing, and treating silence as assent
    /// is how one instrument comes to answer for another.
    fn satisfies(&self, claims: &[Claim]) -> bool {
        // The fold establishes the two facts; the EMITTED `rung_satisfied`
        // decides. `Ratio.Ingest.rung_satisfied_agrees` proves they are the
        // same function, and `empty_rung_matches_nothing` is why the guard is
        // there at all — `all` on an empty list is `true`, so without it a
        // ladder that read no columns would match every entity in the master.
        rung_satisfied(
            claims.is_empty(),
            claims.iter().all(|c| self.carries(c)),
        )
    }

    /// Does the entity carry this exact claim?
    ///
    /// Silence is not assent: an attribute the entity does not record is not
    /// agreement with a claim about it. That is the emitted `claim_holds`,
    /// which is `recorded && values_equal` and nothing else.
    fn carries(&self, c: &Claim) -> bool {
        let recorded = self.attributes.get(&c.attr);
        claim_holds(
            recorded.is_some(),
            recorded.is_some_and(|v| v.eq_ignore_ascii_case(&c.value)),
        )
    }
}

// ─── claims and facts ──────────────────────────────────────────────────────

/// One assertion of identity read off a row: `isin = GB00B03MLX29`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    /// Named `attr` to match `Ratio.Ingest.Claim`, where `attribute` is a Lean
    /// command keyword. The proof and the running code say the same word.
    pub attr: String,
    pub value: String,
}

/// A reference to an entity, as READ rather than as resolved.
///
/// `rungs` is the identity ladder from the template with the row's values
/// substituted — the first rung whose columns were all present and non-empty
/// comes first. Resolution walks it in order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub rungs: Vec<Vec<Claim>>,
}

/// Where a fact came from, all the way back to the bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// SHA-256 of the delivered bytes.
    pub delivery: String,
    /// 1-based, counting the header as row 1 — what a person sees in a
    /// spreadsheet, not what an array index says.
    pub row: i64,
    /// The template that mapped it, by the digest of the configuration holding
    /// it. A fact can always name the mapping that produced it.
    pub template: String,
    pub template_id: String,
    pub received: i64,
}

/// A typed value read off a row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Value {
    /// Minor units. Money is NEVER a float here or anywhere else.
    Money { minor: i64, currency: String },
    Decimal { minor: i64 },
    Text { text: String },
    Date { iso: String },
}

impl Value {
    pub fn as_minor(&self) -> Option<i64> {
        match self {
            Value::Money { minor, .. } | Value::Decimal { minor } => Some(*minor),
            _ => None,
        }
    }
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text { text } => Some(text),
            Value::Date { iso } => Some(iso),
            _ => None,
        }
    }
}

/// A canonical fact: what a row asserted, before anything was resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fact {
    /// Stable and derived from provenance, so re-ingesting the same bytes under
    /// the same template produces the same fact ids rather than duplicates.
    pub id: String,
    pub kind: String,
    /// The row's own reference, as the counterparty knows it.
    pub reference: String,
    pub entities: BTreeMap<String, EntityRef>,
    pub values: BTreeMap<String, Value>,
    pub provenance: Provenance,
}

// ─── resolution ────────────────────────────────────────────────────────────

/// What resolving one reference produced.
///
/// THREE outcomes, not two. "Absent" is a reference-data gap that someone fills
/// by adding an instrument; "ambiguous" is a data-quality problem in the master
/// that someone fixes by de-duplicating or by narrowing the ladder. Collapsing
/// them into "failed" hides the second one, which is the one that silently
/// books a trade against the wrong security.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolution {
    Resolved { entity: String },
    Absent { tried: Vec<Vec<Claim>> },
    Ambiguous { rung: Vec<Claim>, candidates: Vec<String> },
}

impl Resolution {
    pub fn is_resolved(&self) -> bool {
        matches!(self, Resolution::Resolved { .. })
    }
}

/// Walk the identity ladder against the master.
///
/// The FIRST rung that matches anything decides the outcome — including when
/// what it matches is two entities. Falling through an ambiguous rung to a
/// weaker one that happens to match uniquely would turn a data-quality problem
/// into a confident wrong answer.
pub fn resolve(reference: &EntityRef, master: &[Entity]) -> Resolution {
    for rung in &reference.rungs {
        let hits: Vec<String> = master
            .iter()
            .filter(|e| e.kind == reference.kind && e.satisfies(rung))
            .map(|e| e.id.clone())
            .collect();
        // The fold counts; the EMITTED `resolution_of_count` decides.
        // `Ratio.Ingest.kind_is_determined_by_count` proves the outcome depends
        // on nothing but this count, which is what makes a hand-written loop
        // safe here — the two cannot disagree, because only one of them is
        // deciding anything.
        match resolution_of_count(hits.len() as i64) {
            ResolutionKind::Absent => continue,
            ResolutionKind::Resolved => {
                return Resolution::Resolved { entity: hits[0].clone() }
            }
            ResolutionKind::Ambiguous => {
                return Resolution::Ambiguous { rung: rung.clone(), candidates: hits }
            }
        }
    }
    Resolution::Absent { tried: reference.rungs.clone() }
}

/// A fact with every reference resolved against a given master.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub fact: Fact,
    pub entities: BTreeMap<String, Resolution>,
}

impl Resolved {
    /// Admissible when every reference resolved. A fact that is not admissible
    /// does not post — it blocks, in the same queue as a reconciliation break,
    /// because a figure missing an input is not a figure.
    pub fn is_admissible(&self) -> bool {
        self.entities.values().all(Resolution::is_resolved)
    }

    /// The resolved entity behind one reference, if it resolved.
    pub fn entity(&self, name: &str) -> Option<&str> {
        match self.entities.get(name) {
            Some(Resolution::Resolved { entity }) => Some(entity.as_str()),
            _ => None,
        }
    }

    /// Why this fact is not admissible, in a sentence an operator can act on.
    pub fn blocker(&self) -> Option<String> {
        self.entities.iter().find_map(|(name, r)| match r {
            Resolution::Resolved { .. } => None,
            Resolution::Absent { tried } => Some(format!(
                "no {name} matches {}",
                tried.iter().map(|r| render_rung(r)).collect::<Vec<_>>().join(" or "),
            )),
            Resolution::Ambiguous { rung, candidates } => Some(format!(
                "{} {}s match {} — the master needs narrowing, not a new record",
                candidates.len(),
                name,
                render_rung(rung),
            )),
        })
    }
}

fn render_rung(rung: &[Claim]) -> String {
    rung.iter()
        .map(|c| format!("{} {}", c.attr, c.value))
        .collect::<Vec<_>>()
        .join(" within ")
}

/// Resolve every fact against the master. A FOLD, not a queue drain — this is
/// what makes a pending fact clear without being touched.
pub fn resolve_all(facts: &[Fact], master: &[Entity]) -> Vec<Resolved> {
    facts
        .iter()
        .map(|f| Resolved {
            fact: f.clone(),
            entities: f
                .entities
                .iter()
                .map(|(name, r)| (name.clone(), resolve(r, master)))
                .collect(),
        })
        .collect()
}

// ─── the template ──────────────────────────────────────────────────────────

/// How to read the delivered bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reader {
    Csv,
}

/// What to do when a reference matches nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Absent {
    /// Hold the fact. It clears when the entity appears.
    #[default]
    Pend,
    /// Refuse the row outright — for a reference that should never be new.
    Reject,
}

/// One rung of an identity ladder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimDecl {
    pub attribute: String,
    pub column: String,
    /// A qualifier, because some identifiers are only unique within something
    /// else: a ticker within an exchange, a broker code within a market.
    #[serde(default)]
    pub within: Option<Qualifier>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Qualifier {
    pub attribute: String,
    pub column: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityDecl {
    pub name: String,
    pub kind: EntityKind,
    #[serde(default)]
    pub absent: Absent,
    /// Tried in order. Put the identifier you trust first.
    #[serde(rename = "by")]
    pub ladder: Vec<ClaimDecl>,
}

/// How to read one column into a typed value.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "as", rename_all = "snake_case")]
pub enum ValueDecl {
    Money { column: String, currency: String },
    Decimal { column: String },
    Text { column: String },
    Date { column: String, format: String },
    /// A closed vocabulary. A value outside it is a rejected row, not a
    /// pass-through: an unrecognized side is exactly the thing that must not
    /// reach the books unnoticed.
    Enum { column: String, map: BTreeMap<String, String> },
}

impl ValueDecl {
    fn column(&self) -> &str {
        match self {
            ValueDecl::Money { column, .. }
            | ValueDecl::Decimal { column }
            | ValueDecl::Text { column }
            | ValueDecl::Date { column, .. }
            | ValueDecl::Enum { column, .. } => column,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDecl {
    pub field: String,
    #[serde(default)]
    pub optional: bool,
    #[serde(flatten)]
    pub value: ValueDecl,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactDecl {
    pub kind: String,
    /// The column holding the row's own reference.
    pub reference: String,
    /// Field name → the entity declared above that it points at.
    #[serde(default)]
    pub entities: BTreeMap<String, String>,
    #[serde(default, rename = "value")]
    pub values: Vec<FieldDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Template {
    pub id: String,
    pub reads: Reader,
    #[serde(default, rename = "entity")]
    pub entities: Vec<EntityDecl>,
    pub fact: FactDecl,
}

/// The templates in a configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateSet {
    #[serde(default, rename = "template")]
    pub templates: Vec<Template>,
}

impl TemplateSet {
    pub fn from_toml(text: &str) -> Result<Self> {
        toml::from_str(text).context("the configuration is not a valid template set")
    }
    pub fn template(&self, id: &str) -> Option<&Template> {
        self.templates.iter().find(|t| t.id == id)
    }
}

impl Template {
    /// Check a template against itself, before it maps anything.
    ///
    /// A template that names an entity its facts never use, or points a fact at
    /// an entity that was never declared, is a mapping that will fail on the
    /// first file rather than at approval — which is the wrong end.
    pub fn check(&self) -> Vec<String> {
        let mut out = Vec::new();
        let declared: BTreeSet<&str> = self.entities.iter().map(|e| e.name.as_str()).collect();
        for (field, entity) in &self.fact.entities {
            if !declared.contains(entity.as_str()) {
                out.push(format!("`{field}` points at entity `{entity}`, which is not declared"));
            }
        }
        let used: BTreeSet<&str> = self.fact.entities.values().map(String::as_str).collect();
        for e in &self.entities {
            if !used.contains(e.name.as_str()) {
                out.push(format!("entity `{}` is declared and never used", e.name));
            }
            if e.ladder.is_empty() {
                out.push(format!("entity `{}` has no identity ladder, so nothing can match it", e.name));
            }
        }
        if self.fact.reference.is_empty() {
            out.push("the fact has no reference column, so its rows cannot be cited".into());
        }
        out
    }

    /// The template as a person reads it — the form in the architecture note.
    ///
    /// Rendered rather than parsed, exactly like a posting rule: one authored
    /// syntax (TOML), one readable form derived from it, and no second grammar
    /// to keep in step.
    pub fn render(&self) -> String {
        let mut s = format!("template {} {{\n", self.id);
        s.push_str(&format!(
            "  reads      {}\n  grain      one {} per row\n",
            match self.reads {
                Reader::Csv => "csv with header",
            },
            self.fact.kind,
        ));
        for e in &self.entities {
            s.push_str(&format!("\n  entity     {}  ({})\n", e.name, e.kind.as_str()));
            for (i, c) in e.ladder.iter().enumerate() {
                let lead = if i == 0 { "by" } else { "or" };
                s.push_str(&format!("    {lead:<7}{:<7}from \"{}\"", c.attribute, c.column));
                if let Some(q) = &c.within {
                    s.push_str(&format!(" within {} from \"{}\"", q.attribute, q.column));
                }
                s.push('\n');
            }
            s.push_str(&format!(
                "    absent   {}\n",
                match e.absent {
                    Absent::Pend => "pend",
                    Absent::Reject => "reject",
                }
            ));
        }
        s.push_str(&format!("\n  fact       {}\n", self.fact.kind));
        s.push_str(&format!("    {:<12}from \"{}\"\n", "reference", self.fact.reference));
        for (field, entity) in &self.fact.entities {
            s.push_str(&format!("    {field:<12}{entity}\n"));
        }
        for v in &self.fact.values {
            s.push_str(&format!("    {:<12}from \"{}\" as ", v.field, v.value.column()));
            match &v.value {
                ValueDecl::Money { currency, .. } => s.push_str(&format!("money in \"{currency}\"")),
                ValueDecl::Decimal { .. } => s.push_str("decimal"),
                ValueDecl::Text { .. } => s.push_str("text"),
                ValueDecl::Date { format, .. } => s.push_str(&format!("date \"{format}\"")),
                ValueDecl::Enum { map, .. } => {
                    let pairs: Vec<String> =
                        map.iter().map(|(k, v)| format!("{k}: {v}")).collect();
                    s.push_str(&format!("{{ {} }}", pairs.join(", ")));
                }
            }
            if v.optional {
                s.push_str(" optional");
            }
            s.push('\n');
        }
        s.push_str("}\n");
        s
    }
}

// ─── extraction ────────────────────────────────────────────────────────────

/// A delivered file: bytes, where they came from, and when.
///
/// Hashed on arrival and never modified. Every fact cites this digest, so the
/// chain from a figure back to the counterparty's bytes is unbroken.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delivery {
    pub digest: String,
    pub origin: String,
    pub received: i64,
    pub bytes: i64,
}

/// One row, by column name.
pub type Row = BTreeMap<String, String>;

/// Split a CSV, honoring quotes. Columns are located by HEADER NAME, never by
/// position — a counterparty who adds a column should not silently shift every
/// figure one to the left.
pub fn extract_csv(text: &str) -> Result<Vec<Row>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().context("the file is empty")?;
    let cols = split_csv_line(header);
    let mut out = Vec::new();
    for line in lines {
        let f = split_csv_line(line);
        let mut row = Row::new();
        for (i, c) in cols.iter().enumerate() {
            row.insert(c.clone(), f.get(i).cloned().unwrap_or_default());
        }
        out.push(row);
    }
    Ok(out)
}

fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().map(|s| s.trim().to_string()).collect()
}

// ─── projection ────────────────────────────────────────────────────────────

/// A row the template could not map, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub row: i64,
    pub reason: String,
}

/// What one delivery produced.
#[derive(Clone, Debug, Default)]
pub struct Projection {
    pub facts: Vec<Fact>,
    /// ⚠ PER ROW, never per file. One bad row rejecting a ten-thousand-row file
    /// is the most common failure of this kind of system, and it is a choice.
    pub rejected: Vec<Rejected>,
}

/// Map rows to facts.
///
/// `config` is the digest of the configuration holding the template, so every
/// fact can name the mapping that produced it — the same way a journal entry
/// names the rules that produced its postings.
pub fn project(
    template: &Template,
    delivery: &Delivery,
    rows: &[Row],
    config: &str,
) -> Projection {
    let mut out = Projection::default();
    for (i, row) in rows.iter().enumerate() {
        let n = i as i64 + 2; // 1-based, after the header
        match project_row(template, delivery, row, config, n) {
            Ok(f) => out.facts.push(f),
            Err(e) => out.rejected.push(Rejected { row: n, reason: format!("{e:#}") }),
        }
    }
    out
}

fn project_row(
    template: &Template,
    delivery: &Delivery,
    row: &Row,
    config: &str,
    n: i64,
) -> Result<Fact> {
    let cell = |name: &str| -> String { row.get(name).cloned().unwrap_or_default() };

    let reference = cell(&template.fact.reference);
    if reference.trim().is_empty() {
        bail!("no value in the reference column {:?}", template.fact.reference);
    }

    let mut entities = BTreeMap::new();
    for (field, entity_name) in &template.fact.entities {
        let decl = template
            .entities
            .iter()
            .find(|e| &e.name == entity_name)
            .with_context(|| format!("entity {entity_name:?} is not declared"))?;

        // A rung is only usable when every column it needs has a value. A blank
        // CUSIP column must not become a claim of "CUSIP is empty", which would
        // match every instrument that also has no CUSIP recorded.
        let mut rungs = Vec::new();
        for c in &decl.ladder {
            let v = cell(&c.column);
            if v.trim().is_empty() {
                continue;
            }
            let mut claims = vec![Claim { attr: c.attribute.clone(), value: v }];
            if let Some(q) = &c.within {
                let qv = cell(&q.column);
                if qv.trim().is_empty() {
                    continue; // a qualified claim without its qualifier is not a claim
                }
                claims.push(Claim { attr: q.attribute.clone(), value: qv });
            }
            rungs.push(claims);
        }
        if rungs.is_empty() {
            bail!("nothing identifies the {field} — every column its ladder reads is empty");
        }
        entities.insert(field.clone(), EntityRef { kind: decl.kind, rungs });
    }

    let mut values = BTreeMap::new();
    for v in &template.fact.values {
        let raw = cell(v.value.column());
        if raw.trim().is_empty() {
            if v.optional {
                continue;
            }
            bail!("{} is required and column {:?} is empty", v.field, v.value.column());
        }
        let value = match &v.value {
            ValueDecl::Money { currency, .. } => Value::Money {
                minor: parse_minor(&raw).with_context(|| format!("{}", v.field))?,
                currency: cell(currency),
            },
            ValueDecl::Decimal { .. } => Value::Decimal {
                minor: parse_minor(&raw).with_context(|| format!("{}", v.field))?,
            },
            ValueDecl::Text { .. } => Value::Text { text: raw },
            ValueDecl::Date { format, .. } => Value::Date { iso: parse_date(&raw, format)? },
            ValueDecl::Enum { map, .. } => Value::Text {
                text: map
                    .get(raw.trim())
                    .cloned()
                    .with_context(|| {
                        format!(
                            "{} is {raw:?}, which is not one of {}",
                            v.field,
                            map.keys().cloned().collect::<Vec<_>>().join(", ")
                        )
                    })?,
            },
        };
        values.insert(v.field.clone(), value);
    }

    Ok(Fact {
        // Derived from provenance, so re-ingesting the same bytes under the
        // same template yields the same ids rather than a second copy.
        id: format!("{}:{n}", &delivery.digest[..16]),
        kind: template.fact.kind.clone(),
        reference,
        entities,
        values,
        provenance: Provenance {
            delivery: delivery.digest.clone(),
            row: n,
            template: config.to_string(),
            template_id: template.id.clone(),
            received: delivery.received,
        },
    })
}

/// A date in the template's format, as ISO-8601.
///
/// Only the layouts counterparties actually send. An unrecognized format is an
/// error rather than a guess — reading `03/04/2026` as the wrong one of March
/// and April is a silent off-by-a-month.
pub fn parse_date(raw: &str, format: &str) -> Result<String> {
    let t = raw.trim();
    let n: Vec<&str> = t.split(['/', '-', '.']).collect();
    let four = |s: &str| -> Result<i64> { s.parse().context("not a number") };
    let (y, m, d) = match format {
        "YYYY-MM-DD" => {
            if n.len() != 3 {
                bail!("{t:?} is not YYYY-MM-DD");
            }
            (four(n[0])?, four(n[1])?, four(n[2])?)
        }
        "MM/DD/YYYY" => {
            if n.len() != 3 {
                bail!("{t:?} is not MM/DD/YYYY");
            }
            (four(n[2])?, four(n[0])?, four(n[1])?)
        }
        "DD/MM/YYYY" => {
            if n.len() != 3 {
                bail!("{t:?} is not DD/MM/YYYY");
            }
            (four(n[2])?, four(n[1])?, four(n[0])?)
        }
        other => bail!("date format {other:?} is not one this reads"),
    };
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || !(1000..=9999).contains(&y) {
        bail!("{t:?} is not a date");
    }
    Ok(format!("{y:04}-{m:02}-{d:02}"))
}

#[cfg(test)]
mod tests;
