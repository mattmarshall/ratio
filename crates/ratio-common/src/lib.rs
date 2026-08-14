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

/// One copy of each distinct string, shared by everything that names it.
pub mod intern;

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
    // ⛔ THE YEAR IS BOUNDED BECAUSE THE ARITHMETIC BELOW IS NOT CHECKED.
    // `era * 146_097` overflows an `i64` somewhere past year 6×10¹³, which
    // panics in a debug build and wraps in a release one — in a function that
    // parses whatever a delivered file or a typed URL happened to contain.
    // `0..=9999` is ISO 8601's own range for a date written without the `+`
    // expanded-year prefix, which is the only shape this parser accepts.
    if !(0..=9999).contains(&y) {
        bail!("{s:?} names year {y}, outside 0000-9999");
    }

    // Howard Hinnant's `days_from_civil`, the inverse of the `civil_from_unix`
    // in `ratio_nav`. ⚠ `div_euclid`, not `/`: before the epoch a truncating
    // division rounds toward zero and the era comes out one too high.
    let written = y;
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    // ⛔ CHECKED AGAINST THE CALENDAR, NOT MERELY AGAINST A RANGE. The bounds
    // above accept day 31 in every month, so `2026-02-30` got past them and the
    // arithmetic turned it into the 2nd of March — a real day, silently, with
    // no error anywhere. What that costs depends on which caller asked:
    //
    //   * a TRADE DATE becomes a lot acquired two days after it was bought, so
    //     its holding period is two days somebody typed by accident, and the
    //     journal it is on is append-only;
    //   * a CALENDAR HOLIDAY becomes a holiday on a different day, and every
    //     T+n settlement date computed through that calendar moves with it —
    //     `views.rs` reads them with `filter_map(…ok())`, so a date that
    //     silently succeeds is a date silently believed.
    //
    // ⭐ ASKED OF THE INVERSE RATHER THAN OF A DAYS-IN-MONTH TABLE. A table
    // needs its own leap-year rule, which is a second place to get the same
    // thing wrong; `iso_date_from_days` already exists, is already tested
    // against this function by round trip, and disagrees with exactly the
    // inputs that are not days.
    //
    // ⚠ COMPARED ON THE PARSED COMPONENTS, NOT THE INPUT STRING. `2026-2-3` is
    // accepted above and is a real day; comparing against the raw text would
    // reject it for its padding.
    if iso_date_from_days(days) != format!("{written:04}-{m:02}-{d:02}") {
        bail!("{s:?} is not a day in the calendar");
    }
    Ok(days)
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
    fn a_date_in_range_that_is_not_a_day_is_refused_rather_than_moved() {
        // ⛔ THE FAILURE THIS TEST EXISTS FOR IS SILENT. `1..=31` accepts day 31
        // in February, and the civil arithmetic below it does not object — it
        // carries into the next month and returns a real day. `2026-02-30`
        // became the 2nd of March, and nothing anywhere said so.
        //
        // ⚠ NEGATIVE-TEST THIS. Delete the round-trip guard in
        // `days_from_iso_date` and every assertion here goes red; delete the
        // `1..=12` bound instead and none of them do, which is the point — the
        // range check and the calendar check catch different things.
        for (bad, would_have_been) in [
            ("2026-02-30", "2026-03-02"),
            ("2026-02-29", "2026-03-01"), // 2026 is not a leap year
            ("2026-04-31", "2026-05-01"),
            ("2026-06-31", "2026-07-01"),
            ("2026-09-31", "2026-10-01"),
            ("2026-11-31", "2026-12-01"),
            ("2023-02-29", "2023-03-01"),
            ("1900-02-29", "1900-03-01"), // divisible by 100, not by 400
        ] {
            let r = days_from_iso_date(bad);
            assert!(
                r.is_err(),
                "{bad:?} is not a day — it silently became {would_have_been}"
            );
            let said = r.unwrap_err().to_string();
            assert!(said.contains("not a day in the calendar"), "{bad:?} said {said:?}");
        }

        // ⭐ AND THE DAYS THAT ARE DAYS STILL ARE. A guard that refuses real
        // leap days is a worse defect than the one it replaced: it would reject
        // a trade date an operator typed correctly, on a day the market was
        // open, with no way to book it.
        for good in [
            "2024-02-29", // divisible by 4
            "2000-02-29", // divisible by 400
            "2026-01-31",
            "2026-02-28",
            "2026-12-31",
            "2026-04-30",
        ] {
            assert!(days_from_iso_date(good).is_ok(), "{good:?} is a day");
        }

        // ⚠ AND UNPADDED COMPONENTS ARE STILL A DATE. The guard compares the
        // PARSED components against the inverse, not the input text — otherwise
        // it would reject `2026-2-3` for its formatting rather than for being
        // an impossible day, and `2026-2-3` is the 3rd of February.
        assert_eq!(
            days_from_iso_date("2026-2-3").unwrap(),
            days_from_iso_date("2026-02-03").unwrap()
        );

        // ⛔ AND A YEAR THAT WOULD OVERFLOW THE ARITHMETIC IS REFUSED BEFORE IT
        // REACHES IT. This assertion is the reason the bound exists: without
        // it, `era * 146_097` panics here in a debug build — a parser that
        // takes down the process on a string somebody typed. The bound is ISO
        // 8601's own, so the ends of it are days.
        for bad in ["99999999999999-01-01", "-0001-01-01", "10000-01-01"] {
            assert!(days_from_iso_date(bad).is_err(), "{bad:?} should not parse");
        }
        assert!(days_from_iso_date("0001-01-01").is_ok(), "the bottom of the range");
        assert!(days_from_iso_date("9999-12-31").is_ok(), "the top of it");
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
