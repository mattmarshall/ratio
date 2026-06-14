import Lake
open Lake DSL

-- Dep-free workspace: ratio's kernel model imports only Lean 4 core. The
-- lakefile exists so rules_lean's `lake.workspace` extension can download the
-- pinned toolchain (v4.30.0-rc2) and expose `:lean_toolchain_def`. The domain
-- (Money / double-entry) is authored here, proven via `lean_test`, and emitted
-- to Rust via the vendored `Polyglot.Rust` builders + `lean_emit`.
package ratioLean where
