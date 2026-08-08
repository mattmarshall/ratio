//! Marking a book to market.
//!
//! ⛔ EVERY DECISION HERE IS PROVED IN `Ratio.Valuation`, and the names match so
//! the two can be read against each other:
//!
//! | here                | there                             |
//! |---------------------|-----------------------------------|
//! | `mark_price`        | `markPrice`                       |
//! | `market_value`      | `marketValue`                     |
//! | `mark_delta`        | `markDelta`                       |
//!
//! The one that matters most is `mark_delta`, and it takes the CARRYING value,
//! not the cost. `//tla:mark_from_cost_check` shows what the other way does:
//! the book drifts by the whole gain on every mark, and every entry it posts is
//! still balanced — so the trial balance ties while the figure is wrong, and no
//! conservation check can see it.

use anyhow::Result;

/// A price as it was observed: on a day, at an amount per unit in minor units.
///
/// The day is the ISO date the fact carried. Strings compare in date order when
/// they are ISO-8601, which is the reason the projector normalizes every date
/// to that shape rather than keeping the file's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    pub on_day: String,
    pub minor: i64,
    /// Which delivery said so, so a mark can name its evidence.
    pub delivery: String,
}

/// The price to mark at: the most recent observation on or before the day.
///
/// ⛔ NEVER A PRICE FROM AFTER THE VALUATION DATE. Using one is how a NAV comes
/// to be struck on information that did not exist when it was struck — the
/// replay would need tomorrow's file. `Ratio.Valuation.markPrice_is_not_after`.
///
/// A stale price is still a price: it is the last thing anybody observed, so it
/// is returned, and "how stale is too stale" belongs in the configuration
/// beside the other tolerances rather than here.
pub fn mark_price<'a>(as_of: &str, observed: &'a [Observation]) -> Option<&'a Observation> {
    observed
        .iter()
        .filter(|o| o.on_day.as_str() <= as_of)
        // `max_by_key` keeps the LAST maximum, so two observations on one day
        // resolve to the later-delivered one — a correction supersedes the
        // thing it corrects.
        .max_by_key(|o| o.on_day.clone())
}

/// Market value in minor units, from units in hundredths and a price in minor.
///
/// Both are scaled by a hundred, so the product is scaled by ten thousand and
/// has to come back by a hundred. When that does not divide exactly there is a
/// ROUNDING DECISION, and this refuses rather than making it — which way to
/// round is a term of an administration agreement, not a property of
/// arithmetic. `Ratio.Valuation.marketValue_is_exact`.
pub fn market_value(units_hundredths: i64, price_minor: i64) -> Option<i64> {
    let scaled = units_hundredths.checked_mul(price_minor)?;
    (scaled % 100 == 0).then_some(scaled / 100)
}

/// What a mark moves: what it is worth, less what the book holds it at.
///
/// ⛔ FROM CARRYING, NOT FROM COST. See the module note.
pub fn mark_delta(carrying: i64, market: i64) -> i64 {
    market - carrying
}

/// One position's valuation at a day.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Valuation {
    /// Worth this, and the book should move by that much.
    Marked { market: i64, delta: i64, price: i64, on_day: String, delivery: String },
    /// Nothing was observed on or before the day.
    ///
    /// ⛔ NOT A ZERO MARK. Zero says "worth what it cost"; this says "we do not
    /// know what this is worth", and a NAV struck over the first is wrong in a
    /// way nobody can see. `Ratio.Valuation.unpriced_yields_no_mark`.
    Unpriced,
    /// A price exists and the arithmetic does not land on a whole minor unit.
    Inexact { price: i64, reason: String },
}

/// Value one position.
pub fn value_position(
    as_of: &str,
    units_hundredths: i64,
    carrying: i64,
    observed: &[Observation],
) -> Valuation {
    let Some(o) = mark_price(as_of, observed) else {
        return Valuation::Unpriced;
    };
    match market_value(units_hundredths, o.minor) {
        Some(market) => Valuation::Marked {
            market,
            delta: mark_delta(carrying, market),
            price: o.minor,
            on_day: o.on_day.clone(),
            delivery: o.delivery.clone(),
        },
        None => Valuation::Inexact {
            price: o.minor,
            reason: format!(
                "units x price is not a whole number of minor units, so posting it would \
                 take a rounding decision the configuration has not declared"
            ),
        },
    }
}

