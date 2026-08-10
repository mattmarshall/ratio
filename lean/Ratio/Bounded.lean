set_option warningAsError true
/-! `Ratio.Bounded` — the hypothesis every theorem in this repo was assuming.

Every proof here is stated over Lean's `Int`, which is unbounded. Every emitted
function runs on `i64`, which is not. Nothing had ever written down what happens
in the gap, and the answer turns out to be worse than a crash:

    4_000_000_000_000_000_000 * 3        wraps to  -6446744073709551616
    (-6446744073709551616).rem_euclid 2  ==        0

⛔ SO A DIVISIBILITY GUARD SAYS YES ABOUT A PRODUCT THAT NEVER HAPPENED. The
proof cannot see it, because in `Int` the multiplication simply happened. This
is not a rounding difference or a precision question; it is a different number.

Measured on the emitted surface, three of these are live:

    per_share(100, 0)     PANICS — divide by zero. A fund with no units in
                          issue crashes the NAV path.
    per_share(MIN, -1)    PANICS — the one division in two's complement whose
                          result is not representable.
    money_neg(MIN)        WRAPS TO ITSELF. Negation is not always negation.

This module is what those checks are checked against. It does not make
arithmetic safe — nothing can, at a fixed width — it makes the REFUSAL exact:
an operation either agrees with the `Int` theorem or declines to answer, and
there is no third case where it answers differently.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Bounded

/-- What a signed 64-bit integer can hold. -/
def lo : Int := -9223372036854775808
def hi : Int := 9223372036854775807

/-- Whether a value is representable at all. -/
def fits (x : Int) : Bool := decide (lo ≤ x) && decide (x ≤ hi)

/-- **The range is not symmetric**, and that asymmetry is the whole reason
negation can fail. There is one more negative value than positive. -/
theorem the_range_is_not_symmetric : ¬ fits (-lo) := by decide

/- ── The operations ───────────────────────────────────────────────────── -/

/-- Addition, or nothing. -/
def add (a b : Int) : Option Int := if fits (a + b) then some (a + b) else none

/-- Multiplication, or nothing. -/
def mul (a b : Int) : Option Int := if fits (a * b) then some (a * b) else none

/-- Negation, or nothing. ⛔ It can fail, at exactly one input. -/
def neg (a : Int) : Option Int := if fits (-a) then some (-a) else none

/-- Subtraction, or nothing.

⛔ **NOT `add a (-b)`**, and the difference is a real one. Writing a difference
that way asks for `-b` first, and at `b = lo` that negation is the one input
`neg` refuses — so the subtraction declines at an input where the DIFFERENCE is
perfectly representable. `-1 - lo` is exactly `hi`; `-lo` is not a value. -/
def sub (a b : Int) : Option Int := if fits (a - b) then some (a - b) else none

/-- Euclidean division, or nothing.
⛔ TWO WAYS TO FAIL AND THEY ARE DIFFERENT: a zero divisor is a question with no
answer, and `lo / -1` is a question whose answer will not fit. -/
def div (a b : Int) : Option Int :=
  if b = 0 then none else if fits (a / b) then some (a / b) else none

/- ── What they guarantee ──────────────────────────────────────────────── -/

/-- **⭐ WHEN IT ANSWERS, IT AGREES WITH THE THEOREM.**

The property the whole module exists for. Every proof in this repo is about
`a + b` in `Int`; this says the bounded operation returns that exact value or
nothing at all. There is no third case in which it returns something else, which
is precisely the case `i64` has by default. -/
theorem add_agrees (a b c : Int) (h : add a b = some c) : c = a + b := by
  unfold add at h; split at h <;> simp_all

theorem mul_agrees (a b c : Int) (h : mul a b = some c) : c = a * b := by
  unfold mul at h; split at h <;> simp_all

theorem neg_agrees (a c : Int) (h : neg a = some c) : c = -a := by
  unfold neg at h; split at h <;> simp_all

theorem sub_agrees (a b c : Int) (h : sub a b = some c) : c = a - b := by
  unfold sub at h; split at h <;> simp_all

/-- **⛔ AND SUBTRACTION IS STRICTLY WEAKER WHEN ROUTED THROUGH NEGATION.**

