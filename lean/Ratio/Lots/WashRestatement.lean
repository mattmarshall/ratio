import Ratio.Lots
import Ratio.Lots.Wash

set_option warningAsError true
/-! `Ratio.Lots.WashRestatement` — a struck figure that a later wash moved.

`Ratio.Lots.Wash` proves the arithmetic: how much of a loss a repurchase
defers, that a gain is never washed, and that the deferral nets out. Every
one of those theorems is about a sale and a repurchase considered TOGETHER.

⛔ AND THEY ARE NOT AVAILABLE TOGETHER. The repurchase can arrive up to a
window AFTER the sale, and a NAV is struck in between. At the moment of the
strike the loss is real, allowable, and reported. Days later a repurchase
lands inside the window and the same sale's realized gain is a different
number — retroactively, for a period that is closed.

This is not the staleness `//tla:relief_engine_check` models. There, a relief
read one state and wrote against another, and the fix was to pin what it
read. Here NOTHING WAS STALE. The strike read the whole journal, pinned its
prefix, and computed the only correct answer available at the time. The
figure changed because a FUTURE event reached backwards, which is a property
of the tax rule and not a defect in the engine.

⚠ SO THE OBLIGATION IS NOT "DO NOT LET IT CHANGE". It is: a figure somebody
was paid on must not change SILENTLY. Either the strike carries a
qualification saying its wash window is still open, or a later repurchase
that moves it produces a restatement. An engine that simply recomputes
leaves two different answers to "what was the realized gain in March", both
reproducible, both digested, and no record that they differ.

⛔ AND THE TEMPTING FIX IS WRONG. Applying the adjustment only to periods
still open — prospectively — keeps every struck figure stable and reports a
realized gain the tax rule does not agree with. It trades a visible
restatement for an invisible error, which is the trade this whole system
exists to refuse.

⭐ A RESTATEMENT IS A NEW KIND OF THING THAT CITES WHAT IT SUPERSEDES.
`Ratio.Period` forbids a second value occupying the same day: "a
restatement, when it is wanted, has to be a new kind of thing that cites
what it supersedes — not a second value quietly occupying the same day."
This file is that kind of thing, for the one rule that genuinely reaches
backwards. Rewriting the struck figure in place keeps the citeable identity
and changes the number — which is the silent defect, not a restatement.

⚠ NOT AN `Order`, NOT A `Method`, AND NOT `lot_method = "wash"`. The
restatement is a record about a figure that already exists. Inventing a
relief variant for it would smuggle a reporting obligation into lot
selection.

`//tla:wash_restatement_check` is the sequence. The probes
`//tla:silent_wash_restatement_check` and
`//tla:mutating_wash_restatement_check` flip one dial each.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Lots

/- ── The citeable identity ─────────────────────────────────────────────── -/

/-- A citeable identity of a struck figure: the journal prefix it folded.

⛔ NOT THE FIGURE. Two strikes can report the same number from different
prefixes; the identity is which prefix was read. Rewriting the number while
keeping this id is the silent defect — the digest still cites, and the
number is somebody else's. -/
structure StrikeId where
  prefix : Nat
deriving DecidableEq, Repr

/-- A realized gain that was struck — the number somebody was paid on.

`qualified` is written at strike time: the wash window was still open, so
this figure can still move. Adding the flag later, once a repurchase has
arrived, is a restatement — a different thing, and one that has already
been read. -/
structure StruckGain where
  id : StrikeId
  soldOn : Int
  figure : Int
  qualified : Bool
deriving DecidableEq, Repr

/-- A restatement: a new record that cites the strike it supersedes.

`original` is the figure as struck. `movedTo` is what the tax rule now
assigns. The strike itself is not a field here and cannot become one —
putting the new number on the strike is `rewriteInPlace`, which this type
exists to refuse. -/
structure Restatement where
  cites : StrikeId
  original : Int
  movedTo : Int
deriving DecidableEq, Repr

/- ── Strike, qualify, restate ──────────────────────────────────────────── -/

/-- Whether the wash window is still open on a sale, on this day.

A strike taken after the window has closed needs no qualification: nothing
can move it. Flagging those too would train a reader to ignore the flag. -/
def windowOpen (window soldOn day : Int) : Bool :=
  decide (day ≤ soldOn + window)

