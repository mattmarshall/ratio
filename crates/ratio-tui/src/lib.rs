//! ratio-tui — terminal UI.
//!
//! Phase-0 stub. Phase 3 hosts meridian's `meridian_tui` renderer inside a
//! ratio-owned ratatui app shell.

/// Placeholder marker for the Phase-0 build graph.
pub const CRATE: &str = "ratio-tui";

#[cfg(test)]
mod tests {
    #[test]
    fn links_kernel() {
        assert!(ratio_kernel::money_is_zero(ratio_kernel::usd_zero()));
    }
}
