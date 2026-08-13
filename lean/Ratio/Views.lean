import Ratio.Core

set_option warningAsError true

/-! `Ratio.Views` — one journal, more than one answer, and what that constrains.

A fund is asked for two figures about the same day. The accounting book of
record recognises a trade when it is STRUCK; a settlement-basis book recognises
it when cash and stock actually MOVE, some business days later. Both are the
fund's books. Neither is a model of the other, and that is the point: what
disappears is reconciling two STORES, not explaining a convention.

⭐ **A VIEW IS A RECOGNITION PREDICATE, NOT A DIMENSION AND NOT A SECOND BOOK.**
`Ratio.Chart.Dimensions` classifies every label a posting carries as conserved
(currency), partitioning (account, instrument) or measured (quantity). A view is
none of the three, and trying to make it one of them is the mistake this file
forecloses: it does not say how much value there is, or where it sits, or how
many units there are. It says WHICH ENTRIES ARE IN SCOPE AS OF A DAY.

That classification is what makes the rest cheap. A view keeps or drops WHOLE
entries and each entry conserves, so every view's books tie
(`every_view_conserves`). And two views over one journal differ by exactly the
entries one recognises and the other does not
(`two_views_differ_by_exactly_what_is_in_flight`) — which is what makes the gap
between an ABOR and an IBOR NAV a list somebody can read rather than a
discrepancy somebody has to hunt.

⛔ **THE SETTLEMENT DATE IS DERIVED, NEVER STORED.** It is the trade date rolled
forward over a calendar, both of which the entry's pinned configuration already
determines. A second date on the record would be a second thing to disagree
with the first, which is the argument `JournalEntry.trade_date` already makes
for living on the entry rather than on each posting.

⛔ **`recorded` IS NOT `settlement 0`, AND THE DIFFERENCE IS THE MIGRATION.**
A book that declares no view has one, and it recognises entries in the
journal's own order — it consults no date, so it answers over every book ever
written, including the ones whose entries carry no `trade_date` at all. A
same-day settlement convention is a real and DIFFERENT thing: it is an election,
it reads the calendar, and it refuses an undated entry. Collapsing them is
`lot_method: None` versus `Some Fifo` one layer out — "nobody said" defaulted
into an election nobody made.

⚠ **THE TRAP THIS FILE NAMES OUT LOUD IS `a_fold_with_no_cut_hides_the_
settlement_gap`.** Folded to the end of history every view agrees, because
everything eventually settles. So a differential test between two views that
does not CUT compares a figure with itself and goes green forever, under a
broken convention as readily as a working one. HANDOFF.md records the
multi-currency version of that mistake being made twice.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Views

/-- A day, as an ordinal. Ordering is all any of this depends on — the same
abbreviation `Ratio.Period` uses, for the same reason. -/
abbrev Day := Int

/-- Which date a view recognises an entry on.

⛔ THREE, AND THE FIRST IS NOT A CONVENTION. `recorded` is the absence of an
election: the journal's own order, which is what every book does today and what
every book written before this existed must go on doing. `trade` and
`settlement` are elections — somebody agreed to them, they read the record's
dates, and they refuse an entry that cannot support them. -/
inductive Basis
  | recorded
  | trade
  | settlement
deriving DecidableEq, Repr

/-- What a view's configuration says.

`holiday` is a predicate rather than a list because the question asked of it is
always "is the market shut on this day", and a list would invite two answers for
one day — the same reason `Ratio.Period.Fund.struck` is a function.

⚠ A TERM OF AN ADMINISTRATION AGREEMENT, in exactly the sense
`Ratio.Lots.Methods.Order` is. Which calendar governs settlement decides which
period a trade lands in, and that decides a figure somebody is paid on. -/
structure Convention where
  basis : Basis
  /-- Business days from trade to settlement. Read only on a settlement basis. -/
  lag : Nat
  holiday : Day → Bool

