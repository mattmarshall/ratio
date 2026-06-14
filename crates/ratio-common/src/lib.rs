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
