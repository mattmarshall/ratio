/-
`Polyglot.Rust.RustAst` — Lean substrate for round-tripping `syn::File`
through transforms while keeping full fidelity for the parts we don't touch.

Design (the "json substrate + typed shells" pattern, the same shape Babel
uses for JavaScript and `ra_ap_syntax` does for Rust):

  * The full AST is `Lean.Json`. Parse / emit via the Rust binary
    `rules_lang/rust/cli/rust_ast_io/` (uses `syn` + `syn-serde` +
    `prettyplease`). No need to model every `syn` variant in Lean.

  * Typed *views* — `asFn?`, `asLet?`, `asCallee?`, `asIfThen?`, … — are
    added as transforms need them. Each view extracts a small typed
    record from the underlying `Json` (returns `none` if the shape
    doesn't match).

  * Typed *constructors* — `mkFn`, `mkLet`, … — produce `Json` matching
    the `syn-serde` shape so emitted RustAst round-trips through the
    Rust binary back to formatted source.

  * The `walk` family applies a transform recursively through the tree.
    `walk f j` invokes `f` on every node (post-order); `walkM` is the
    monadic variant for transforms that need state.

Why this beats writing a full Lean mirror of `syn`: `syn::File` has ~200
constructors with hundreds of fields. A typed mirror is months of work
and locks us to one syn version. The substrate is byte-faithful for the
180 variants we don't transform, type-safe for the 20 we do, and the
typed surface grows with consumer demand.
-/

import Lean.Data.Json

set_option warningAsError true

namespace Polyglot.Rust

open Lean (Json)

/-- A Rust AST node — opaque `Json` underneath. Use `RustAst.kind?`,
`RustAst.asFn?` etc. to extract typed views, or `RustAst.toString` /
`RustAst.toJson` for serialization back to the Rust binary. -/
abbrev RustAst := Json

/-- Serialize the underlying `Json` to its compact textual form. The Rust
binary `json_to_rust` consumes this; `prettyplease` formats the resulting
Rust source. -/
def RustAst.toString (a : RustAst) : String := Json.compress a

/-- The serialized JSON representation. Alias of the identity over `Json`
(we *are* `Json` underneath); provided for readability at call sites. -/
def RustAst.toJson (a : RustAst) : Json := a

/-- `syn-serde` tags each enum variant as a single-key object: e.g. an `Fn`
item is `{"fn": {…}}`, a `Let` stmt is `{"let": {…}}`. Extract that key as
the node's kind, or `none` for things that don't have a single-tag shape
(literals, identifiers, paths — those use other fields). -/
def RustAst.kind? : RustAst → Option String
  | .obj kvs =>
      match kvs.toArray with
      | #[(k, _)] => some k
      | _ => none
  | _ => none

/-- Look up a field on an object node, returning `none` if the node isn't
an object or the field is missing. -/
def RustAst.field? (a : RustAst) (name : String) : Option Json :=
  match a with
  | .obj kvs => kvs.toArray.find? (·.1 == name) |>.map (·.2)
  | _ => none

/-- Treat the node as a `{"<kind>": <payload>}` single-key object and
return the payload. Useful for chained traversals like
`a.kind?.bind (fun _ => a.payload?)`. -/
def RustAst.payload? : RustAst → Option Json
  | .obj kvs =>
      match kvs.toArray with
      | #[(_, v)] => some v
      | _ => none
  | _ => none

/-- Recursively apply `f` to every node in the tree, post-order. Each
`Json` value (object, array element, or leaf) is presented to `f`; `f`'s
result replaces the original. Subtree traversal first, then the current
node — so `f` sees nodes after its descendants have been transformed.
Matches the order most rewrite passes expect. -/
partial def RustAst.walk (f : RustAst → RustAst) : RustAst → RustAst
  | .obj kvs =>
      let pairs := kvs.toArray.toList.map (fun (k, v) => (k, RustAst.walk f v))
      f (Json.mkObj pairs)
  | .arr es =>
      f (.arr (es.map (RustAst.walk f)))
  | other => f other

/-- Monadic variant of `walk` for transforms that need to thread state
(rename tables, counters, error reporting). Same post-order shape. -/
partial def RustAst.walkM [Monad m] (f : RustAst → m RustAst) : RustAst → m RustAst
  | .obj kvs => do
      let pairs ← kvs.toArray.toList.mapM (fun (k, v) => do
        let v' ← RustAst.walkM f v
        pure (k, v'))
      f (Json.mkObj pairs)
  | .arr es => do
      let es' ← es.mapM (RustAst.walkM f)
      f (.arr es')
  | other => f other

end Polyglot.Rust
