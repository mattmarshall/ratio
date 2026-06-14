//! ratio-tui — terminal UI.
//!
//! Phase-0 stub. Phase 3 hosts meridian's `meridian_tui` renderer (PanelView /
//! PanelAppState / RpcInvoker) inside a ratio-owned ratatui app shell, with the
//! double-entry split-entry form + charts as adhoc panels.

/// Placeholder marker for the Phase-0 build graph.
pub const CRATE: &str = "ratio-tui";

#[cfg(test)]
mod tests {
    #[test]
    fn links_kernel() {
        assert_eq!(ratio_kernel::common_crate(), "ratio-common");
    }
}
