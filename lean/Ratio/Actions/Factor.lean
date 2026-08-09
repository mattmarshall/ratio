import Ratio.Actions

set_option warningAsError true
/-! `Ratio.Actions.Factor` — a split as something you read through, rather than
something you write.

`Ratio.Closure` makes corporate actions the term that dominates a period end:
`actionCost = openActions * securities` is the only superlinear one, and one
unapplied action costs more reads than marking every security. That cost is
entirely an artifact of HOW an action is applied — rewriting every affected lot.
A split on one S&P name rewrites forty thousand lots.

The alternative is to leave the lots alone and compose the splits into a FACTOR
read at the point of use. Nothing is rewritten, so the bulk cost disappears.

⛔ AND THE FIRST THING TO PROVE IS THAT IT IS NOT THE SAME THING, because it
is not. `a_factor_can_succeed_where_the_rewrite_refuses` is a counterexample,
not a caveat: a 3-for-2 then a 2-for-1 on five units REFUSES under the rewrite
— fifteen halves are not shares, and the holder was paid cash in lieu — while
the composed factor is 6/2, which divides, and silently returns fifteen. Both
are arithmetically fine. One of them loses the half-share the holder was
actually paid for.

So the equivalence holds under a hypothesis and not in general, and the
hypothesis is exactly "every intermediate step divided". That is what
`applyAll_scales_units` is stated to capture — MULTIPLICATIVELY, so no division
appears in the statement and the theorem cannot be true by rounding.

⛔ THE SECOND THING IS REPLAY. `ratio-nav` promises a strike can be re-derived
from the journal prefix it pinned. Under a rewrite that holds for free: the
applied action IS an entry in the prefix. Under a factor there is nothing to
apply, and the factor is computed from the ANNOUNCED set — so if announcements
live anywhere but the journal, a replay run later reads a bigger set and
produces a different number than the strike recorded.
`replay_is_determined_by_the_prefix` is that obligation written down. It is why
announcements have to become journal entries, and it is a precondition of the
redesign rather than a detail of it.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Actions

/-- The splits of one instrument, composed into a single ratio.

Oldest first, though multiplication does not care — what the order decides is
whether the REWRITE succeeds, which is the whole content of the counterexample
below. -/
def compose : List Ratio → Ratio
  | [] => ⟨1, 1⟩
  | r :: rest =>
    let f := compose rest
    ⟨r.num * f.num, r.den * f.den⟩

/-- **A composed factor describes the rewrite it stands in for.**

⛔ STATED AS A MULTIPLICATION, deliberately. Written with division it would be a
claim about two roundings agreeing, and `Ratio.Lots` is a standing reminder that
conservation can hold while the wrong amount moves. Here there is nothing to
round: whenever the rewrite succeeds, the units it produces are exactly what the
composed factor scales the original to.

Note what this does NOT say: that the factor succeeds whenever the rewrite does
not. It does, sometimes, and that is the next theorem. -/
theorem applyAll_scales_units :
    ∀ (rs : List Ratio) (l m : Lot), applyAll rs l = some m →
      m.units * (compose rs).den = l.units * (compose rs).num
  | [], l, m, h => by
    simp [applyAll] at h
    subst h
    simp [compose]
  | r :: rest, l, m, h => by
    unfold applyAll at h
    cases hk : split r l with
    | none => rw [hk] at h; simp at h
    | some k =>
      rw [hk] at h
      have ih := applyAll_scales_units rest k m h
      have hs := split_scales_units r l k hk
      simp only [compose]
      calc m.units * (r.den * (compose rest).den)
          = m.units * (compose rest).den * r.den := by
            rw [Int.mul_comm r.den (compose rest).den, ← Int.mul_assoc]
        _ = k.units * (compose rest).num * r.den := by rw [ih]
        _ = k.units * r.den * (compose rest).num := by
            rw [Int.mul_assoc, Int.mul_comm (compose rest).num r.den, ← Int.mul_assoc]
        _ = l.units * r.num * (compose rest).num := by rw [hs]
        _ = l.units * (r.num * (compose rest).num) := by rw [Int.mul_assoc]

