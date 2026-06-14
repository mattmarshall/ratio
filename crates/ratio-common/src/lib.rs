//! ratio-common — shared value types (Money, Currency).
//!
//! Phase-0 stub. In Phase 1 this crate's domain types are authored in Lean
//! (`//lean/Ratio/...`) and emitted to Rust via `@polyglot_ast`'s Rust builders;
//! the emitted `ratio_common.rs` replaces this stub and is `lean_regen_test`-gated.

/// Placeholder marker for the Phase-0 build graph.
pub const CRATE: &str = "ratio-common";

#[cfg(test)]
mod tests {
    #[test]
    fn crate_name() {
        assert_eq!(super::CRATE, "ratio-common");
    }
}
