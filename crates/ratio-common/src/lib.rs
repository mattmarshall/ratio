//! ratio-common — shared value types (Money, Currency, RoundingMethod).
//!
//! `generated.rs` is **authored in Lean** (`//lean:Ratio/Common/Emit.lean`) and
//! emitted to Rust via the `Polyglot.Rust` builders; a `diff_test` gates it so
//! Lean stays the source of truth (regenerate with `//lean:ratio_common_rs`).
//! The algebraic properties of the Money operations are proven in `Ratio.Core`
//! (`//lean:money_proof_test`). Hand-written Rust (idiomatic wrappers, trait
//! impls) belongs here in `lib.rs`, alongside the generated module.

/// Arithmetic that agrees with the proof or declines to answer — the Rust side
/// of `Ratio.Bounded`, and the hypothesis every theorem in this repo assumes.
pub mod checked;

mod generated;
pub use generated::*;

use anyhow::{bail, Context, Result};

/// An ISO date — `"2026-06-30"` — as days since 1970-01-01.
///
/// ⛔ THE UNIT `Ratio.Lots.Methods.heldDays` IS STATED IN. That file is
/// `asOf - acquired` over `Int`, and a holding period decides which tax rate a
/// realized gain is taxed at — so the conversion from the string a journal
/// entry actually carries has to be exact, at the boundaries, in both
/// directions. Off by one here moves a disposal between rates, and nothing
/// about the resulting figure looks unusual.
///
/// ⚠ HAND-ROLLED CALENDAR ARITHMETIC, deliberately: the alternative is a date
/// crate in a hermetic build for one function. It earns its place by being
/// tested at a leap day, a century boundary and before the epoch — the three
/// places this algorithm is ever wrong.
pub fn days_from_iso_date(s: &str) -> Result<i64> {
    let parts: Vec<&str> = s.trim().split('-').collect();
    let [y, m, d] = parts[..] else {
        bail!("{s:?} is not an ISO date — expected YYYY-MM-DD");
    };
    let y: i64 = y.parse().with_context(|| format!("{s:?} has no year"))?;
    let m: i64 = m.parse().with_context(|| format!("{s:?} has no month"))?;
    let d: i64 = d.parse().with_context(|| format!("{s:?} has no day"))?;
    if !(1..=12).contains(&m) {
        bail!("{s:?} names month {m}");
    }
    if !(1..=31).contains(&d) {
        bail!("{s:?} names day {d}");
    }

    // Howard Hinnant's `days_from_civil`, the inverse of the `civil_from_unix`
    // in `ratio_nav`. ⚠ `div_euclid`, not `/`: before the epoch a truncating
    // division rounds toward zero and the era comes out one too high.
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Ok(era * 146_097 + doe - 719_468)
}