/-- An entry, abstracted to what a view turns on: the day it was struck — if the
record carries one — and its net effect.

⚠ `traded` IS AN `Option` BECAUSE THE RECORD'S IS. `JournalEntry.trade_date` is
optional, every book written before it existed lacks one, and modelling it as
present would prove theorems about a journal nobody has. -/
structure Entry (Dim : Type) where
  traded : Option Day
  net : Vec Dim

/- ── The calendar walk ─────────────────────────────────────────────────── -/

/-- The next open day strictly after `d`, given at most `fuel` days to look.

⛔ FUEL, BECAUSE A CALENDAR CAN CLOSE THE MARKET FOREVER AND THE SEARCH WOULD
NOT RETURN. That is not a hypothetical to assume away: a calendar is
configuration, somebody writes it, and "every day is a holiday" is one typo in a
generated file. Bounding the search makes the failure a REFUSAL — carried like
every other refusal here — rather than a fold that never finishes.
`a_calendar_that_never_opens_refuses_rather_than_looping` is the statement.

⚠ AND IT IS WHY THIS IS STRUCTURALLY RECURSIVE. `decide` cannot reduce
well-founded recursion, so a walk that terminated by a measure argument would
be a walk no theorem below could evaluate. -/
def nextOpenDay (holiday : Day → Bool) : Nat → Day → Option Day
  | 0, _ => none
  | fuel + 1, d => if holiday (d + 1) then nextOpenDay holiday fuel (d + 1) else some (d + 1)

/-- `n` open days after `d`, or nothing if the calendar never opens far enough. -/
def openDaysAfter (holiday : Day → Bool) (fuel : Nat) : Nat → Day → Option Day
  | 0, d => some d
  | n + 1, d =>
    match nextOpenDay holiday fuel d with
    | none => none
    | some d' => openDaysAfter holiday fuel n d'

/- ── Where a view puts an entry ────────────────────────────────────────── -/

/-- Where a view places an entry, as three answers rather than two.

⛔ `unplaceable` IS NOT "NOT YET". An entry the view cannot date at all — no
trade date, or a calendar that never opens — is a REFUSAL that has to be
reported, and it is a different fact from an entry that will be recognised
later. Collapsing them into "out of scope" is the whole failure: value would
leave a book silently while the trial balance went on tying, which is the shape
every defect in HANDOFF.md's table has. -/
inductive Placement
  | «always»
  | on (d : Day)
  | unplaceable
deriving DecidableEq, Repr

/-- Where this view puts this entry. -/
def placement {Dim : Type} (c : Convention) (fuel : Nat) (e : Entry Dim) : Placement :=
  match c.basis with
  | Basis.recorded => Placement.always
  | Basis.trade =>
    match e.traded with
    | none => Placement.unplaceable
    | some t => Placement.on t
  | Basis.settlement =>
    match e.traded with
    | none => Placement.unplaceable
    | some t =>
      match openDaysAfter c.holiday fuel c.lag t with
      | none => Placement.unplaceable
      | some s => Placement.on s

/-- Whether this view has this entry, as of a cut.

⚠ `asOf = none` IS "THE WHOLE JOURNAL". A fold to the end of history recognises
everything every convention can place, because everything eventually settles.
The difference between two views lives entirely at a CUT — see
`a_fold_with_no_cut_hides_the_settlement_gap`.

⚠ AND `false` HERE IS NOT THE WHOLE ANSWER FOR AN UNPLACEABLE ENTRY. It is out
of the fold, and it must also be in `unplaceable`, which is what
`nothing_is_dropped_without_being_reported` says. -/
def inScope {Dim : Type} (c : Convention) (fuel : Nat) (asOf : Option Day)
    (e : Entry Dim) : Bool :=
  match placement c fuel e with
  | Placement.always => true
  | Placement.on d =>
    match asOf with
    | none => true
    | some cut => decide (d ≤ cut)
  | Placement.unplaceable => false

