//! ratio-kernel — the double-entry accounting engine.
//!
//! Phase-0 stub. In Phase 1 the domain (Account/Split/Transaction, posting +
//! balance logic) is authored + proven in Lean and emitted to Rust.

/// Placeholder marker for the Phase-0 build graph.
pub const CRATE: &str = "ratio-kernel";

/// Smoke that the dependency edge to `ratio-common` links.
pub fn common_crate() -> &'static str {
    ratio_common::CRATE
}

#[cfg(test)]
mod tests {
    #[test]
    fn links_common() {
        assert_eq!(super::common_crate(), "ratio-common");
    }
}
