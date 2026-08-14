set_option warningAsError true
/-! `Ratio.Tolerance` — how big a difference has to be before it stops the NAV.

⛔ A TERM OF AN AGREEMENT, NOT A PROPERTY OF THE SOFTWARE. Two funds looking at
the same 800.00 break disagree about whether it matters, and both are right:
one administration agreement says report anything over 5.00 and stop for
anything over 1,000.00, another sets both a decimal place out. A threshold
compiled into the engine makes it wrong somewhere instead of configurable
everywhere — the same argument `Ratio.Lots.Methods.isLongTerm` makes for taking
the holding period as a parameter rather than writing 365 into the definition.

⚠ THE BOUNDARY IS ON THE AMOUNT. A difference of exactly `blocksNav` blocks:
`a_difference_at_the_threshold_blocks_the_nav`. Off by one moves a break
between "stops the close" and "somebody looks at it tomorrow", and neither
figure looks unusual on a screen.

⛔ AND AN INVERTED PAIR IS NOT A TOLERANCE, IT IS A BAND NOTHING CAN ENTER.
With `blocksNav < belowNotice` the middle grade becomes unreachable — a queue
offering a state no difference can reach, which is
`Ratio.Lots.Posting.a_collided_chart_hides_the_gain` wearing different clothes:
the configuration parses, every figure ties, and one whole category silently
stops existing. `an_inverted_tolerance_makes_the_middle_band_unreachable` is
why `wellFormed` is checked when the configuration is READ rather than when a
break is graded.

What is emitted from here is the grading decision and the well-formedness
check, and nothing else — the walk over a report's lines stays in Rust. The
answer depends on nothing but the magnitude and the two bounds, which is what
lets a Rust loop and a Lean theorem be unable to disagree about a severity.

Integers only. Lean core + `omega`, no Mathlib, matching `Ratio.Core`. -/

namespace Ratio.Tolerance

/-- The two bounds, in minor units.

⛔ MINOR UNITS, BECAUSE MONEY IS NEVER A FLOAT HERE. A tolerance of "1000.00"
is 100000, and a fractional one cannot be written down rather than being
rejected by a check somebody might forget to run. -/
structure Tolerance where
  /-- At or above this, a difference is worth reporting. -/
  belowNotice : Int
  /-- At or above this, a difference stops the NAV. -/
  blocksNav : Int
deriving Repr, DecidableEq

/-- How serious one difference is. -/
inductive Severity where
  /-- Below notice. -/
  | low
  /-- Worth reporting, and not blocking. -/
  | medium
  /-- Stops the NAV. -/
  | high
deriving Repr, DecidableEq

/-- An ordering on severities, so "never less severe" can be stated. -/
def rank : Severity → Nat
  | .low => 0
  | .medium => 1
  | .high => 2

/-- How big a difference is, regardless of which way it goes.

⚠ A BREAK IS GRADED BY SIZE AND NOT BY SIGN. Our figure being 800.00 under
theirs is exactly as serious as being 800.00 over, and a grading that read the
sign would let half of every category through ungraded.
`severity_reads_only_the_magnitude` is that stated. -/
def magnitude (d : Int) : Int := if d < 0 then -d else d

/-- A tolerance whose bands are ordered and non-negative.

⛔ CHECKED WHEN THE CONFIGURATION IS READ. A tolerance that cannot express its
middle band is wrong the moment somebody writes it down, and finding out at the
first break means finding out on a NAV morning. Same placement, and the same
reason, as `Ratio.Lots.Posting`'s chart roles. -/
def wellFormed (t : Tolerance) : Bool :=
  decide (0 ≤ t.belowNotice ∧ t.belowNotice ≤ t.blocksNav)

/-- What one difference grades as. -/
def severity (t : Tolerance) (d : Int) : Severity :=
  if t.blocksNav ≤ magnitude d then .high
  else if t.belowNotice ≤ magnitude d then .medium
  else .low

/-- Whether one difference stops the NAV. -/
def blocks (t : Tolerance) (d : Int) : Bool :=
  match severity t d with
  | .high => true
  | _ => false

/-- The grading a fund gets when its configuration declares none.

⚠ BY CUSTOM, NOT BY ELECTION, and the distinction is the whole reason the
configuration field is an option rather than these two numbers. A fund that
declared nothing and a fund that declared exactly this are different facts, and
a screen reporting the second of a fund that did the first is asserting an
agreement nobody made. `Ratio.Lots.Methods` makes the same argument about FIFO. -/
def byCustom : Tolerance := { belowNotice := 500, blocksNav := 100000 }

/- ── The magnitude ──────────────────────────────────────────────────────── -/

/-- **A magnitude is never negative.** -/
theorem a_magnitude_is_never_negative (d : Int) : 0 ≤ magnitude d := by
  unfold magnitude; split <;> omega

/-- **A difference and its opposite are the same size.** -/
theorem the_opposite_difference_is_the_same_size (d : Int) :
    magnitude (-d) = magnitude d := by
  unfold magnitude; split <;> split <;> omega

/- ── The bands ──────────────────────────────────────────────────────────── -/

/-- **⚠ THE BOUNDARY IS ON THE AMOUNT: a difference of exactly the threshold
blocks the NAV.**

Stated rather than left to a reading of `≤`, because the alternative is a break
that is one minor unit short of stopping the close and does not, on a book where
nobody will look at the comparison again. The holding-period threshold is
written down for the same reason and
`Ratio.Lots.Methods.the_threshold_day_is_long_term` is the same shape. -/
theorem a_difference_at_the_threshold_blocks_the_nav
    (t : Tolerance) (h : wellFormed t = true) : blocks t t.blocksNav = true := by
  unfold wellFormed at h
  simp only [decide_eq_true_eq] at h
  unfold blocks severity magnitude
  have : ¬ (t.blocksNav < 0) := by omega
  simp [this]