/-- The entries this view cannot place at all, which a caller must report. -/
def unplaceable {Dim : Type} (c : Convention) (fuel : Nat) :
    List (Entry Dim) → List (Entry Dim)
  | [] => []
  | e :: es =>
    match placement c fuel e with
    | Placement.unplaceable => e :: unplaceable c fuel es
    | _ => unplaceable c fuel es

/- ── The fold ──────────────────────────────────────────────────────────── -/

/-- The whole journal, folded. `Ratio.Core.ledger` with the entry unwrapped. -/
def fold {Dim : Type} : List (Entry Dim) → Vec Dim
  | [] => Vec.zero
  | e :: es => Vec.add e.net (fold es)

/-- The entries a predicate keeps, folded. -/
def foldWhere {Dim : Type} (p : Entry Dim → Bool) : List (Entry Dim) → Vec Dim
  | [] => Vec.zero
  | e :: es => if p e then Vec.add e.net (foldWhere p es) else foldWhere p es

/-- What one view reads off one journal, as of a cut. -/
def foldIn {Dim : Type} (c : Convention) (fuel : Nat) (asOf : Option Day)
    (l : List (Entry Dim)) : Vec Dim :=
  foldWhere (inScope c fuel asOf) l

/- ── What a view preserves ─────────────────────────────────────────────── -/

/-- **A book that declares no view folds exactly as it always did.**

⛔ THE BACKWARD-COMPATIBILITY CLAIM, AS A THEOREM RATHER THAN A HOPE. Every book
in existence declares nothing, carries entries with no trade date, and the
feature is worthless if adding the concept moves their figures. The `recorded`
basis places every entry `always`, so it folds to the same vector as no view at
all — at every cut, including none, and whatever the calendar says. -/
theorem the_recorded_basis_folds_the_whole_journal {Dim : Type}
    (lag : Nat) (holiday : Day → Bool) (fuel : Nat) (asOf : Option Day)
    (l : List (Entry Dim)) :
    foldIn { basis := Basis.recorded, lag := lag, holiday := holiday } fuel asOf l = fold l := by
  induction l with
  | nil => rfl
  | cons e es ih => simp [foldIn, foldWhere, inScope, placement, fold] at *; exact ih

/-- **Every view's books tie.**

A view keeps or drops WHOLE ENTRIES and each entry conserves on its own, so
conservation survives the filter — `Ratio.Core.conserves_add` does the work and
there is nothing view-specific left to check. This is the theorem that says a
view cannot be the thing that unbalances a book, and it is why the trial balance
does not move under the view in the resource hierarchy. -/
theorem foldWhere_conserves {Dim : Type} (p : Entry Dim → Bool) (l : List (Entry Dim))
    (h : ∀ e ∈ l, Conserves e.net) : Conserves (foldWhere p l) := by
  induction l with
  | nil => exact conserves_zero
  | cons e es ih =>
      have he : Conserves e.net := h e (by simp)
      have hes : Conserves (foldWhere p es) := ih (fun x hx => h x (by simp [hx]))
      by_cases hp : p e = true
      · simpa [foldWhere, hp] using conserves_add he hes
      · simp only [Bool.not_eq_true] at hp
        simpa [foldWhere, hp] using hes

/-- The same statement about an actual view. -/
theorem every_view_conserves {Dim : Type} (c : Convention) (fuel : Nat)
    (asOf : Option Day) (l : List (Entry Dim)) (h : ∀ e ∈ l, Conserves e.net) :
    Conserves (foldIn c fuel asOf l) :=
  foldWhere_conserves _ l h

/-- **Nothing leaves a view's fold without appearing in what it could not
place.**

