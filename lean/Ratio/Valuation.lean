set_option warningAsError true
/-! `Ratio.Valuation` — marking a book to market, proved before it is built.

Valuation is where an accounting system is most tempted to MUTATE. A position is
held at cost, a price arrives, and the obvious thing is to overwrite the
carrying value. That would destroy the property everything else here rests on:
the journal is append-only, a NAV strike is pinned to a journal prefix, and a
figure can name what produced it.

So marking to market is **a posting, not an assignment** — the movement in the
asset and its equal and opposite in unrealized gain — and the theorems below are
the ones that make that safe:

* `mark_conserves`          a mark is a balanced entry, so a marked book still
                            ties. Conservation is not something valuation gets
                            to opt out of.
* `mark_lands_on_market`    after marking, the book holds market value. The
                            point of the exercise, stated as an equation.
* `mark_again_posts_nothing`  marking twice at one price posts a second entry
                            of zero. Re-running a valuation is not a way to
                            invent gains.
* `mark_is_the_difference`  a later price posts only what changed, so the
                            entries sum to the total movement rather than
                            double-counting it.
* `unpriced_yields_no_mark` a position with no price yields NO mark, not a zero
                            one. Zero says "worth what it cost"; none says "we
                            do not know", and a NAV struck over the first is
                            wrong in a way nobody can see.
* `markPrice_is_latest_on_or_before` / `markPrice_is_not_after`
                            which price. Deterministic, never from the future.

Lean core + `omega`, no Mathlib, matching `Ratio.Core`. -/

namespace Ratio.Valuation

/- ── Which price ───────────────────────────────────────────────────────── -/

/-- A price as it was observed: on a day, at an amount per unit in minor units.

The day is an ordinal rather than a calendar date; ordering is all the choice
below depends on, and a calendar in the kernel would be a second thing to get
wrong. -/
structure Observation where
  onDay : Int
  minor : Int
deriving DecidableEq, Repr

/-- The observations a valuation may use: those on or before the day it values.

⛔ A PRICE FROM AFTER THE VALUATION DATE IS NOT EVIDENCE ABOUT IT. Using one is
how a NAV comes to be struck on information that did not exist when it was
struck, which is unauditable in the most literal sense — the replay would need
tomorrow's file. -/
def eligible (asOf : Int) (os : List Observation) : List Observation :=
  os.filter (fun o => decide (o.onDay ≤ asOf))

/-- The latest of a list, or none if it is empty. -/
def latest : List Observation → Option Observation
  | [] => none
  | o :: rest =>
    match latest rest with
    | none => some o
    | some b => some (if o.onDay ≥ b.onDay then o else b)

/-- The price to mark at: the most recent observation on or before the day.

A stale price is still a price — it is the last thing anybody observed — so this
returns it and leaves "how stale is too stale" to the configuration, where a
tolerance belongs. What it will not do is guess. -/
def markPrice (asOf : Int) (os : List Observation) : Option Observation :=
  latest (eligible asOf os)

theorem latest_mem : ∀ (os : List Observation) (o : Observation),
    latest os = some o → o ∈ os := by
  intro os
  induction os with
  | nil => intro o h; simp [latest] at h
  | cons p rest ih =>
    intro o h
    unfold latest at h
    cases hr : latest rest with
    | none =>
      rw [hr] at h
      simp at h
      subst h
      simp
    | some b =>
      rw [hr] at h
      simp at h
      by_cases hc : p.onDay ≥ b.onDay
      · simp [hc] at h; subst h; simp
      · simp [hc] at h
        subst h
        exact List.mem_cons_of_mem _ (ih b hr)