/-- **⭐ A GRADING READS THE SIZE AND NOTHING ELSE.**

The Rust takes an absolute value before it asks; this is what makes that
faithful rather than merely conventional. -/
theorem severity_reads_only_the_magnitude (t : Tolerance) (d : Int) :
    severity t (-d) = severity t d := by
  unfold severity
  rw [the_opposite_difference_is_the_same_size]

/-- **A bigger difference is never less severe.**

The property a queue ordered by money is relying on: sorting by size cannot
put a blocking break under a reportable one. -/
theorem a_bigger_difference_is_never_less_severe
    (t : Tolerance) (a b : Int) (h : magnitude a ≤ magnitude b) :
    rank (severity t a) ≤ rank (severity t b) := by
  by_cases hb2 : t.blocksNav ≤ magnitude b
  · have hbh : severity t b = .high := by unfold severity; simp [hb2]
    rw [hbh]
    unfold severity
    split
    · simp [rank]
    · split <;> simp [rank]
  · by_cases hb1 : t.belowNotice ≤ magnitude b
    · have hbm : severity t b = .medium := by unfold severity; simp [hb2, hb1]
      have ha2 : ¬ (t.blocksNav ≤ magnitude a) := by omega
      rw [hbm]
      unfold severity
      rw [if_neg ha2]
      split <;> simp [rank]
    · have hbl : severity t b = .low := by unfold severity; simp [hb2, hb1]
      have ha2 : ¬ (t.blocksNav ≤ magnitude a) := by omega
      have ha1 : ¬ (t.belowNotice ≤ magnitude a) := by omega
      rw [hbl]
      unfold severity
      simp [ha2, ha1, rank]

/-- **⭐ EVERY DIFFERENCE LANDS IN EXACTLY ONE BAND, AND THE BANDS ARE THE
BOUNDS.**

Totality is the point: there is no difference the grading declines to grade,
and no difference that grades as two things. A partial grading would leave a
break with no severity, which a queue ordered by severity would silently drop. -/
theorem the_three_bands_partition_every_difference
    (t : Tolerance) (h : wellFormed t = true) (d : Int) :
    (severity t d = .high ↔ t.blocksNav ≤ magnitude d)
    ∧ (severity t d = .medium ↔
        (t.belowNotice ≤ magnitude d ∧ magnitude d < t.blocksNav))
    ∧ (severity t d = .low ↔ magnitude d < t.belowNotice) := by
  unfold wellFormed at h
  simp only [decide_eq_true_eq] at h
  unfold severity
  by_cases hb : t.blocksNav ≤ magnitude d
  · simp only [hb, if_pos]
    refine ⟨?_, ?_, ?_⟩ <;> simp <;> omega
  · by_cases hn : t.belowNotice ≤ magnitude d
    · simp only [hb, hn, if_neg, if_pos, not_false_eq_true]
      refine ⟨?_, ?_, ?_⟩ <;> simp <;> omega
    · simp only [hb, hn, if_neg, not_false_eq_true]
      refine ⟨?_, ?_, ?_⟩ <;> simp <;> omega

/-- **⛔ AN INVERTED TOLERANCE MAKES THE MIDDLE BAND UNREACHABLE.**

`belowNotice` above `blocksNav` is not a strict tolerance or a lenient one, it
is a tolerance with a category nothing can ever be in — every difference is
either blocking or beneath notice, and the screen offering "in review" is
offering a state no book can reach. This is why `wellFormed` fails the
configuration on the way in rather than grading with it. -/
theorem an_inverted_tolerance_makes_the_middle_band_unreachable
    (t : Tolerance) (h : t.blocksNav < t.belowNotice) (d : Int) :
    severity t d ≠ .medium := by
  unfold severity
  split
  · simp
  · rename_i hb
    split
    · exfalso; omega
    · simp

/-- **⛔ A TOLERANCE OF ZERO BLOCKS ON EVERYTHING, INCLUDING NOTHING.**

The regression this file exists to make loud. `#[derive(Default)]` on the Rust
side gives every field its type's zero, and a `blocksNav` of zero grades every
difference — including a difference of nothing at all — as stopping the NAV, on
every fund whose configuration came through the wrong door. `Ratio.Lots.Methods`
records the same hazard for a holding period of zero days, which classifies
every disposal as long-term at the favorable rate. -/
theorem a_tolerance_of_zero_blocks_on_everything
    (t : Tolerance) (h : t.blocksNav = 0) (d : Int) : severity t d = .high := by
  unfold severity
  have := a_magnitude_is_never_negative d
  simp [h, this]

/-- **The custom grading is well formed.**

⚠ Not decoration: it is the guard on the two numbers a fund gets when it
elected nothing, and `a_tolerance_of_zero_blocks_on_everything` is what a
derived default would have made of them. -/
theorem the_custom_tolerance_is_well_formed : wellFormed byCustom = true := by
  decide

/-- **Nothing is beneath notice under the custom grading by accident.**

A witness that the bands are actually apart, so the partition theorem above is
not being read over a tolerance whose three cases are one case. -/
theorem the_custom_grading_uses_all_three_bands :
    severity byCustom 0 = .low
    ∧ severity byCustom 1000 = .medium
    ∧ severity byCustom 200000 = .high := by
  refine ⟨?_, ?_, ?_⟩ <;> decide

end Ratio.Tolerance
