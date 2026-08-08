set_option warningAsError true
/-! `Ratio.Ingest` — the data plane's proved core.

The plane that turns a counterparty's file into canonical facts has exactly two
places where a mistake is invisible until it is expensive, and both are here.

**Resolution.** A fact stores the *claims* it read off a row, not the entity
they resolved to, so resolution is a fold over the fact log and the master —
the same relationship the trial balance has to the journal. That is what makes
a pending fact clear on its own when the missing instrument is finally added.
The property the whole design rests on is that this can only ever go forwards:
`resolved_never_becomes_absent`. Reference data is added constantly, and if
adding it could un-resolve a fact that had already posted, the plane would be
unsound.

**Parsing.** A decimal string from a broker becomes minor units. The theorem
that matters is that the conversion is a *multiplication and never a division*
(`parse_is_scaling_not_division`), because a division is where a rounding
decision would hide. Anything that cannot be scaled exactly is refused rather
than truncated (`three_places_is_refused`).

Lean core + `omega`, no Mathlib, matching `Ratio.Core`. The decision functions
here are emitted to Rust by `Ratio.Ingest.Emit`; the folds over rows and
ladders are hand-written in `ratio-ingest` against them, the same division as
`Ratio.Chart` (emit the per-element decision, write the loop). -/

namespace Ratio.Ingest

/- ── The master, and what it means to match ────────────────────────────── -/

/-- One assertion of identity read off a row: `isin = GB00B03MLX29`. -/
structure Claim where
  /-- Named `attr`, not `attribute`: the latter is a Lean command keyword, so a
  field of that name does not parse and takes every projection below it with
  it. The Rust emitted from this carries the same name, so the proof and the
  code that runs say the same word. -/
  attr : String
  value : String
deriving DecidableEq, Repr

/-- An entity in the master.

Identifiers are **plural and none is authoritative** — an instrument may carry
an ISIN, a CUSIP, a ticker and an internal code, and different counterparties
send different ones. So attributes are a list of pairs rather than a record with
an `isin` field. -/
structure Entity where
  id : String
  attributes : List Claim
deriving DecidableEq, Repr

/-- What the entity records for an attribute, if anything. -/
def lookup (e : Entity) (a : String) : Option String :=
  (e.attributes.find? (fun p => p.attr == a)).map (·.value)

/-- Does the entity carry this exact claim? -/
def carries (e : Entity) (c : Claim) : Bool :=
  match lookup e c.attr with
  | none => false
  | some v => v == c.value

/-- Does the entity satisfy every claim of a rung?

The `isEmpty` guard is load-bearing and is the reason `empty_rung_matches_all_by_vacuity`
below is stated as it is: `List.all` on the empty list is `true`, so without the
guard an entity would satisfy an *empty* rung — and a template whose ladder read
only columns the file does not have would match every instrument in the master
and book a trade against an arbitrary one. -/
def satisfies (e : Entity) (rung : List Claim) : Bool :=
  !rung.isEmpty && rung.all (carries e)

/-- **Absence is not agreement.**

An entity that records nothing for an attribute does not match a claim about it.
The alternative — treating a missing CUSIP as "matches any CUSIP" — is how one
instrument comes to answer for another. -/
theorem silence_is_not_assent (e : Entity) (a v : String)
    (h : lookup e a = none) : carries e ⟨a, v⟩ = false := by
  simp [carries, h]

/-- Nothing satisfies an empty rung, so a ladder that read no columns matches
nothing rather than everything. Without the guard in `satisfies` this would be
`true` for every entity. -/
theorem empty_rung_matches_nothing (e : Entity) : satisfies e [] = false := by
  simp [satisfies]

/- ── Resolution: three outcomes, not two ───────────────────────────────── -/

/-- What resolving one reference produced.