⛔ THE PROPERTY THAT KEEPS A REFUSAL FROM BECOMING A DISAPPEARANCE. An entry the
view drops is either out of scope for the cut — which is timing, and reconciles
— or unplaceable, which is a refusal somebody has to see. This says the second
kind is always in the list a caller is obliged to report, so a figure can never
be quietly short of an entry nobody was told about. -/
theorem nothing_is_dropped_without_being_reported {Dim : Type} (c : Convention)
    (fuel : Nat) (asOf : Option Day) (l : List (Entry Dim)) (e : Entry Dim)
    (hmem : e ∈ l) (hplace : placement c fuel e = Placement.unplaceable) :
    inScope c fuel asOf e = false ∧ e ∈ unplaceable c fuel l := by
  refine ⟨by simp [inScope, hplace], ?_⟩
  induction l with
  | nil => cases hmem
  | cons x xs ih =>
      rcases List.mem_cons.mp hmem with h | h
      · subst h
        simp [unplaceable, hplace]
      · have := ih h
        unfold unplaceable
        cases hx : placement c fuel x with
        | «always» => simpa [hx] using this
        | on d => simpa [hx] using this
        | unplaceable => simp [hx]; exact Or.inr this

/- ── What a view leaves behind ─────────────────────────────────────────── -/

/-- A view and its complement account for the whole journal, dimension by
dimension. The vector form is `a_view_is_a_partition_of_the_journal`. -/
theorem partition_at {Dim : Type} (p : Entry Dim → Bool) (l : List (Entry Dim)) (d : Dim) :
    foldWhere p l d + foldWhere (fun e => !p e) l d = fold l d := by
  induction l with
  | nil => simp [foldWhere, fold, Vec.zero]
  | cons e es ih =>
      by_cases hp : p e = true
      · simp [foldWhere, fold, hp, Vec.add]
        omega
      · simp only [Bool.not_eq_true] at hp
        simp [foldWhere, fold, hp, Vec.add]
        omega

/-- **Nothing falls out of the journal between a view and its complement.**

⛔ THE PROPERTY THAT MAKES A VIEW SAFE TO ADD. What one view does not recognise,
its complement does — so the two together are the book, exactly, on every
dimension. A view can therefore move a figure between periods and can never make
value disappear. -/
theorem a_view_is_a_partition_of_the_journal {Dim : Type} (p : Entry Dim → Bool)
    (l : List (Entry Dim)) :
    Vec.add (foldWhere p l) (foldWhere (fun e => !p e) l) = fold l := by
  funext d
  simpa [Vec.add] using partition_at p l d

/-- Two views' union, split into one of them and what the other adds. -/
theorem union_splits {Dim : Type} (a b : Entry Dim → Bool) (l : List (Entry Dim)) (d : Dim) :
    foldWhere (fun e => a e || b e) l d
      = foldWhere a l d + foldWhere (fun e => b e && !a e) l d := by
  induction l with
  | nil => simp [foldWhere, Vec.zero]
  | cons e es ih =>
      by_cases ha : a e = true <;> by_cases hb : b e = true <;>
        simp only [Bool.not_eq_true] at * <;>
        simp [foldWhere, ha, hb, Vec.add] <;> omega

/-- **Two views differ by exactly the entries in flight between them.**

⭐ THE THEOREM THE RECONCILIATION SCREEN IS. An ABOR figure and an IBOR figure
for one day are not two opinions to argue about: each is the other plus the
entries the other has not recognised yet, and those entries are a LIST somebody
can read. The gap is a derivation, not a discrepancy — which is the argument for
having both views in one system rather than two systems and a reconciliation
team.

Stated as an equality of sums rather than a subtraction because that is the form
a screen shows: each side is "the view, plus what it is still waiting for". -/
theorem two_views_differ_by_exactly_what_is_in_flight {Dim : Type}
    (a b : Entry Dim → Bool) (l : List (Entry Dim)) :
    Vec.add (foldWhere a l) (foldWhere (fun e => b e && !a e) l)
      = Vec.add (foldWhere b l) (foldWhere (fun e => a e && !b e) l) := by
  funext d
  have hab := union_splits a b l d
  have hba := union_splits b a l d
  have hcomm : (fun e : Entry Dim => b e || a e) = (fun e : Entry Dim => a e || b e) := by
    funext e; exact Bool.or_comm _ _
  rw [hcomm] at hba
  simp only [Vec.add]
  omega