/-- Strike a realized gain. Qualifies iff the window is still open.

⭐ THE QUALIFICATION IS WRITTEN AT STRIKE TIME, because that is the only
moment the engine knows the window is open and the reader has not yet
relied on the figure. -/
def strikeGain (id : StrikeId) (window soldOn day figure : Int) : StruckGain :=
  ⟨id, soldOn, figure, windowOpen window soldOn day⟩

/-- A later repurchase that washes a struck sale.

Returns a restatement citing the strike, or `none` if the repurchase does
not move this figure (outside the window, or the number did not change).
Never returns a mutated `StruckGain` — that is the point. -/
def restate (s : StruckGain) (window buyDay newFigure : Int) : Option Restatement :=
  if inWashWindow window s.soldOn buyDay && !decide (newFigure = s.figure) then
    some ⟨s.id, s.figure, newFigure⟩
  else none

/-- The forbidden operation: overwrite the struck figure and keep the id.

Defined so the theorem can name it. An engine that "updated the strike" is
this, not `restate`. -/
def rewriteInPlace (s : StruckGain) (newFigure : Int) : StruckGain :=
  { s with figure := newFigure }

/-- Whether the record said the figure can move, or said it did.

The third case — struck clean, changed quietly — is `false`. -/
def saysSo (s : StruckGain) (r : Option Restatement) : Bool :=
  s.qualified || r.isSome

/- ── Qualification is written at strike time ───────────────────────────── -/

/-- **A strike taken after the window has closed is not qualified.**

Day 131 is outside a thirty-day window of a sale on day 100. Nothing can
still move this figure, so the flag would be a lie. -/
theorem a_closed_window_is_not_qualified :
    (strikeGain ⟨7⟩ 30 100 131 (-1000)).qualified = false := by
  decide

/-- **And a strike taken while the window is open says so.**

Day 105 is inside the same window. The figure can still move. -/
theorem an_open_window_is_qualified :
    (strikeGain ⟨7⟩ 30 100 105 (-1000)).qualified = true := by
  decide

/-- **The window closing is the same day the wash window closes**, not a
horizon of the engine's invention. Day 130 is still open; day 131 is not. -/
theorem the_window_closes_on_the_last_in_window_day :
    windowOpen 30 100 130 = true ∧ windowOpen 30 100 131 = false := by
  decide

/- ── Restatement cites; it does not mutate ─────────────────────────────── -/

/-- **⭐ RESTATEMENT CITES THE STRIKE IT SUPERSEDES.**

A sale struck at prefix 7 for −1000, then a repurchase on day 110 that
moves the figure to −600. The restatement names prefix 7 and the original
number. The strike still says −1000. -/
theorem restatement_cites_the_strike_it_supersedes :
    let s := strikeGain ⟨7⟩ 30 100 105 (-1000)
    let r := restate s 30 110 (-600)
    s.figure = -1000 ∧ r = some ⟨⟨7⟩, -1000, -600⟩ := by
  decide

/-- **And a restatement of any moved figure cites that strike's id and
original number** — not a generalisation that `decide` happened to see. -/
theorem restatement_carries_the_identity_and_the_original
    (s : StruckGain) (window buyDay newFigure : Int) (r : Restatement)
    (h : restate s window buyDay newFigure = some r) :
    r.cites = s.id ∧ r.original = s.figure ∧ r.movedTo = newFigure := by
  unfold restate at h
  split at h
  · simp at h
    subst h
    exact ⟨rfl, rfl, rfl⟩
  · simp at h

/-- **A wash that does not move the figure produces no restatement.**

Same number, in-window repurchase: there is nothing to say. Inventing a
restatement here would train a reader to ignore the ones that matter. -/
theorem a_wash_that_does_not_change_the_figure_does_not_restate :
    let s := strikeGain ⟨7⟩ 30 100 105 (-1000)
    restate s 30 110 (-1000) = none := by
  decide

/-- **And a repurchase outside the window does not restate**, even when the
number differs — that difference is a different sale, not a wash. -/
theorem a_wash_outside_the_window_does_not_restate :
    let s := strikeGain ⟨7⟩ 30 100 105 (-1000)
    restate s 30 200 (-600) = none := by
  decide

