/-
`Polyglot.Rust.Builders` — smart constructors for `RustAst` nodes that
match `syn-serde`'s tagged-union JSON shape.

Each builder returns a `Json` that, when serialized and fed back through
`json_to_rust`, pretty-prints to the corresponding Rust source.

Coverage is grown per consumer demand — the constructors below are the
subset rules_texlive's Pascal→Rust translator needs to emit:
literals, identifiers, paths, binary/unary expressions, calls, blocks,
let/assign/if/while/loop/for/break/continue/return, fn items.

Reference for the JSON shape: serialize a small Rust file via the
sibling binary `rules_lang/rust/cli/rust_ast_io/rust_to_json` and
inspect — every constructor below has a corresponding inspect-cycle
documented in this commit.
-/

import Lean.Data.Json
import Polyglot.Rust.RustAst

set_option warningAsError true

namespace Polyglot.Rust.Builders

open Lean (Json)
open Polyglot.Rust

/- ─── Identifiers, paths, literals ───────────────────────────────────────── -/

/-- Single-segment path: `x` becomes `{"path":{"segments":[{"ident":"x"}]}}`.
For dotted paths (`std::collections::HashMap`), use `mkPathSegments`. -/
def mkPath (name : String) : Json :=
  Json.mkObj [("path", Json.mkObj
    [("segments", Json.arr #[Json.mkObj [("ident", Json.str name)]])])]

/-- Multi-segment path: `["std", "collections", "HashMap"]` → `std::collections::HashMap`. -/
def mkPathSegments (segs : List String) : Json :=
  Json.mkObj [("path", Json.mkObj
    [("segments", Json.arr (segs.map (fun s =>
      Json.mkObj [("ident", Json.str s)])).toArray)])]

/-- Integer literal: `mkLitInt "42"` → `{"lit":{"int":"42"}}`. The value
is stored as a string (matches `syn::LitInt`'s representation). -/
def mkLitInt (digits : String) : Json :=
  Json.mkObj [("lit", Json.mkObj [("int", Json.str digits)])]

/-- String literal: `mkLitStr "hello"` → `{"lit":{"str":"\"hello\""}}`.
syn-serde stores the raw source-form of the literal (quotes included),
because it parses it back via `syn::LitStr::parse`, which expects
`"…"`. Caller passes the unwrapped content; we add the quotes here. -/
def mkLitStr (s : String) : Json :=
  Json.mkObj [("lit", Json.mkObj [("str", Json.str ("\"" ++ s ++ "\""))])]

/-- Boolean literal: `mkLitBool true` → `{"lit":{"bool":true}}`. -/
def mkLitBool (b : Bool) : Json :=
  Json.mkObj [("lit", Json.mkObj [("bool", Json.bool b)])]

/-- Float literal: `mkLitFloat "0.2"` → `{"lit":{"float":"0.2"}}`.
syn-serde stores the source form as a string (must include the `.`,
since `LitInt` rejects it). Use this for Pascal real literals. -/
def mkLitFloat (digits : String) : Json :=
  Json.mkObj [("lit", Json.mkObj [("float", Json.str digits)])]

/-- Char literal: `mkLitChar "'A'"` → `{"lit":{"char":"'A'"}}`. The
input MUST include the surrounding single quotes (syn-serde parses
back via `LitChar::parse`, which expects `'…'`). For the NUL byte use
`"'\\u{0}'"` (Lean string-escaped); for an escape like `'\''` use
`"'\\''"`. -/
def mkLitChar (c : String) : Json :=
  Json.mkObj [("lit", Json.mkObj [("char", Json.str c)])]

/- ─── Expressions ────────────────────────────────────────────────────────── -/

/-- Binary expression: `mkBin "+" a b` → `a + b`. `op` is the literal
operator string (`"+"`, `"-"`, `"=="`, `"&&"`, …). -/
def mkBin (op : String) (left right : Json) : Json :=
  Json.mkObj [("binary", Json.mkObj
    [("left", left), ("op", Json.str op), ("right", right)])]

/-- Function call expression: `mkCall f [a, b]` → `f(a, b)`. -/
def mkCall (func : Json) (args : List Json) : Json :=
  Json.mkObj [("call", Json.mkObj
    [("func", func), ("args", Json.arr args.toArray)])]

/-- Bare `return;` — syn-serde encodes this as `{"return": {}}`. -/
def mkReturn : Json :=
  Json.mkObj [("return", Json.mkObj [])]

/-- `return <value>;` — `{"return": {"value": <expr>}}`. -/
def mkReturnValue (val : Json) : Json :=
  Json.mkObj [("return", Json.mkObj [("expr", val)])]

/-- Bare `break;`. -/
def mkBreak : Json := Json.mkObj [("break", Json.mkObj [])]

/-- `break 'label;` — the label string is the *bare* name (no leading `'`). -/
def mkBreakLabel (label : String) : Json :=
  Json.mkObj [("break", Json.mkObj [("label", Json.str label)])]

/-- Bare `continue;`. -/
def mkContinue : Json := Json.mkObj [("continue", Json.mkObj [])]

/-- `continue 'label;`. -/
def mkContinueLabel (label : String) : Json :=
  Json.mkObj [("continue", Json.mkObj [("label", Json.str label)])]

/-- Block expression `{ stmts }`. Each elem of `stmts` is a Stmt-shaped
node (use the `mkStmt*` family below). -/
def mkBlock (stmts : List Json) : Json :=
  Json.mkObj [("block", Json.mkObj [("stmts", Json.arr stmts.toArray)])]

/-- `'label: { stmts }` — a labeled block expression. `break 'label;` from
inside exits to the position right after the closing brace; the labeled-
block lift pattern used by Pascal.ToRust for forward gotos. -/
def mkBlockLabeled (label : String) (stmts : List Json) : Json :=
  Json.mkObj [("block", Json.mkObj
    [("label", Json.str label), ("stmts", Json.arr stmts.toArray)])]

/-- Method call expression: `receiver.method(args)` — used for things like
`(lo..=hi).rev()` to build a `for…downto` range. -/
def mkMethodCall (receiver : Json) (method : String) (args : List Json) : Json :=
  Json.mkObj [("method_call", Json.mkObj
    [("receiver", receiver),
     ("method", Json.str method),
     ("args", Json.arr args.toArray)])]

/- ─── Statements (the `stmts:` array body of a block / fn) ──────────────── -/

/-- `let x = init;` — pattern is a bare ident, no type annotation. -/
def mkLetIdent (name : String) (init : Json) : Json :=
  Json.mkObj [("let", Json.mkObj
    [("pat", Json.mkObj [("ident", Json.mkObj [("ident", Json.str name)])]),
     ("init", Json.mkObj [("expr", init)])])]

/-- `let x: T = init;` — pattern is `(ident, type)`. -/
def mkLetTyped (name : String) (ty : Json) (init : Json) : Json :=
  Json.mkObj [("let", Json.mkObj
    [("pat", Json.mkObj
      [("type", Json.mkObj
        [("pat", Json.mkObj [("ident", Json.mkObj [("ident", Json.str name)])]),
         ("ty", ty)])]),
     ("init", Json.mkObj [("expr", init)])])]

/-- `let mut x = init;` — bare ident with `mut`. -/
def mkLetMut (name : String) (init : Json) : Json :=
  Json.mkObj [("let", Json.mkObj
    [("pat", Json.mkObj [("ident", Json.mkObj
      [("mut", Json.bool true), ("ident", Json.str name)])]),
     ("init", Json.mkObj [("expr", init)])])]

/-- `let mut x: T = init;` — `mut`-prefixed ident with type annotation. -/
def mkLetMutTyped (name : String) (ty : Json) (init : Json) : Json :=
  Json.mkObj [("let", Json.mkObj
    [("pat", Json.mkObj
      [("type", Json.mkObj
        [("pat", Json.mkObj [("ident", Json.mkObj
          [("mut", Json.bool true), ("ident", Json.str name)])]),
         ("ty", ty)])]),
     ("init", Json.mkObj [("expr", init)])])]

/-- Wrap an expression as a statement. `withSemi` = `true` means `expr;`
(terminating semicolon, value discarded); `false` means `expr` (last
expression of a block, value returned). -/
def mkExprStmt (e : Json) (withSemi : Bool := true) : Json :=
  Json.mkObj [("expr", Json.arr #[e, Json.bool withSemi])]

/- ─── Control flow expressions ──────────────────────────────────────────── -/

/-- `if cond { then_body }` — `then_body` is a list of statements (the
block contents, NOT a wrapping block expression). -/
def mkIfThen (cond : Json) (thenBody : List Json) : Json :=
  Json.mkObj [("if", Json.mkObj
    [("cond", cond), ("then_branch", Json.arr thenBody.toArray)])]

/-- `if cond { then_body } else { else_body }`. Note syn-serde's asymmetry:
the then-branch is a bare stmt array, the else-branch is wrapped in
`{"block": {"stmts": …}}`. -/
def mkIfThenElse (cond : Json) (thenBody : List Json) (elseBody : List Json) : Json :=
  Json.mkObj [("if", Json.mkObj
    [("cond", cond),
     ("then_branch", Json.arr thenBody.toArray),
     ("else_branch", Json.mkObj
       [("block", Json.mkObj [("stmts", Json.arr elseBody.toArray)])])])]

/-- `while cond { body }` — body is the stmt list (unwrapped). -/
def mkWhile (cond : Json) (body : List Json) : Json :=
  Json.mkObj [("while", Json.mkObj
    [("cond", cond), ("body", Json.arr body.toArray)])]

/-- Bare `loop { body }`. -/
def mkLoop (body : List Json) : Json :=
  Json.mkObj [("loop", Json.mkObj [("body", Json.arr body.toArray)])]

/-- `'label: loop { body }`. The label is the bare name (no leading `'`). -/
def mkLoopLabeled (label : String) (body : List Json) : Json :=
  Json.mkObj [("loop", Json.mkObj
    [("label", Json.str label), ("body", Json.arr body.toArray)])]

/-- `for pat in <range> { body }`. `lo..=hi`-style range built by
`mkRangeInclusive`. -/
def mkForRange (pat range : Json) (body : List Json) : Json :=
  Json.mkObj [("for_loop", Json.mkObj
    [("pat", pat), ("expr", range), ("body", Json.arr body.toArray)])]

/-- Inclusive range `start..=end`. -/
def mkRangeInclusive (start end_ : Json) : Json :=
  Json.mkObj [("range", Json.mkObj
    [("start", start), ("limits", Json.str "..="), ("end", end_)])]

/-- `match scrut { arms }`. Each arm is built by `mkMatchArm`. -/
def mkMatch (scrut : Json) (arms : List Json) : Json :=
  Json.mkObj [("match", Json.mkObj
    [("expr", scrut), ("arms", Json.arr arms.toArray)])]

/-- A match arm: `pat => body`. For `1 | 2 | 3 => body` use a `mkPatOr`-
shaped pat. `mkBlock`-style bodies for multi-stmt arms. -/
def mkMatchArm (pat body : Json) : Json :=
  Json.mkObj [("pat", pat), ("body", body)]

/-- Or-pattern: `pat1 | pat2 | …`. syn-serde shape:
`{"or": {"cases": [pat1, pat2, …]}}`. -/
def mkPatOr (alts : List Json) : Json :=
  Json.mkObj [("or", Json.mkObj [("cases", Json.arr alts.toArray)])]

/-- Wildcard pattern `_`. syn-serde encodes this as the underscore key. -/
def mkPatWild : Json := Json.mkObj [("_", Json.mkObj [])]

/-- Literal pattern (matches an exact int / bool / str). syn-serde uses
the SAME `{"lit": …}` shape for both expression and pattern positions, so
`mkPatLit` is just the identity over the result of `mkLitInt`/`mkLitStr`/
`mkLitBool` — wrapping would double-tag and trip "no variant of Lit". -/
def mkPatLit (lit : Json) : Json := lit

/-- `unsafe { body }` block expression. -/
def mkUnsafe (body : List Json) : Json :=
  Json.mkObj [("unsafe", Json.mkObj [("stmts", Json.arr body.toArray)])]

/-- Assignment expression `lhs = rhs` (without trailing semicolon — wrap
with `mkExprStmt` to get the statement form). -/
def mkAssign (lhs rhs : Json) : Json :=
  Json.mkObj [("assign", Json.mkObj [("left", lhs), ("right", rhs)])]

/-- Reference expression `&e` (`&mut e` via `mkRefMut`). -/
def mkRef (e : Json) : Json :=
  Json.mkObj [("reference", Json.mkObj [("expr", e)])]

/-- Mutable reference expression `&mut e`. -/
def mkRefMut (e : Json) : Json :=
  Json.mkObj [("reference", Json.mkObj [("mut", Json.bool true), ("expr", e)])]

/-- Index expression `e[i]`. -/
def mkIndex (e i : Json) : Json :=
  Json.mkObj [("index", Json.mkObj [("expr", e), ("index", i)])]

/-- Cast expression `e as T`. -/
def mkCast (e ty : Json) : Json :=
  Json.mkObj [("cast", Json.mkObj [("expr", e), ("ty", ty)])]

/-- Field access `e.field`. syn-serde flattens `syn::Member::Named` into
the parent object as `"ident": "<name>"` (no `"member"` wrapper). -/
def mkField (e : Json) (field : String) : Json :=
  Json.mkObj [("field", Json.mkObj
    [("base", e), ("ident", Json.str field)])]

/-- Unary prefix expression: `-e`, `!e`, `*e`. `op` is the operator string. -/
def mkUnary (op : String) (e : Json) : Json :=
  Json.mkObj [("unary", Json.mkObj [("op", Json.str op), ("expr", e)])]

/-- Parenthesized expression `(e)` — used to force precedence. -/
def mkParen (e : Json) : Json :=
  Json.mkObj [("paren", Json.mkObj [("expr", e)])]

/-- Tuple-literal expression: `(a, b, c)`. Used to bundle variadic
call args into a single value the stub's generic `_x: T` parameter
can accept. -/
def mkTuple (elems : List Json) : Json :=
  Json.mkObj [("tuple", Json.mkObj
    [("elems", Json.arr elems.toArray)])]

/- ─── Types ──────────────────────────────────────────────────────────────── -/

/-- A type reference by name: `mkTypeNamed "i64"` → the type `i64`. -/
def mkTypeNamed (name : String) : Json := mkPath name

/-- A generic type application: `mkTypeApp "Vec" [i64]` → `Vec<i64>`.
syn-serde shape: `{"path": {"segments": [{"ident": "Vec",
"arguments": {"angle_bracketed": {"args": [{"type": <i64>}]}}}]}}`. -/
def mkTypeApp (name : String) (args : List Json) : Json :=
  let argList := args.map fun t => Json.mkObj [("type", t)]
  let ab := Json.mkObj [("angle_bracketed",
    Json.mkObj [("args", Json.arr argList.toArray)])]
  let seg := Json.mkObj [("ident", Json.str name), ("arguments", ab)]
  Json.mkObj [("path",
    Json.mkObj [("segments", Json.arr #[seg])])]

/-- `&mut T`. -/
def mkTypeRefMut (elem : Json) : Json :=
  Json.mkObj [("reference", Json.mkObj
    [("mut", Json.bool true), ("elem", elem)])]

/-- `&T` (shared reference). -/
def mkTypeRef (elem : Json) : Json :=
  Json.mkObj [("reference", Json.mkObj [("elem", elem)])]

/-- Array type `[T; N]`. `len` is an expression (typically a numeric literal). -/
def mkTypeArray (elem len : Json) : Json :=
  Json.mkObj [("array", Json.mkObj [("elem", elem), ("len", len)])]

/- ─── Patterns / params ──────────────────────────────────────────────────── -/

/-- A bare identifier pattern: `mkPatIdent "x"` → matches/binds `x`. -/
def mkPatIdent (name : String) : Json :=
  Json.mkObj [("ident", Json.mkObj [("ident", Json.str name)])]

/-- A `mut`-prefixed identifier pattern: `mut x`. -/
def mkPatIdentMut (name : String) : Json :=
  Json.mkObj [("ident", Json.mkObj
    [("mut", Json.bool true), ("ident", Json.str name)])]

/-- A typed pattern `pat: ty` (used for fn params and `let mut x: T`). -/
def mkPatTyped (pat ty : Json) : Json :=
  Json.mkObj [("type", Json.mkObj [("pat", pat), ("ty", ty)])]

/-- A fn input (parameter): `name: T` typed pattern. -/
def mkParam (pat ty : Json) : Json :=
  Json.mkObj [("typed", Json.mkObj [("pat", pat), ("ty", ty)])]

/- ─── Items (top-level) ──────────────────────────────────────────────────── -/

/-- A fn item with stmts body. `output` is the return type (use `Json.null`
for procedures). -/
def mkFnItem (name : String) (params : List Json) (output : Json) (stmts : List Json) : Json :=
  Json.mkObj [("fn", Json.mkObj
    [("ident", Json.str name),
     ("inputs", Json.arr params.toArray),
     ("output", output),
     ("stmts", Json.arr stmts.toArray)])]

/-- A *generic* fn item: `fn name<T1, T2, …>(params) -> output { stmts }`.
The type parameters appear under `generics.params` as
`[{"type": {"ident": "T1"}}, …]`. Useful for permissive built-in
stubs that accept any argument type (Pascal-WEB `get`, `write_ln`, …). -/
def mkFnItemGeneric (name : String) (typeParams : List String)
    (params : List Json) (output : Json) (stmts : List Json) : Json :=
  let tpNodes := typeParams.map (fun t =>
    Json.mkObj [("type", Json.mkObj [("ident", Json.str t)])])
  Json.mkObj [("fn", Json.mkObj
    [("ident", Json.str name),
     ("generics", Json.mkObj [("params", Json.arr tpNodes.toArray)]),
     ("inputs", Json.arr params.toArray),
     ("output", output),
     ("stmts", Json.arr stmts.toArray)])]

/-- A foreign-fn declaration (no body — used inside `extern "C" { … }`). -/
def mkForeignFn (name : String) (params : List Json) (output : Json) : Json :=
  Json.mkObj [("fn", Json.mkObj
    [("ident", Json.str name),
     ("inputs", Json.arr params.toArray),
     ("output", output)])]

/-- `extern "C" { items }` block. -/
def mkExternC (items : List Json) : Json :=
  Json.mkObj [("foreign_mod", Json.mkObj
    [("abi", Json.mkObj [("name", Json.str "\"C\"")]),
     ("items", Json.arr items.toArray)])]

/-- Struct-literal expression: `Foo { a: 1, b: 2 }`. `fields` is a list
of `(field-name, value)` pairs. -/
def mkStructLit (name : String) (fields : List (String × Json)) : Json :=
  let fieldNodes := fields.map (fun (n, v) =>
    Json.mkObj
      [("ident", Json.str n),
       ("colon_token", Json.bool true),
       ("expr", v)])
  Json.mkObj [("struct", Json.mkObj
    [("path", Json.mkObj
      [("segments", Json.arr #[Json.mkObj [("ident", Json.str name)]])]),
     ("fields", Json.arr fieldNodes.toArray)])]

/-- `type <name> = <ty>;` item. -/
def mkTypeAlias (name : String) (ty : Json) : Json :=
  Json.mkObj [("type", Json.mkObj
    [("ident", Json.str name), ("ty", ty)])]

/-- `const <name>: <ty> = <expr>;` item. -/
def mkConstItem (name : String) (ty expr : Json) : Json :=
  Json.mkObj [("const", Json.mkObj
    [("ident", Json.str name), ("ty", ty), ("expr", expr)])]

/-- `static mut <name>: <ty> = <expr>;` item. The `mut` field carries
the literal string `"mut"`; syn-serde uses an enum-flag here. -/
def mkStaticMut (name : String) (ty expr : Json) : Json :=
  Json.mkObj [("static", Json.mkObj
    [("mut", Json.str "mut"),
     ("ident", Json.str name),
     ("ty", ty),
     ("expr", expr)])]

/-- `impl <Trait> for <Type> { <items> }`. `traitName` is the bare path
ident (`Copy`, `Clone`, …); `typeName` is the target struct's name.
syn-serde's `trait` field is a `[bool, path-segments-object]` array. -/
def mkImplTrait (traitName typeName : String) (items : List Json) : Json :=
  let traitPath : Json :=
    Json.mkObj [("segments",
      Json.arr #[Json.mkObj [("ident", Json.str traitName)]])]
  let selfTy : Json :=
    Json.mkObj [("path",
      Json.mkObj [("segments",
        Json.arr #[Json.mkObj [("ident", Json.str typeName)]])])]
  Json.mkObj [("impl", Json.mkObj
    [("trait", Json.arr #[Json.bool false, traitPath]),
     ("self_ty", selfTy),
     ("items", Json.arr items.toArray)])]

/-- `fn clone(&self) -> Self { *self }` — the canonical "Copy-derive
the Clone" body. Used as the single item of an `impl Clone for T`
block. -/
def mkCloneFn : Json :=
  let selfRefTy : Json :=
    Json.mkObj [("reference", Json.mkObj
      [("elem",
        Json.mkObj [("path", Json.mkObj
          [("segments", Json.arr #[Json.mkObj [("ident", Json.str "Self")]])])])])]
  let inputReceiver : Json :=
    Json.mkObj [("receiver", Json.mkObj
      [("ref", Json.bool true), ("ty", selfRefTy)])]
  let outputSelf : Json :=
    Json.mkObj [("path", Json.mkObj
      [("segments", Json.arr #[Json.mkObj [("ident", Json.str "Self")]])])]
  let bodyExpr : Json := mkUnary "*" (mkPath "self")
  Json.mkObj [("fn", Json.mkObj
    [("ident", Json.str "clone"),
     ("inputs", Json.arr #[inputReceiver]),
     ("output", outputSelf),
     ("stmts", Json.arr #[mkExprStmt bodyExpr (withSemi := false)])])]

/-- `struct <name> { <field>: <ty>, … }` item. `fields` is a list of
`(name, type)` pairs. Empty-field structs (`struct Foo;`) emit as
unit-tuple form `{"unit": {}}`; structs with at least one field use
the named-field form. -/
def mkStructItem (name : String) (fields : List (String × Json)) : Json :=
  let fieldsJson :=
    if fields.isEmpty then
      -- Unit struct (`struct Foo;`) — syn-serde uses a bare string tag.
      Json.str "unit"
    else
      let fieldNodes := fields.map (fun (n, t) =>
        Json.mkObj
          [("ident", Json.str n),
           ("colon_token", Json.bool true),
           ("ty", t)])
      Json.mkObj [("named", Json.arr fieldNodes.toArray)]
  Json.mkObj [("struct", Json.mkObj
    [("ident", Json.str name),
     ("fields", fieldsJson)])]

/-- `union <name> { <field>: <ty>, … }` item. Used to lower Pascal
variant records that have ONLY variant parts (the canonical case is
`memory_word`, whose four fields `int` / `gr` / `hh` / `qqqq` all
overlap into one word). syn's `ItemUnion` has `fields: FieldsNamed`
directly (no `Fields::Named/Unnamed/Unit` enum wrapper like struct
has), so the field-list JSON is a bare array, NOT wrapped in
`{"named": [...]}` the way struct fields are. -/
def mkUnionItem (name : String) (fields : List (String × Json)) : Json :=
  let fieldNodes := fields.map (fun (n, t) =>
    Json.mkObj
      [("ident", Json.str n),
       ("colon_token", Json.bool true),
       ("ty", t)])
  Json.mkObj [("union", Json.mkObj
    [("ident", Json.str name),
     ("fields", Json.arr fieldNodes.toArray)])]

/-- The top-level `syn::File` envelope wrapping a list of items. -/
def mkFile (items : List Json) : Json :=
  Json.mkObj [("items", Json.arr items.toArray)]

/- ─── ratio additions: enum items + `#[derive(...)]` attributes ──────────────
The vendored Builders cover structs / fns / impls but not enum items or derive
attributes (rules_texlive's Pascal corpus needed neither). ratio's domain wants
`enum Currency { … }` and `#[derive(Debug, Clone, …)]` on every type, so these
are added here. Shapes discovered via the `rust_to_json` inspect-cycle. -/

/-- A `#[derive(T0, T1, …)]` outer attribute. The `tokens` token-stream is the
trait idents separated by `,`-puncts (syn-serde's `Meta::List` shape). -/
def mkDeriveAttr (traits : List String) : Json :=
  let identTok (s : String) : Json := Json.mkObj [("ident", Json.str s)]
  let comma : Json := Json.mkObj [("punct", Json.mkObj
    [("op", Json.str ","), ("spacing", Json.str "alone")])]
  let toks : List Json := match traits with
    | [] => []
    | t :: ts => identTok t :: ts.foldr (fun x acc => comma :: identTok x :: acc) []
  Json.mkObj
    [("style", Json.str "outer"),
     ("meta", Json.mkObj [("list", Json.mkObj
       [("path", Json.mkObj [("segments",
          Json.arr #[Json.mkObj [("ident", Json.str "derive")]])]),
        ("delimiter", Json.str "paren"),
        ("tokens", Json.arr toks.toArray)])])]

/-- Optional `attrs` field carrying one `#[derive(...)]`, or nothing. -/
def deriveAttrs (derives : List String) : List (String × Json) :=
  if derives.isEmpty then [] else [("attrs", Json.arr #[mkDeriveAttr derives])]

/-- `#[derive(...)] pub enum <name> { V0, V1, … }` with unit (fieldless)
variants — the shape `Currency` / `AccountType` / `RoundingMethod` want. -/
def mkEnumItem (name : String) (variants : List String)
    (derives : List String := []) : Json :=
  let variantNodes := variants.map (fun v =>
    Json.mkObj [("ident", Json.str v), ("fields", Json.str "unit")])
  Json.mkObj [("enum", Json.mkObj
    (deriveAttrs derives ++
      [("vis", Json.str "pub"),
       ("ident", Json.str name),
       ("variants", Json.arr variantNodes.toArray)]))]

/-- Like `mkStructItem`, but `pub` (struct + every field) and carrying a
`#[derive(...)]` attribute — the shape a domain value type wants. -/
def mkStructItemD (name : String) (fields : List (String × Json))
    (derives : List String := []) : Json :=
  let fieldsJson :=
    if fields.isEmpty then
      Json.str "unit"
    else
      let fieldNodes := fields.map (fun (n, t) =>
        Json.mkObj
          [("vis", Json.str "pub"),
           ("ident", Json.str n),
           ("colon_token", Json.bool true),
           ("ty", t)])
      Json.mkObj [("named", Json.arr fieldNodes.toArray)]
  Json.mkObj [("struct", Json.mkObj
    (deriveAttrs derives ++
      [("vis", Json.str "pub"),
       ("ident", Json.str name),
       ("fields", fieldsJson)]))]

/-- `pub fn name(params) -> output { stmts }` — like `mkFnItem` but `pub`, for
the crate's public API. -/
def mkFnItemD (name : String) (params : List Json) (output : Json)
    (stmts : List Json) : Json :=
  Json.mkObj [("fn", Json.mkObj
    [("vis", Json.str "pub"),
     ("ident", Json.str name),
     ("inputs", Json.arr params.toArray),
     ("output", output),
     ("stmts", Json.arr stmts.toArray)])]

end Polyglot.Rust.Builders