/// Days since 1970-01-01 back into an ISO date.
///
/// The inverse of [`days_from_iso_date`], and the two are tested by round trip
/// as well as against fixed points — a pair of conversions that agree with each
/// other and not with the calendar is the failure a single-direction test
/// cannot see.
pub fn iso_date_from_days(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

/// A decimal string as a person or a counterparty writes it — `"25,000.00"`,
/// `"$1,204,880.11"`, `"-7"`, `".5"` — into minor units.
///
/// ⛔ BY SPLITTING ON THE POINT, NEVER BY PARSING A FLOAT.
/// `"1.005".parse::<f64>().unwrap() * 100.0` is 100.49999999999999, which
/// truncates to 100 and loses a cent on every such amount. A product whose
/// entire claim is that the arithmetic is exact cannot lose it at the boundary
/// where the numbers come in. A third decimal place is refused rather than
/// silently dropped: the books are kept in minor units, and quietly discarding
/// a digit is worse than saying so.
///
/// ⚠ THIS IS THE ONLY MONEY PARSER. It lives here because there were briefly
/// two — one in `ratio-recon` for counterparty files and one in
/// `ratio-console` for the entry form — written months apart and agreeing by
/// luck rather than by construction. Two parsers for the same product's money
/// is two places for the same rounding bug, and only one of them has tests.
pub fn parse_minor(text: &str) -> Result<i64> {
    let t = text.trim().replace(',', "").replace('$', "");
    let (neg, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest.to_string()),
        None => (false, t.strip_prefix('+').unwrap_or(&t).to_string()),
    };
    if digits.is_empty() {
        bail!("an amount is required");
    }
    let (whole, frac) = match digits.split_once('.') {
        Some((w, f)) => (w, f),
        None => (digits.as_str(), ""),
    };
    if whole.is_empty() && frac.is_empty() {
        bail!("{text:?} is not an amount");
    }
    if frac.len() > 2 {
        bail!("{text:?} has more than two decimal places; the books are kept in minor units");
    }
    if !whole.chars().all(|c| c.is_ascii_digit()) || !frac.chars().all(|c| c.is_ascii_digit()) {
        bail!("{text:?} is not an amount");
    }
    let major: i64 = if whole.is_empty() { 0 } else { whole.parse().context("amount too large")? };
    let minor: i64 = match frac.len() {
        0 => 0,
        1 => frac.parse::<i64>()? * 10,
        _ => frac.parse()?,
    };
    let v = major
        .checked_mul(100)
        .and_then(|m| m.checked_add(minor))
        .context("amount too large")?;
    Ok(if neg { -v } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_amount_is_parsed_by_splitting_not_by_floating() {
        assert_eq!(parse_minor("250000.00").unwrap(), 25_000_000);
        assert_eq!(parse_minor("0.10").unwrap(), 10);
        assert_eq!(parse_minor("0.1").unwrap(), 10);
        assert_eq!(parse_minor("1.5").unwrap(), 150);
        assert_eq!(parse_minor("42").unwrap(), 4_200);
        assert_eq!(parse_minor(".5").unwrap(), 50);
        assert_eq!(parse_minor("-25,000.00").unwrap(), -2_500_000);
        assert_eq!(parse_minor("$1,204,880.11").unwrap(), 120_488_011);

        // ⛔ THE CASE THE WHOLE APPROACH EXISTS FOR.
        // `1.005 as f64 * 100.0` is 100.49999999999999 — a cent lost, silently,
        // on every amount like it.
        assert!(parse_minor("1.005").is_err(), "three decimal places must be refused");
        assert_eq!(parse_minor("1.00").unwrap(), 100);

        for bad in ["", "  ", "abc", "1.2.3", "1e5", "--1", "1 000"] {
            assert!(parse_minor(bad).is_err(), "{bad:?} should not parse");
        }
        assert!(parse_minor("92233720368547758.08").is_err(), "overflow must be refused");
    }

    #[test]
    fn iso_dates_convert_exactly_at_the_places_this_algorithm_goes_wrong() {
        // Hand-rolled calendar arithmetic earns its place only if it is right at
        // the edges — the same standard `ratio_nav::civil_from_unix` is held to,
        // in the other direction.
        assert_eq!(days_from_iso_date("1970-01-01").unwrap(), 0, "the epoch");
        assert_eq!(days_from_iso_date("1970-01-02").unwrap(), 1);
        assert_eq!(days_from_iso_date("1969-12-31").unwrap(), -1, "before it");
        assert_eq!(days_from_iso_date("2024-02-29").unwrap(), 19_782, "a leap day");
        assert_eq!(days_from_iso_date("2000-03-01").unwrap(), 11_017, "a leap century");
        // ⚠ 1900 is NOT a leap year — divisible by 100 and not by 400 — which is
        // the one the naive rule gets wrong. Cross-checked against an
        // independent implementation rather than worked out by hand; the
        // hand-worked value was off by exactly that day.
        assert_eq!(days_from_iso_date("1900-03-01").unwrap(), -25_508);

        // ⛔ THE PROPERTY THE HOLDING PERIOD ACTUALLY TURNS ON: a year is 365
        // days and `Ratio.Lots.Methods.the_threshold_day_is_long_term` puts the
        // boundary ON the day, not after it.
        let a = days_from_iso_date("2025-01-15").unwrap();
        assert_eq!(days_from_iso_date("2026-01-15").unwrap() - a, 365);
        assert_eq!(days_from_iso_date("2026-01-14").unwrap() - a, 364, "one day short");

        // And a leap year in the middle makes the same calendar gap 366.
        let b = days_from_iso_date("2023-06-01").unwrap();
        assert_eq!(days_from_iso_date("2024-06-01").unwrap() - b, 366);

        for bad in ["", "2026-13-01", "2026-01-32", "2026-01", "not-a-date", "2026-xx-01"] {
            assert!(days_from_iso_date(bad).is_err(), "{bad:?} should not parse");
        }
    }

    #[test]
    fn the_two_date_conversions_agree_with_the_calendar_and_each_other() {
        // ⛔ BOTH CHECKS, BECAUSE EITHER ALONE PASSES A BROKEN PAIR. Two
        // conversions that are each other's exact inverse can still both be
        // wrong about what day it is, and fixed points alone say nothing about
        // whether one undoes the other.
        for (s, n) in [
            ("1970-01-01", 0),
            ("2024-02-29", 19_782),
            ("1900-03-01", -25_508),
            ("2026-06-30", 20_634),
        ] {
            assert_eq!(days_from_iso_date(s).unwrap(), n, "{s}");
            assert_eq!(iso_date_from_days(n), s, "{n}");
        }

        // Round trip across a leap day, a century, and the epoch.
        for d in [-30_000i64, -1, 0, 1, 19_782, 20_634, 40_000] {
            let s = iso_date_from_days(d);
            assert_eq!(days_from_iso_date(&s).unwrap(), d, "{s} round-trips");
        }
    }
}