/-- Read a lot's units through a factor, refusing a result that is not whole. -/
def unitsThrough (f : Ratio) (l : Lot) : Option Int :=
  if f.den = 0 ∨ l.units * f.num % f.den ≠ 0 then none
  else some (l.units * f.num / f.den)

/-- **⛔ A FACTOR CAN SUCCEED WHERE THE REWRITE REFUSES, AND IT IS THE FACTOR
THAT IS WRONG.**

Five units. A 3-for-2, then a 2-for-1.

  rewrite   5 × 3 = 15, and 15 does not halve — REFUSED. The holder held seven
            shares and was paid cash for the half, which realizes a gain and is
            a posting the configuration has to declare.
  factor    compose is ⟨6, 2⟩. 5 × 6 = 30, which halves. FIFTEEN.

Both are correct arithmetic. The factor is wrong about the world: it reports
the holding the shareholder would have had if the intermediate fraction had
survived, and it did not — it was sold.

⚠ SO THE REDESIGN IS NOT A FREE SPEED-UP. The factor may replace the rewrite
only where every intermediate step divides, which has to be CHECKED per lot
rather than assumed for the instrument. That check is O(splits), not O(lots),
so the win survives — forty thousand lots are still not rewritten — but the
version of this idea that skips the check computes a number nobody is owed. -/
theorem a_factor_can_succeed_where_the_rewrite_refuses :
    applyAll [⟨3, 2⟩, ⟨2, 1⟩] ⟨1, 5, 100⟩ = none
    ∧ unitsThrough (compose [⟨3, 2⟩, ⟨2, 1⟩]) ⟨1, 5, 100⟩ = some 15 := by
  constructor
  · simp [applyAll, split]
  · simp [unitsThrough, compose]

/-- And where every step does divide, the two agree — so the optimization is
available exactly on the lots where it is sound. -/
theorem they_agree_when_every_step_divides :
    applyAll [⟨3, 2⟩, ⟨2, 1⟩] ⟨1, 4, 100⟩ = some ⟨1, 12, 100⟩
    ∧ unitsThrough (compose [⟨3, 2⟩, ⟨2, 1⟩]) ⟨1, 4, 100⟩ = some 12 := by
  constructor
  · simp [applyAll, split]
  · simp [unitsThrough, compose]

/- **A factor changes no cost** — and this is deliberately NOT a theorem.

`Ratio.Actions.split_conserves_cost` had to be proved because `split` returns a
`Lot` and could have touched the cost field. `unitsThrough` returns an `Int`;
there is no cost in its result to conserve, so the property holds by TYPING
rather than by argument.

⚠ I wrote it as a theorem first — `l.cost = l.cost := rfl`, with the factor and
the lot as unused hypotheses. `warningAsError` rejected it for the unused
binder, which was the right answer for the wrong reason: the statement was a
tautology dressed up in the vocabulary of the thing it claimed to be about, and
it would have counted toward the file's theorem count while proving nothing. A
representation that cannot express a revaluation is stronger than one proved not
to perform one, and saying so in prose is honest where a `rfl` is not. -/

/- ── Announcements, and why they have to be in the journal ─────────────── -/

/-- A corporate action as announced: when it takes effect, and by how much. -/
structure Announcement where
  /-- Days since an epoch. A `Nat` so it compares in date order. -/
  exDate : Nat
  ratio : Ratio
deriving Repr

/-- The factor in force for a valuation day, reading only the announcements the
journal held at a pinned position.

⛔ `pinned` IS A JOURNAL POSITION, NOT A COUNT OF ANNOUNCEMENTS. That is the
whole design constraint: under a rewrite, the applied action is an entry and a
strike's prefix determines it for free. Under a factor, nothing is applied — so
unless announcements are themselves entries, the prefix does not determine the
factor and a replay reads whatever has arrived since. -/
def factorAt (as : List Announcement) (pinned day : Nat) : Ratio :=
  compose (((as.take pinned).filter (fun a => a.exDate ≤ day)).map (·.ratio))