Three outcomes. "Absent" is a reference-data gap someone fills by adding an
instrument; "ambiguous" is a data-quality problem in the master someone fixes by
de-duplicating or by narrowing the ladder. Collapsing them into "failed" hides
the second, which is the one that silently books against the wrong security. -/
inductive Resolution where
  | resolved (id : String)
  | absent
  | ambiguous (candidates : List String)
deriving DecidableEq, Repr

/-- Everything in the master that satisfies the rung. -/
def matching (M : List Entity) (rung : List Claim) : List Entity :=
  M.filter (fun e => satisfies e rung)

/-- Resolve one rung. Exactly one match resolves; more than one is ambiguous
and **blocks** — it does not pick. -/
def resolveRung (M : List Entity) (rung : List Claim) : Resolution :=
  match matching M rung with
  | [] => .absent
  | [e] => .resolved e.id
  | es => .ambiguous (es.map (·.id))

/-- Absent means nothing matched, and nothing matching means absent. -/
theorem absent_iff_nothing_matches (M : List Entity) (rung : List Claim) :
    resolveRung M rung = .absent ↔ matching M rung = [] := by
  unfold resolveRung
  cases h : matching M rung with
  | nil => simp
  | cons a t => cases t <;> simp

/-- If anything in the master satisfies the rung, the outcome is not absent.

This is the lemma the whole pending story rests on. -/
theorem not_absent_of_witness (M : List Entity) (rung : List Claim) (e : Entity)
    (hm : e ∈ M) (hs : satisfies e rung = true) :
    resolveRung M rung ≠ .absent := by
  intro habs
  rw [absent_iff_nothing_matches] at habs
  have hmem : e ∈ matching M rung := by
    simp only [matching, List.mem_filter]
    exact ⟨hm, hs⟩
  rw [habs] at hmem
  simp at hmem

/-- A resolved reference has a witness in the master. -/
theorem resolved_has_witness (M : List Entity) (rung : List Claim) (id : String)
    (h : resolveRung M rung = .resolved id) :
    ∃ e, e ∈ M ∧ satisfies e rung = true := by
  have hne : matching M rung ≠ [] := by
    intro hnil
    rw [← absent_iff_nothing_matches] at hnil
    rw [hnil] at h
    simp at h
  cases hm : matching M rung with
  | nil => exact absurd hm hne
  | cons a t =>
    have hmem : a ∈ matching M rung := by rw [hm]; simp
    simp only [matching, List.mem_filter] at hmem
    exact ⟨a, hmem.1, hmem.2⟩

/-- **The property the design rests on: growing the master never un-resolves.**

Reference data is added constantly — a new instrument, a new broker, a
correction. If adding it could turn a reference that had resolved back into one
that matches nothing, then a fact that had already posted could become pending
again, and the plane would be unsound.