theorem latest_is_greatest : ∀ (os : List Observation) (o p : Observation),
    latest os = some o → p ∈ os → p.onDay ≤ o.onDay := by
  intro os
  induction os with
  | nil => intro o p h _; simp [latest] at h
  | cons q rest ih =>
    intro o p h hp
    unfold latest at h
    cases hr : latest rest with
    | none =>
      rw [hr] at h
      simp at h
      subst h
      -- `latest rest = none` forces `rest = []`, so `p` must be the head.
      have hrest : rest = [] := by
        cases rest with
        | nil => rfl
        | cons a t =>
          -- `latest` of a non-empty list is always `some`, whichever branch it
          -- takes, so this case cannot arise.
          exfalso
          unfold latest at hr
          -- `cases` on the value alone does not rewrite `hr`; it has to be
          -- named and substituted.
          cases hl : latest t with
          | none => rw [hl] at hr; simp at hr
          | some b => rw [hl] at hr; simp at hr
      subst hrest
      simp at hp
      subst hp
      omega
    | some b =>
      rw [hr] at h
      simp at h
      rcases List.mem_cons.mp hp with rfl | hin
      · by_cases hc : p.onDay ≥ b.onDay
        · simp [hc] at h; subst h; omega
        · simp [hc] at h; subst h; omega
      · have hb : p.onDay ≤ b.onDay := ih b p hr hin
        by_cases hc : q.onDay ≥ b.onDay
        · simp [hc] at h; subst h; omega
        · simp [hc] at h; subst h; omega

/-- **The chosen price is never from the future.** -/
theorem markPrice_is_not_after (asOf : Int) (os : List Observation) (o : Observation)
    (h : markPrice asOf os = some o) : o.onDay ≤ asOf := by
  have := latest_mem _ o h
  simp [eligible, List.mem_filter] at this
  exact this.2

/-- **And it is the latest one that is not.** -/
theorem markPrice_is_latest_on_or_before
    (asOf : Int) (os : List Observation) (o p : Observation)
    (h : markPrice asOf os = some o) (hp : p ∈ os) (hle : p.onDay ≤ asOf) :
    p.onDay ≤ o.onDay := by
  refine latest_is_greatest (eligible asOf os) o p h ?_
  simp [eligible, List.mem_filter]
  exact ⟨hp, hle⟩

/-- No price on or before the day means no mark. Nothing is invented. -/
theorem markPrice_none_of_all_later (asOf : Int) (os : List Observation)
    (h : ∀ p ∈ os, asOf < p.onDay) : markPrice asOf os = none := by
  have : eligible asOf os = [] := by
    unfold eligible
    rw [List.filter_eq_nil_iff]
    intro a ha
    have := h a ha
    simp
    omega
  simp [markPrice, this, latest]

/- ── What a position is worth ──────────────────────────────────────────── -/

/-- Units held, in hundredths of a unit — fractional shares exist.

Held as an integer for the same reason money is: a position that drifts because
its quantity was a float is a position nobody can reconcile. -/
abbrev Units := Int

/-- Market value in minor units.

Units are scaled by a hundred and the price is scaled by a hundred, so their
product is scaled by ten thousand and has to come back down by a hundred to
land in minor units. When that does not divide exactly there is a ROUNDING
DECISION, and this refuses rather than making it — which way to round is a term
of an administration agreement, not a property of arithmetic. -/
def marketValue (units price : Int) : Option Int :=
  if units * price % 100 = 0 then some (units * price / 100) else none

/-- **Nothing is lost when it does answer.** The value, scaled back up, is
exactly the product — so the figure is the arithmetic and not a rounding of it. -/
theorem marketValue_is_exact (units price v : Int)
    (h : marketValue units price = some v) : v * 100 = units * price := by
  unfold marketValue at h
  by_cases hd : units * price % 100 = 0
  · simp [hd] at h
    subst h
    -- Linear once the product is one opaque quantity.
    generalize units * price = k at hd ⊢
    omega
  · simp [hd] at h

/-- **A whole number of units always values exactly**, so the refusal above can
only ever bite on a fractional holding. -/
theorem whole_units_value_exactly (u price : Int) :
    marketValue (u * 100) price = some (u * price) := by
  have h : u * 100 * price = u * price * 100 := by
    rw [Int.mul_assoc, Int.mul_comm 100 price, ← Int.mul_assoc]
  unfold marketValue
  rw [h]
  simp [Int.mul_emod_left, Int.mul_ediv_cancel]

