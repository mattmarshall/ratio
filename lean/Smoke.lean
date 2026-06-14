/-! Toolchain smoke: passes iff the pinned Lean toolchain elaborates a trivial
proof. The real domain (Money / double-entry, with proofs) lands under
`Ratio/` in Phase 1, emitted to Rust via the `Polyglot.Rust` builders. -/

theorem ratio_lean_toolchain_ok : True := trivial