It can go the other way: adding a *duplicate* turns `resolved` into `ambiguous`,
which blocks. That is the intended behavior and is why this is stated as "never
absent" rather than "still resolved" — the honest claim, not the one that reads
better. -/
theorem resolved_never_becomes_absent
    (M M' : List Entity) (rung : List Claim) (id : String)
    (hsub : ∀ e, e ∈ M → e ∈ M')
    (h : resolveRung M rung = .resolved id) :
    resolveRung M' rung ≠ .absent := by
  obtain ⟨e, hm, hs⟩ := resolved_has_witness M rung id h
  exact not_absent_of_witness M' rung e (hsub e hm) hs

/- ── The identity ladder ───────────────────────────────────────────────── -/

/-- Walk the ladder. The **first rung that matches anything decides**, including
when what it matches is two entities — falling through an ambiguous rung to a
weaker one that happens to match uniquely would turn a data-quality problem into
a confident wrong answer. -/
def resolveLadder (M : List Entity) : List (List Claim) → Resolution
  | [] => .absent
  | rung :: rest =>
    match resolveRung M rung with
    | .absent => resolveLadder M rest
    | r => r

/-- An empty ladder resolves to absent: a reference nothing identifies matches
nothing. -/
theorem empty_ladder_is_absent (M : List Entity) :
    resolveLadder M [] = .absent := rfl

/-- A ladder is absent exactly when every rung is. -/
theorem ladder_absent_iff (M : List Entity) (l : List (List Claim)) :
    resolveLadder M l = .absent ↔ ∀ rung ∈ l, resolveRung M rung = .absent := by
  induction l with
  | nil => simp [resolveLadder]
  | cons rung rest ih =>
    unfold resolveLadder
    cases h : resolveRung M rung with
    | absent =>
      simp only [ih]
      constructor
      · intro hr r hmem
        rcases List.mem_cons.mp hmem with rfl | hr'
        · exact h
        · exact hr r hr'
      · intro hall r hmem
        exact hall r (List.mem_cons_of_mem _ hmem)
    | resolved id =>
      constructor
      · intro hc; simp at hc
      · intro hall
        have hx := hall rung (by simp)
        rw [h] at hx; simp at hx
    | ambiguous cs =>
      constructor
      · intro hc; simp at hc
      · intro hall
        have hx := hall rung (by simp)
        rw [h] at hx; simp at hx

/-- A ladder that resolved did so at one of its rungs — the first that matched. -/
theorem ladder_resolved_at_some_rung (M : List Entity) :
    ∀ (l : List (List Claim)) (id : String),
      resolveLadder M l = .resolved id →
      ∃ rung, rung ∈ l ∧ resolveRung M rung = .resolved id := by
  intro l
  induction l with
  | nil => intro id h; simp [resolveLadder] at h
  | cons rung rest ih =>
    intro id h
    unfold resolveLadder at h
    cases hM : resolveRung M rung with
    | absent =>
      rw [hM] at h
      obtain ⟨r, hr, hres⟩ := ih id h
      exact ⟨r, List.mem_cons_of_mem _ hr, hres⟩
    | resolved i =>
      rw [hM] at h
      have hi : i = id := by simp at h; exact h
      exact ⟨rung, by simp, by rw [hM, hi]⟩
    | ambiguous cs =>
      rw [hM] at h; simp at h

/-- One rung that is not absent is enough to keep the whole ladder off absent. -/
theorem ladder_not_absent_of_rung (M : List Entity) (l : List (List Claim))
    (rung : List Claim) (hmem : rung ∈ l) (hne : resolveRung M rung ≠ .absent) :
    resolveLadder M l ≠ .absent := by
  intro hab
  rw [ladder_absent_iff] at hab
  exact hne (hab rung hmem)

/-- **The ladder inherits the safety property.**

A reference that resolved through *some* rung cannot go absent when the master
grows, whichever rung it was. This is the statement the pending queue relies on:
a fact that cleared stays cleared, no matter how much reference data arrives
afterwards. -/
theorem ladder_resolved_never_becomes_absent
    (M M' : List Entity) (hsub : ∀ e, e ∈ M → e ∈ M')
    (l : List (List Claim)) (id : String)
    (h : resolveLadder M l = .resolved id) :
    resolveLadder M' l ≠ .absent := by
  obtain ⟨rung, hmem, hres⟩ := ladder_resolved_at_some_rung M l id h
  exact ladder_not_absent_of_rung M' l rung hmem
    (resolved_never_becomes_absent M M' rung id hsub hres)

/- ── Parsing: minor units, exactly ─────────────────────────────────────── -/

/-- A decimal as it was written: a sign, the digits before the point, the digits
after it, and how many of those there were.

`fracLen` is carried separately because `"1.5"` and `"1.05"` have different
values and the same `frac` read as a number would not distinguish them — the
leading zero is information. -/
structure Decimal where
  negative : Bool
  whole : Nat
  frac : Nat
  fracLen : Nat
deriving DecidableEq, Repr

/-- The scale a fractional part is multiplied by to reach minor units. -/
def scale : Nat → Option Nat
  | 0 => some 100
  | 1 => some 10
  | 2 => some 1
  | _ => none

/-- A decimal in minor units, or nothing if it cannot be represented exactly. -/
def parseMinor (d : Decimal) : Option Int :=
  match scale d.fracLen with
  | none => none
  | some s =>
    let v : Int := Int.ofNat (d.whole * 100 + d.frac * s)
    some (if d.negative then -v else v)

/-- **The conversion is a multiplication, never a division.**

This is the theorem that matters. A division is the only place a rounding
decision could hide, and there is none: the result is the whole part scaled by a
hundred plus the fractional digits scaled by a power of ten. Nothing is
discarded, so nothing can be discarded silently. -/
theorem parse_is_scaling_not_division (d : Decimal) (s : Nat)
    (hs : scale d.fracLen = some s) (hpos : d.negative = false) :
    parseMinor d = some (Int.ofNat (d.whole * 100 + d.frac * s)) := by
  simp [parseMinor, hs, hpos]

/-- Two decimal places are exact: the fractional digits *are* the minor units. -/
theorem two_places_are_minor_units (neg : Bool) (w f : Nat) :
    parseMinor ⟨neg, w, f, 2⟩ =
      some (if neg then -Int.ofNat (w * 100 + f) else Int.ofNat (w * 100 + f)) := by
  simp [parseMinor, scale]

/-- One decimal place is tenths, so it scales by ten — `"1.5"` is 150 minor
units, not 15. -/
theorem one_place_is_tenths (w f : Nat) :
    parseMinor ⟨false, w, f, 1⟩ = some (Int.ofNat (w * 100 + f * 10)) := by
  simp [parseMinor, scale]

/-- **Three decimal places are refused, not truncated.**

`1.005` is the case the whole approach exists for: as a float it is
100.49999999999999, which truncates to 100 and loses a cent on every amount like
it. Here there is no representation, so there is no answer — and a refusal an
operator sees beats a cent an auditor finds. -/
theorem scale_none_of_three (n : Nat) (h : n ≥ 3) : scale n = none := by
  cases n with
  | zero => omega
  | succ a =>
    cases a with
    | zero => omega
    | succ b =>
      cases b with
      | zero => omega
      | succ _ => rfl

theorem three_places_is_refused (d : Decimal) (h : d.fracLen ≥ 3) :
    parseMinor d = none := by
  simp [parseMinor, scale_none_of_three d.fracLen h]

/-- A whole number is a hundred minor units per unit, with nothing after the
point to scale. -/
theorem no_point_is_whole_units (w : Nat) :
    parseMinor ⟨false, w, 0, 0⟩ = some (Int.ofNat (w * 100)) := by
  simp [parseMinor, scale]

/-- Negation is exactly a sign flip: the magnitude a negative amount parses to is
the one its positive twin parses to. No separate path, so no way for the two to
disagree. -/
theorem sign_only_flips (w f l : Nat) (s : Nat) (hs : scale l = some s) :
    parseMinor ⟨true, w, f, l⟩ =
      (parseMinor ⟨false, w, f, l⟩).map (fun v => -v) := by
  simp [parseMinor, hs]

/-- Minor units back to a decimal: the round trip a rendered figure makes. -/
def renderMinor (v : Nat) : Decimal :=
  ⟨false, v / 100, v % 100, 2⟩

/-- **Rendering and parsing are inverse on non-negative amounts.**

A figure written out and read back is the same figure — which is what lets the
same amount cross a file, a screen and a journal without drifting. -/
theorem render_parse_round_trip (v : Nat) :
    parseMinor (renderMinor v) = some (Int.ofNat v) := by
  have hv : v / 100 * 100 + v % 100 = v := by omega
  simp [renderMinor, parseMinor, scale, hv]

/- ── The emitted surface, and what ties it to the proofs above ─────────────

`Polyglot.Rust`'s builders cover enums, structs, functions, matches and
arithmetic — not folds over lists. That is the same division `Ratio.Chart`
already draws, and for the same reason: emitting a loop buys nothing, while the
per-element DECISION is where a mistake hides.

So the Rust counts, and the emitted function decides. The theorems below are
what make that safe — they say the fold cannot disagree with the decision,
because the decision depends on nothing but the count. Without them the emitted
Rust would be a function whose correctness argument lives in a docstring. -/

/-- The outcome of a resolution, without the payload. -/
inductive ResolutionKind where
  | resolved
  | absent
  | ambiguous
deriving DecidableEq, Repr

def kindOf : Resolution → ResolutionKind
  | .resolved _ => .resolved
  | .absent => .absent
  | .ambiguous _ => .ambiguous

/-- The three-outcome decision as a function of how many entities matched.
This is the shape emitted to Rust. -/
def resolutionOfCount (n : Nat) : ResolutionKind :=
  match n with
  | 0 => .absent
  | 1 => .resolved
  | _ => .ambiguous

/-- **The bridge.** The outcome's kind depends on nothing but the number of
matches — so a hand-written Rust fold that counts, feeding the emitted
`resolution_of_count`, cannot disagree with `resolveRung`. This is the
hypothesis the running code has to establish, stated so it can be checked
rather than assumed. -/
theorem kind_is_determined_by_count (M : List Entity) (rung : List Claim) :
    kindOf (resolveRung M rung) = resolutionOfCount (matching M rung).length := by
  unfold resolveRung
  cases h : matching M rung with
  | nil => simp [kindOf, resolutionOfCount]
  | cons a t =>
    cases t with
    | nil => simp [kindOf, resolutionOfCount]
    | cons b u => simp [kindOf, resolutionOfCount]

/-- Whether a rung is satisfied, as a function of the two facts the Rust fold
establishes: whether the rung was empty, and whether every claim held. -/
def rungSatisfied (isEmpty allHold : Bool) : Bool := !isEmpty && allHold

/-- The emitted guard agrees with `satisfies`. -/
theorem rung_satisfied_agrees (e : Entity) (rung : List Claim) :
    satisfies e rung = rungSatisfied rung.isEmpty (rung.all (carries e)) := rfl

/-- Whether one claim holds, from the two facts the Rust establishes: whether
the entity records the attribute at all, and whether the recorded value equals
the claimed one. **Silence is not assent** is exactly `false && _ = false`. -/
def claimHolds (recorded valuesEqual : Bool) : Bool := recorded && valuesEqual

theorem claim_needs_a_record (eq : Bool) : claimHolds false eq = false := rfl

/-- The scale with `0` standing for "not representable", because `Option` is not
in the emit vocabulary. The refusal is preserved: a zero scale is exactly the
case `scale` refuses. -/
def scaleInt (fracLen : Nat) : Int :=
  match fracLen with
  | 0 => 100
  | 1 => 10
  | 2 => 1
  | _ => 0

theorem scale_int_zero_iff_refused (n : Nat) : scaleInt n = 0 ↔ scale n = none := by
  cases n with
  | zero => simp [scaleInt, scale]
  | succ a =>
    cases a with
    | zero => simp [scaleInt, scale]
    | succ b =>
      cases b with
      | zero => simp [scaleInt, scale]
      | succ _ => simp [scaleInt, scale]

/-- Minor units from the parts. **A multiplication and an addition — there is no
division here, so there is nowhere for a rounding decision to hide.** -/
def minorUnits (whole frac s : Int) : Int := whole * 100 + frac * s

/-- The emitted arithmetic agrees with `parseMinor` on everything it accepts. -/
theorem minor_units_agrees (w f : Nat) (l : Nat) (s : Nat)
    (hs : scale l = some s) :
    parseMinor ⟨false, w, f, l⟩ =
      some (minorUnits (Int.ofNat w) (Int.ofNat f) (Int.ofNat s)) := by
  simp [parseMinor, minorUnits, hs]

end Ratio.Ingest