/- ── Marking to market ─────────────────────────────────────────────────── -/

/-- What a mark moves: the difference between what the book holds a position at
and what it is worth. -/
def markDelta (carrying market : Int) : Int := market - carrying

/-- The entry a mark posts: the movement in the asset, and its equal and
opposite in unrealized gain.

**A POSTING, NOT AN ASSIGNMENT.** Overwriting the carrying value would be
simpler and would destroy the audit trail exactly where a reader most wants one
— "why is this worth more than we paid" has to have an entry to point at. -/
def markEntry (carrying market : Int) : List Int :=
  [markDelta carrying market, -(markDelta carrying market)]

/-- **A mark is balanced, so a marked book still ties.**

Conservation is not something valuation gets to opt out of. -/
theorem mark_conserves (carrying market : Int) :
    (markEntry carrying market).sum = 0 := by
  simp [markEntry, markDelta]
  omega

/-- **After marking, the book holds market value.** The point of the exercise,
as an equation. -/
theorem mark_lands_on_market (carrying market : Int) :
    carrying + markDelta carrying market = market := by
  simp [markDelta]; omega

/-- **Marking twice at one price posts a second entry of zero.**

Re-running a valuation is not a way to invent a gain, and an operator who marks
the book twice by accident has not changed it. -/
theorem mark_again_posts_nothing (market : Int) : markDelta market market = 0 := by
  simp [markDelta]

/-- **A later price posts only what changed.**

The two marks sum to the total movement rather than double-counting it, which is
what makes a mark safe to run every day. -/
theorem mark_is_the_difference (carrying first second : Int) :
    markDelta carrying first + markDelta first second = markDelta carrying second := by
  simp [markDelta]; omega

/-- Value a position at a day, given what has been observed about it. -/
def markAt (asOf : Int) (os : List Observation) (units : Units) : Option Int :=
  match markPrice asOf os with
  | none => none
  | some o => marketValue units o.minor

/-- **A position with no usable price yields NO mark, not a zero one.**

The difference is the whole thing. Zero says "worth what it cost"; none says "we
do not know what this is worth" — and a NAV struck over the first is wrong in a
way nobody can see, while the second blocks, which is what an unpriced holding
should do. -/
theorem unpriced_yields_no_mark (asOf : Int) (os : List Observation) (units : Units)
    (h : markPrice asOf os = none) : markAt asOf os units = none := by
  simp [markAt, h]

/-- And a mark, when there is one, came from an observation that existed on or
before the day it values. -/
theorem mark_uses_an_eligible_price
    (asOf : Int) (os : List Observation) (units : Units) (v : Int)
    (h : markAt asOf os units = some v) :
    ∃ o, markPrice asOf os = some o ∧ o.onDay ≤ asOf := by
  unfold markAt at h
  -- Established separately: `cases` on the price would rewrite the goal too,
  -- and then the hypothesis no longer says what the goal needs.
  have hsome : ∃ o, markPrice asOf os = some o := by
    cases hq : markPrice asOf os with
    | none => rw [hq] at h; simp at h
    | some o => exact ⟨o, rfl⟩
  obtain ⟨o, ho⟩ := hsome
  exact ⟨o, ho, markPrice_is_not_after asOf os o ho⟩

/- ── Whether a NAV may be struck at all ────────────────────────────────── -/

/-- What the book holds: an instrument, and how much of it. -/
structure Holding where
  instrument : String
  units : Int
deriving DecidableEq, Repr

/-- Whether a holding can be valued on the day.

The prices are given as a function from instrument to what has been observed
about it — the shape the fact log folds to, and the shape the Rust hands in. -/
def priced (asOf : Int) (prices : String → List Observation) (h : Holding) : Bool :=
  (markPrice asOf (prices h.instrument)).isSome