/-- **⭐ A STRIKE'S FACTOR IS FIXED BY THE PREFIX IT PINNED.**

Announcements arriving after the pinned position cannot change what a strike
folded, so re-deriving it tomorrow lands on the number it recorded. This is
`ratio-nav`'s replay contract carried into the factor representation, and the
reason the redesign REQUIRES announcements to be journal entries rather than a
side log — the current `Plane::Actions` is not pinned by a strike, so a factor
computed from it would produce a different figure on every replay as the world
told us more.

`Ratio.Period.one_answer_per_day` refuses restatement. A replay that quietly
answered differently would not be a restatement — it would be worse, because
nothing would announce that the figure had moved. -/
theorem replay_is_determined_by_the_prefix
    (as bs : List Announcement) (pinned day : Nat) (h : pinned ≤ as.length) :
    factorAt (as ++ bs) pinned day = factorAt as pinned day := by
  simp [factorAt, List.take_append_of_le_length h]

/-- **⛔ AND WITHOUT THAT HYPOTHESIS IT IS FALSE — which is the real failure.**

`pinned ≤ as.length` is not bookkeeping. A pin that reaches past what the
journal actually held reads whatever arrived later, and the counterexample is
the smallest one there is: an empty journal pinned at position one, then a
2-for-1 arrives.

  factorAt []  1 day  =  ⟨1, 1⟩        the strike saw nothing
  factorAt [a] 1 day  =  ⟨2, 1⟩        the replay sees the split

Same pin, same day, different answer — so the figure moves and nothing says it
did. ⚠ THIS IS EXACTLY WHAT PINNING AGAINST `Plane::Actions` WOULD DO: a strike
pins a JOURNAL position, and a side log's length has no relationship to it, so
the hypothesis that makes the theorem true would simply not hold. The mutation
test for this only showed the hypothesis was REFERENCED. This shows it is
load-bearing. -/
theorem an_unpinned_announcement_changes_the_answer :
    factorAt [] 1 5 ≠ factorAt ([] ++ [⟨3, ⟨2, 1⟩⟩]) 1 5 := by
  simp [factorAt, compose]

/-- **⛔ ANNOUNCING THE SAME ACTION TWICE DOUBLES THE FACTOR.**

The failure mode does not disappear under this representation, it MOVES.
`Ratio.Actions.applying_twice_is_not_applying_once` was about a write happening
twice, and the guard was the record of having written. Here nothing is written,
so that guard has nowhere to live — a duplicate ANNOUNCEMENT of one economic
event silently squares the split.

⚠ THAT IS A BETTER PLACE FOR IT, and the reason is worth stating: a duplicate
announcement is a property of a SET, checkable at the moment a record arrives
and against a stable key, whereas "has this been applied" is a property of the
whole journal that has to be recomputed and can be raced. But it is not gone,
and a migration that assumed idempotence came free would double every split. -/
theorem announcing_twice_doubles_the_factor :
    compose [⟨2, 1⟩, ⟨2, 1⟩] ≠ compose [⟨2, 1⟩] := by
  simp [compose]

/-- Nothing announced is the identity, so an instrument with no corporate
history reads straight through — the base case that makes this cheap. -/
theorem no_announcement_is_the_identity (l : Lot) :
    unitsThrough (compose []) l = some l.units := by
  simp [unitsThrough, compose]

/-- And a day before anything took effect sees the identity too, whatever has
been announced since — which is what makes a historic NAV answerable. -/
theorem a_day_before_the_first_ex_date_sees_nothing
    (as : List Announcement) (pinned : Nat) (h : ∀ a ∈ as, 0 < a.exDate) :
    factorAt as pinned 0 = ⟨1, 1⟩ := by
  simp only [factorAt]
  have : ((as.take pinned).filter (fun a => a.exDate ≤ 0)) = [] := by
    apply List.filter_eq_nil_iff.mpr
    intro a ha
    have := h a (List.mem_of_mem_take ha)
    simp
    omega
  rw [this]
  rfl

end Ratio.Actions