/- ── What a view refuses ───────────────────────────────────────────────── -/

/-- **A settlement view refuses an entry with no trade date.**

The same refusal `Ratio.Lots.Methods.a_missing_acquisition_date_refuses` makes,
for the same reason: the epoch would settle it before every cut and make the
views agree; today would settle it after every cut and leave the settlement view
empty. Both look entirely ordinary on a screen. -/
theorem a_settlement_view_refuses_an_undated_entry {Dim : Type} (lag : Nat)
    (holiday : Day → Bool) (fuel : Nat) (n : Vec Dim) :
    placement { basis := Basis.settlement, lag := lag, holiday := holiday } fuel
      ({ traded := none, net := n } : Entry Dim) = Placement.unplaceable := by
  simp [placement]

/-- **And a trade-date view refuses it too**, because it is also an election
about a date the record does not carry. -/
theorem a_trade_date_view_refuses_an_undated_entry {Dim : Type} (lag : Nat)
    (holiday : Day → Bool) (fuel : Nat) (n : Vec Dim) :
    placement { basis := Basis.trade, lag := lag, holiday := holiday } fuel
      ({ traded := none, net := n } : Entry Dim) = Placement.unplaceable := by
  simp [placement]

/-- **"Nobody said" is not a settlement convention.**

⛔ THE `lot_method: None` ARGUMENT, ONE LAYER OUT. There is an entry the
undeclared view places and every election refuses — so a book that declares
nothing is NOT a book that elected same-day settlement, and reporting it as one
asserts a decision nobody made. That is not a hypothetical: it is what the
console did with the lot method, on three live funds, until the distinction was
given a field of its own. -/
theorem nobody_said_is_not_a_settlement_convention {Dim : Type} (fuel : Nat)
    (holiday : Day → Bool) (n : Vec Dim) :
    placement { basis := Basis.recorded, lag := 0, holiday := holiday } fuel
        ({ traded := none, net := n } : Entry Dim) = Placement.always
    ∧ placement { basis := Basis.settlement, lag := 0, holiday := holiday } fuel
        ({ traded := none, net := n } : Entry Dim) = Placement.unplaceable := by
  constructor <;> simp [placement]

/-- **A calendar that never opens refuses rather than looping.** -/
theorem a_calendar_that_never_opens_refuses_rather_than_looping (fuel : Nat) (d : Day) :
    nextOpenDay (fun _ => true) fuel d = none := by
  induction fuel generalizing d with
  | zero => rfl
  | succ n ih => simp [nextOpenDay, ih]

/- ── What a view cannot do to a date ───────────────────────────────────── -/

/-- The next open day is strictly later. -/
theorem nextOpenDay_moves_forward (h : Day → Bool) :
    ∀ (fuel : Nat) (d r : Day), nextOpenDay h fuel d = some r → d < r := by
  intro fuel
  induction fuel with
  | zero => intro d r hr; simp [nextOpenDay] at hr
  | succ n ih =>
      intro d r hr
      unfold nextOpenDay at hr
      by_cases hh : h (d + 1) = true
      · rw [if_pos hh] at hr
        have := ih (d + 1) r hr
        omega
      · simp only [Bool.not_eq_true] at hh
        rw [if_neg (by simp [hh])] at hr
        have : d + 1 = r := by injection hr
        omega