/-- The holdings that cannot be valued on the day. -/
def unpricedAt (asOf : Int) (prices : String → List Observation)
    (hs : List Holding) : List Holding :=
  hs.filter (fun h => !priced asOf prices h)

/-- Whether a NAV may be struck at the day: every holding has a price.

⛔ THE POINT OF THE WHOLE VALUATION LAYER. A net asset value computed over a
position nobody has priced is not a net asset value — it is a number with a
hole in it, and the hole is invisible in the figure. So this is a refusal, and
`strike_refuses_exactly_when_something_is_unpriced` says the refusal is total:
there is no path to a struck NAV with an unvalued holding in it. -/
def strikeable (asOf : Int) (prices : String → List Observation)
    (hs : List Holding) : Bool :=
  hs.all (priced asOf prices)

/-- A fund that holds nothing can be struck. Vacuous, and worth stating: the
refusal must not fire on an empty book. -/
theorem nothing_held_is_strikeable (asOf : Int) (prices : String → List Observation) :
    strikeable asOf prices [] = true := rfl

/-- **The refusal is exactly the unpriced list.** What blocks a strike is what
the screen shows as blocking it — one derivation, so the message and the
decision cannot disagree. -/
theorem strike_refuses_exactly_when_something_is_unpriced
    (asOf : Int) (prices : String → List Observation) (hs : List Holding) :
    strikeable asOf prices hs = true ↔ unpricedAt asOf prices hs = [] := by
  unfold strikeable unpricedAt
  rw [List.filter_eq_nil_iff, List.all_eq_true]
  constructor
  · intro h a ha; simp [h a ha]
  · intro h a ha
    have := h a ha
    simpa using this

/-- An observation from after the day does not change the eligible ones. -/
theorem eligible_ignores_later (asOf : Int) (o : Observation) (os : List Observation)
    (h : asOf < o.onDay) : eligible asOf (o :: os) = eligible asOf os := by
  have hd : decide (o.onDay ≤ asOf) = false := by simp; omega
  simp [eligible, List.filter, hd]

/-- **A price observed after the valuation date does not unblock it.**
Tomorrow's file is not evidence about today, so it cannot be what lets today be
struck — otherwise the strike could not be replayed from what existed when it
was taken. -/
theorem a_later_price_does_not_help
    (asOf : Int) (o : Observation) (os : List Observation) (hlater : asOf < o.onDay) :
    markPrice asOf (o :: os) = markPrice asOf os := by
  unfold markPrice
  rw [eligible_ignores_later asOf o os hlater]

/-- **A price can only ever unblock.** Observing something never takes away a
valuation that was already possible — so a strike that could be taken stays
takeable, and the queue of what blocks only shrinks as data arrives. -/
theorem observing_never_blocks
    (asOf : Int) (os : List Observation) (o : Observation)
    (h : (markPrice asOf os).isSome = true) :
    (markPrice asOf (o :: os)).isSome = true := by
  unfold markPrice at *
  unfold eligible at *
  cases hc : decide (o.onDay ≤ asOf) with
  | false => simpa [List.filter, hc] using h
  | true =>
    simp only [List.filter, hc]
    -- `latest` of a non-empty list is always some, whichever branch it takes.
    unfold latest
    split <;> simp

/- ── The net asset value moves by exactly the marks ────────────────────── -/

/-- The total a list of marks moves the book. -/
def totalMovement (deltas : List Int) : Int := deltas.foldl (· + ·) 0

/-- Every mark contributes a balanced pair, so however many positions are
marked, the book still ties. -/
theorem marks_conserve : ∀ (ds : List Int),
    (ds.map (fun d => d + -d)).foldl (· + ·) 0 = 0
  | [] => rfl
  | d :: rest => by
    simp only [List.map_cons, List.foldl_cons]
    have : d + -d = 0 := by omega
    simp [this]
    exact marks_conserve rest

end Ratio.Valuation