The concrete case: `-1 - lo` is exactly `hi`, so `sub` answers it; `neg lo` has
no answer at all, so `add a (neg b)` never gets to ask. A difference routed
through negation declines a question the difference itself answers correctly.
This is why the Rust has its own `sub` rather than composing the two — the
composition is not the same function.

⚠ The witness has to be a NEGATIVE `a`. `0 - lo` is `-lo`, which is the value
that does not exist — `decide` reported the first version of this theorem FALSE
for exactly that reason. -/
theorem routing_a_difference_through_negation_loses_an_answer :
    sub (-1) lo = some hi ∧ neg lo = none := by
  constructor <;> decide

theorem div_agrees (a b c : Int) (h : div a b = some c) : c = a / b := by
  unfold div at h; split at h
  · simp at h
  · split at h <;> simp_all

/-- **⭐ AND ANY PROPERTY OF THE UNBOUNDED ANSWER HOLDS OF THE BOUNDED ONE.**

The transfer. `Ratio.Lots.cost_is_conserved` and every theorem like it is stated
over `Int`; this is what lets those conclusions be drawn about a machine, and
the hypothesis they always needed was that the operation answered at all. -/
theorem a_property_of_the_sum_holds_of_the_answer
    (P : Int → Prop) (a b c : Int) (h : add a b = some c) (hp : P (a + b)) : P c := by
  rw [add_agrees a b c h]; exact hp

theorem a_property_of_the_product_holds_of_the_answer
    (P : Int → Prop) (a b c : Int) (h : mul a b = some c) (hp : P (a * b)) : P c := by
  rw [mul_agrees a b c h]; exact hp

/-- **It refuses exactly when it must, and not before.**

A conservative check would be sound and useless — refusing arithmetic that would
have been fine is a system that stops working on large funds. This is `iff`. -/
theorem add_refuses_exactly_when_it_must (a b : Int) :
    add a b = none ↔ fits (a + b) = false := by
  unfold add; split <;> simp_all

theorem mul_refuses_exactly_when_it_must (a b : Int) :
    mul a b = none ↔ fits (a * b) = false := by
  unfold mul; split <;> simp_all

/- ── The three that are live ──────────────────────────────────────────── -/

/-- **⛔ `per_share(100, 0)` PANICS, and this refuses.**

A fund with no units in issue is not exotic — it is a fund on the day it is
established, and every fund has one. Rust's `div_euclid` panics on a zero
divisor in every build profile, so this is a crash on the NAV path rather than a
wrong number. -/
theorem a_zero_divisor_is_refused (a : Int) : div a 0 = none := by
  simp [div]

/-- **⛔ AND `per_share(MIN, -1)` PANICS TOO** — the one division in two's
complement whose answer is not representable, because the range is not
symmetric. Nothing about the inputs looks unusual. -/
theorem the_one_division_that_does_not_fit : div lo (-1) = none := by
  decide

/-- **⛔ AND NEGATION IS NOT ALWAYS NEGATION.** `0 - i64::MIN` is `i64::MIN`, so
`money_neg` returns its argument unchanged and the sign silently does not
flip — in a kernel whose entire subject is that debits and credits cancel. -/
theorem negating_the_floor_is_refused : neg lo = none := by
  decide

/-- And everything ordinary goes through, which is what makes the refusals worth
having rather than an obstacle. -/
theorem ordinary_arithmetic_is_untouched :
    add 1000000 2000000 = some 3000000
    ∧ mul 1000000 1000000 = some 1000000000000
    ∧ neg 42 = some (-42)
    ∧ div 1000 3 = some 333 := by
  refine ⟨?_, ?_, ?_, ?_⟩ <;> decide

/-- **⚠ AND THE BOUNDARY ITSELF IS REPRESENTABLE**, on both ends — so the check
is not off by one at exactly the place an off-by-one would hide. -/
theorem the_ends_of_the_range_fit : fits lo ∧ fits hi := by decide

/-- **And one step beyond either end does not.** -/
theorem one_step_past_either_end_does_not_fit :
    add hi 1 = none ∧ add lo (-1) = none := by
  constructor <;> decide

end Ratio.Bounded