/-- Rolling over a calendar never goes backwards. -/
theorem openDaysAfter_never_goes_back (h : Day → Bool) (fuel : Nat) :
    ∀ (n : Nat) (d r : Day), openDaysAfter h fuel n d = some r → d ≤ r := by
  intro n
  induction n with
  | zero =>
      intro d r hr
      have : d = r := by injection hr
      omega
  | succ m ih =>
      intro d r hr
      unfold openDaysAfter at hr
      cases hn : nextOpenDay h fuel d with
      | none => rw [hn] at hr; exact absurd hr (by simp)
      | some d' =>
          rw [hn] at hr
          have h1 : d < d' := nextOpenDay_moves_forward h fuel d d' hn
          have h2 : d' ≤ r := ih d' r hr
          omega

/-- **A settlement date is never before the trade that produced it.**

⚠ THE PROPERTY A HAND-ROLLED CALENDAR LOSES FIRST. A roll that stepped backwards
over a holiday would settle a trade before it was struck — which reads as an
ordinary date, sorts correctly, and puts the entry in the wrong period. -/
theorem settlement_is_never_before_the_trade {Dim : Type} (lag : Nat)
    (holiday : Day → Bool) (fuel : Nat) (e : Entry Dim) (t r : Day)
    (ht : e.traded = some t)
    (hr : placement { basis := Basis.settlement, lag := lag, holiday := holiday } fuel e
            = Placement.on r) : t ≤ r := by
  unfold placement at hr
  simp only [ht] at hr
  cases hs : openDaysAfter holiday fuel lag t with
  | none => rw [hs] at hr; simp at hr
  | some s =>
      rw [hs] at hr
      have : s = r := by injection hr
      subst this
      exact openDaysAfter_never_goes_back holiday fuel lag t s hs

/- ── The two facts a reader most needs, on one concrete book ────────────── -/

/-- A calendar with nothing shut — the simplest one that is still a calendar. -/
def alwaysOpen : Day → Bool := fun _ => false

def tradeDate : Convention := { basis := Basis.trade, lag := 0, holiday := alwaysOpen }
def settledT2 : Convention := { basis := Basis.settlement, lag := 2, holiday := alwaysOpen }

/-- One subscription, struck on day 0.

⛔ A SUBSCRIPTION AND NOT A PURCHASE, AND THE CHOICE IS THE WHOLE TEST. A buy
moves cash into investments — both assets — so recognising it or not moves a
NAV by ZERO, and a differential test written over one is green whatever the
convention does. `HANDOFF.md` records that exact mistake being made when the
multi-currency differential test was written: "Subscriptions are the shape that
works." Modelled here as a single net effect on the dimension a NAV reads. -/
def oneSubscription : List (Entry Unit) := [{ traded := some 0, net := fun _ => 100 }]

/-- **The view decides the figure.**

⭐ THE THEOREM THAT SAYS THIS FEATURE IS NOT COSMETIC, and the shape
`Ratio.Lots.Methods.the_method_decides_the_taxable_gain` established: one
journal, one day, two conventions, two different answers — with nothing on the
record differing between them. Capital subscribed on the 0th and settling on the
2nd is in the book on the 1st under one view and not under the other. -/
theorem the_view_decides_the_figure :
    foldIn tradeDate 8 (some 1) oneSubscription () = 100
    ∧ foldIn settledT2 8 (some 1) oneSubscription () = 0 := by
  constructor <;> rfl

/-- **A fold with no cut hides the settlement gap.**

⛔ THE VACUITY TRAP, WRITTEN DOWN BEFORE IT IS FALLEN INTO. Folded to the end of
history every view that can place its entries agrees, because everything
eventually settles — so a differential test between two views that does not CUT
compares a figure with itself and is green forever, under a broken convention as
readily as a working one. This is the shape of
`the_projection_strikes_the_same_nav_as_a_full_fold`, which HANDOFF.md records
as having existed, passed, and tested nothing — twice.

⚠ So a test of this feature is worthless unless it strikes AS OF a day a trade
straddles, and has been watched failing. -/
theorem a_fold_with_no_cut_hides_the_settlement_gap :
    foldIn tradeDate 8 none oneSubscription = foldIn settledT2 8 none oneSubscription := by
  funext d
  rfl

end Ratio.Views