/// The observations a book has recorded, by the instrument they resolved to.
///
/// Reads the price facts: a fact of kind `price` with an `asOf` date, a `price`
/// amount, and exactly one instrument reference that resolved.
pub fn observations(
    resolved: &[crate::Resolved],
) -> Result<std::collections::BTreeMap<String, Vec<Observation>>> {
    let mut out: std::collections::BTreeMap<String, Vec<Observation>> = Default::default();
    for r in resolved.iter().filter(|r| r.fact.kind == "price" && r.is_admissible()) {
        let (Some(day), Some(minor)) = (
            r.fact.values.get("asOf").and_then(crate::Value::as_text),
            r.fact.values.get("price").and_then(crate::Value::as_minor),
        ) else {
            continue;
        };
        // The instrument it RESOLVED to, never the identifier the file carried:
        // two vendors quoting one security under different identifiers must
        // price one position.
        let Some(instrument) = r
            .fact
            .entities
            .iter()
            .find(|(_, e)| e.kind == crate::EntityKind::Instrument)
            .and_then(|(k, _)| r.entity(k))
        else {
            continue;
        };
        out.entry(instrument.to_string()).or_default().push(Observation {
            on_day: day.to_string(),
            minor,
            delivery: r.fact.provenance.delivery.clone(),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(day: &str, minor: i64) -> Observation {
        Observation { on_day: day.into(), minor, delivery: "d".into() }
    }

    #[test]
    fn the_price_is_the_latest_one_that_is_not_from_the_future() {
        let os = vec![obs("2026-02-24", 100_00), obs("2026-02-26", 251_14), obs("2026-03-02", 999_00)];
        // ⛔ The March price exists and must not be used to value February.
        let p = mark_price("2026-02-27", &os).unwrap();
        assert_eq!(p.on_day, "2026-02-26");
        assert_eq!(p.minor, 251_14);

        // On the day itself, the day's own price counts.
        assert_eq!(mark_price("2026-02-26", &os).unwrap().minor, 251_14);
        // Before anything was observed, nothing is.
        assert!(mark_price("2026-01-01", &os).is_none());
    }

    #[test]
    fn a_correction_on_the_same_day_supersedes_what_it_corrects() {
        // Two observations for one day: the later-recorded one wins, because a
        // correction is a new observation rather than an edit.
        let os = vec![obs("2026-02-26", 251_14), obs("2026-02-26", 252_00)];
        assert_eq!(mark_price("2026-02-26", &os).unwrap().minor, 252_00);
    }

    #[test]
    fn a_whole_quantity_always_values_exactly() {
        // `Ratio.Valuation.whole_units_value_exactly`: 1000 units at 251.14.
        assert_eq!(market_value(1000 * 100, 251_14), Some(251_140_00));
        // …and the refusal can only bite on a fraction.
        assert_eq!(market_value(50, 1_01), None, "half a unit at 1.01 is half a cent");
        assert_eq!(market_value(50, 1_00), Some(50));
    }

    #[test]
    fn an_unpriced_position_is_not_a_zero_one() {
        // ⛔ THE DISTINCTION THE WHOLE THING TURNS ON. Zero says "worth what it
        // cost"; unpriced says "we do not know", and only one of those should
        // let a NAV be struck.
        assert_eq!(value_position("2026-02-26", 100 * 100, 25_000_00, &[]), Valuation::Unpriced);

        let v = value_position("2026-02-26", 100 * 100, 25_000_00, &[obs("2026-02-26", 250_00)]);
        assert_eq!(
            v,
            Valuation::Marked {
                market: 25_000_00,
                delta: 0,
                price: 250_00,
                on_day: "2026-02-26".into(),
                delivery: "d".into(),
            },
            "a position already at market moves by nothing",
        );
    }

    #[test]
    fn marking_twice_posts_the_difference_and_then_nothing() {
        // `Ratio.Valuation.mark_again_posts_nothing` and `mark_is_the_difference`,
        // and the case `//tla:mark_from_cost_check` gets wrong.
        let units = 100 * 100;
        let cost = 25_000_00;

        let first = value_position("2026-02-26", units, cost, &[obs("2026-02-26", 260_00)]);
        let Valuation::Marked { delta: d1, market: m1, .. } = first else { panic!() };
        assert_eq!(m1, 26_000_00);
        assert_eq!(d1, 1_000_00, "the book moves by the gain");

        // The book now carries the market value. Marking again at the same
        // price must move it by NOTHING — not by the gain a second time.
        let again = value_position("2026-02-26", units, cost + d1, &[obs("2026-02-26", 260_00)]);
        let Valuation::Marked { delta: d2, .. } = again else { panic!() };
        assert_eq!(d2, 0, "marking twice at one price must not post the gain twice");

        // And a later price posts only what changed since.
        let later = value_position("2026-02-27", units, cost + d1, &[obs("2026-02-27", 270_00)]);
        let Valuation::Marked { delta: d3, .. } = later else { panic!() };
        assert_eq!(d3, 1_000_00);
        assert_eq!(d1 + d2 + d3, 2_000_00, "the marks sum to the total movement");
    }
}