/-- **A later wash that did move the figure is restated**, whether or not
the strike was qualified. Qualification is for the reader at strike time;
restatement is for the move. An engine that skipped this call is the
silent path. -/
theorem a_moved_figure_is_restated
    (s : StruckGain) (window buyDay newFigure : Int)
    (hw : inWashWindow window s.soldOn buyDay = true)
    (hne : newFigure ≠ s.figure) :
    (restate s window buyDay newFigure).isSome = true := by
  unfold restate
  have hne' : decide (newFigure = s.figure) = false := decide_eq_false hne
  simp [hw, hne']

/- ── The obligation ────────────────────────────────────────────────────── -/

/-- **⭐ A STRUCK GAIN THAT MOVED SAYS SO.**

A silent strike (no qualification) of −1000, then a restatement to −600:
`saysSo` is true because the restatement exists. A qualified strike with
no restatement yet is also true — the flag said the figure could move.
The third case is the one the TLA forbids. -/
theorem a_silent_strike_that_was_restated_says_so :
    let s : StruckGain := ⟨⟨7⟩, 100, -1000, false⟩
    saysSo s (restate s 30 110 (-600)) = true := by
  decide

/-- **A qualified strike says so even before anything moves.** The flag is
the record; waiting for the repurchase to write it is too late. -/
theorem a_qualified_strike_says_so_before_it_moves :
    let s := strikeGain ⟨7⟩ 30 100 105 (-1000)
    saysSo s none = true := by
  decide

/-- **⛔ AND A SILENT STRIKE WITH NO RESTATEMENT DOES NOT SAY SO.**

Struck clean, nothing recorded. This is the third case:
`AStruckGainThatMovedSaysSo` goes red. Stated so "we handle restatement"
cannot be said of an engine that neither qualified nor restated. -/
theorem a_silent_strike_with_no_restatement_does_not_say_so :
    let s : StruckGain := ⟨⟨7⟩, 100, -1000, false⟩
    saysSo s none = false := by
  decide

/-- **The obligation, for any strike and any later wash that moved it.**

If the repurchase is in the window and the number changed, either the
strike was already qualified or `restate` produced a record. An engine
that called `restate` satisfies this by the second disjunct; one that
only qualified satisfies it by the first. One that did neither is
`rewriteInPlace`. -/
theorem a_struck_gain_that_moved_says_so
    (s : StruckGain) (window buyDay newFigure : Int)
    (hw : inWashWindow window s.soldOn buyDay = true)
    (hne : newFigure ≠ s.figure) :
    saysSo s (restate s window buyDay newFigure) = true := by
  have hr := a_moved_figure_is_restated s window buyDay newFigure hw hne
  unfold saysSo
  simp [hr]

/- ── Silent rewrite is not a restatement ───────────────────────────────── -/

/-- **⛔ REWRITING IN PLACE KEEPS THE ID AND CHANGES THE FIGURE.**

Prefix 7 still cites; the number is now −600; nothing is qualified. The
digest still resolves. This is the defect, named so it cannot be confused
with `restate`. -/
theorem rewriting_in_place_keeps_the_id_and_changes_the_figure :
    let s : StruckGain := ⟨⟨7⟩, 100, -1000, false⟩
    let s' := rewriteInPlace s (-600)
    s'.id = s.id ∧ s'.figure = -600 ∧ s'.qualified = false := by
  decide

/-- **And the rewrite is a different value from the original**, so an
equality test on the strike would notice — if anybody kept the original.
The restatement path does not need that luck: it never produces a
`StruckGain`. -/
theorem rewriting_in_place_is_not_the_original_strike :
    let s : StruckGain := ⟨⟨7⟩, 100, -1000, false⟩
    rewriteInPlace s (-600) ≠ s := by
  decide

/-- **Restatement and rewrite answer different questions.** Same sale, same
repurchase, same new number: `restate` yields a cite of the original;
`rewriteInPlace` yields a strike that claims to be the original. -/
theorem restatement_and_rewrite_are_not_the_same_operation :
    let s : StruckGain := ⟨⟨7⟩, 100, -1000, false⟩
    restate s 30 110 (-600) = some ⟨⟨7⟩, -1000, -600⟩
    ∧ rewriteInPlace s (-600) = ⟨⟨7⟩, 100, -600, false⟩ := by
  constructor <;> decide

end Ratio.Lots
