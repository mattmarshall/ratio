//! ratio-common — shared value types (Money, Currency, RoundingMethod).
//!
//! `generated.rs` is **authored in Lean** (`//lean:Ratio/Common/Emit.lean`) and
//! emitted to Rust via the `Polyglot.Rust` builders; a `diff_test` gates it so
//! Lean stays the source of truth (regenerate with `//lean:ratio_common_rs`).
//! The algebraic properties of the Money operations are proven in `Ratio.Core`
//! (`//lean:money_proof_test`). Hand-written Rust (idiomatic wrappers, trait
//! impls) belongs here in `lib.rs`, alongside the generated module.

mod generated;
pub use generated::*;

use anyhow::{bail, Context, Result};

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
}
