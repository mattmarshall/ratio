import Ratio.Partners.Cut

set_option warningAsError true
/-! `Ratio.Partners.Notice` — a capital-call / distribution notice.

The journal already has partner capital activity (`call_*` /
`distribute_*`). This file is the citeable document a walk-through
points at: a kind, a total, the named partner cut, and the amounts.

It is **not a waterfall**. There is no preferred return, no catch-up,
and no carry. A notice is either:

  1. **`issue`** — pro-rata of a named total under the cut
     (`Ratio.Partners.allocate`). A figure that will not divide is
     refused rather than rounded. No preferred-return math.
  2. **`fromPosted`** — the amounts the journal already posted, with
     the cut cited as the agreement in force. The cut does **not**
     rewrite those amounts.

⛔ APPLYING `issue` TO A PARTNER-SCOPED CALL INVENTS THE OTHER
PARTNERS. 80/20 of an LP call of 250_000 is a GP share nobody
posted. That is the waterfall-shaped defect: the books still tie,
and the number is somebody else's. `fromPosted` is what GetBook
walks. The cut is on the document so a reader can see the
agreement; it is not a second engine over the journal.

Unset stays unset. No cut, an empty cut, a zero amount, or an
empty posted list is not a silent 1/N notice.

Lean core + `omega`, no Mathlib. ⛔ `warningAsError` is load-bearing. -/

namespace Ratio.Partners

/-- Call or distribution. Not preferred, not catch-up, not carry. -/
inductive NoticeKind where
  | call
  | distribution
  deriving DecidableEq, Repr

/-- One partner amount the journal posted. Not a weight. -/
structure Posted where
  partner : Partner
  amount : Int
  deriving DecidableEq, Repr

/-- A citeable notice: kind, total, the named cut, the amounts.

The identity **is** these four. Rewriting amounts while pretending
the document is the same is the silent defect — the digest would
still cite, and the number would be somebody else's. -/
structure Notice where
  kind : NoticeKind
  amount : Int
  cut : List Share
  amounts : List (Partner × Int)
  deriving DecidableEq, Repr

def postedSum : List Posted → Int
  | [] => 0
  | p :: ps => p.amount + postedSum ps

def postedAsAlloc : List Posted → List (Partner × Int)
  | [] => []
  | p :: ps => (p.partner, p.amount) :: postedAsAlloc ps

def nonzeroPosted : List Posted → Bool
  | [] => true
  | p :: ps => decide (p.amount ≠ 0) && nonzeroPosted ps

/-- A posted list is nonempty and every amount is nonzero.
A zero row is a no-op, not a partner amount. -/
def wellFormedPosted (ps : List Posted) : Bool :=
  !ps.isEmpty && nonzeroPosted ps

theorem posted_sum_is_the_amounts :
    ∀ ps, allocatedSum (postedAsAlloc ps) = postedSum ps
  | [] => rfl
  | p :: ps => by
    simp [postedAsAlloc, postedSum, allocatedSum]
    exact posted_sum_is_the_amounts ps

/-- Issue a notice from a named total under a cut.

Pro-rata only. `allocate` is the figure. A zero amount is not a
notice. No cut stays unset. There is no preferred, catch-up, or
carry constructor beside this. -/
def issue (kind : NoticeKind) (amount : Int) (cut : Option (List Share)) :
    Option Notice :=
  if amount = 0 then
    none
  else
    match allocate amount cut with
    | none => none
    | some alloc =>
      match cut with
      | none => none
      | some shares => some ⟨kind, amount, shares, alloc⟩

/-- Cite a notice from journal amounts and a named cut.

⛔ THE AMOUNTS ARE WHAT THE JOURNAL POSTED. `issue` on a
partner-scoped call invents the other names. The cut is cited; it
does not restate the books. -/
def fromPosted (kind : NoticeKind) (cut : Option (List Share))
    (posted : List Posted) : Option Notice :=
  match cut with
  | none => none
  | some shares =>
    if wellFormed shares && wellFormedPosted posted then
      let amount := postedSum posted
      if amount = 0 then
        none
      else
        some ⟨kind, amount, shares, postedAsAlloc posted⟩
    else
      none

/-- Rewriting amounts keeps kind / total / cut and changes the
figure. That is a different document, not a restatement wearing
the same identity. -/
def rewriteAmounts (n : Notice) (amounts : List (Partner × Int)) : Notice :=
  { n with amounts }

/- ── Unset stays unset ──────────────────────────────────────────────── -/

theorem no_cut_issue_is_unset (k : NoticeKind) (amount : Int) :
    issue k amount none = none := by
  unfold issue
  split
  · rfl
  · simp [allocate]

theorem a_zero_amount_is_not_an_issued_notice
    (k : NoticeKind) (cut : Option (List Share)) :
    issue k 0 cut = none := by
  simp [issue]

theorem no_cut_from_posted_is_unset (k : NoticeKind) (ps : List Posted) :
    fromPosted k none ps = none := rfl

theorem empty_posted_is_unset (k : NoticeKind) (shares : List Share) :
    fromPosted k (some shares) [] = none := by
  simp [fromPosted, wellFormedPosted]

theorem a_zero_posted_row_is_not_a_notice (k : NoticeKind) (shares : List Share) :
    fromPosted k (some shares) [⟨0, 0⟩] = none := by
  simp [fromPosted, wellFormedPosted, nonzeroPosted]

/- ── Issued amounts are the cut; posted amounts are the journal ───── -/

theorem issued_amounts_are_the_cut
    (k : NoticeKind) (amount : Int) (shares : List Share) (n : Notice)
    (h : issue k amount (some shares) = some n) :
    allocate amount (some shares) = some n.amounts ∧ n.amount = amount := by
  unfold issue at h
  split at h
  · simp at h
  · cases halloc : allocate amount (some shares) with
    | none => simp [halloc] at h
    | some alloc =>
      simp [halloc] at h
      subst h
      simp

theorem posted_amounts_are_the_journal
    (k : NoticeKind) (shares : List Share) (ps : List Posted) (n : Notice)
    (h : fromPosted k (some shares) ps = some n) :
    n.amounts = postedAsAlloc ps := by
  simp [fromPosted] at h
  obtain ⟨⟨_, _⟩, _, hn⟩ := h
  subst hn
  rfl

/-- **When a notice is issued, the shares are the total.** Same
conservation as `allocated_shares_sum_to_the_figure`. A remainder
here would be a misstatement of who was called. -/
theorem issued_shares_sum_to_the_amount
    (k : NoticeKind) (amount : Int) (shares : List Share) (n : Notice)
    (h : issue k amount (some shares) = some n) :
    allocatedSum n.amounts = n.amount := by
  have ⟨halloc, hamt⟩ := issued_amounts_are_the_cut k amount shares n h
  have hsum := allocated_shares_sum_to_the_figure amount shares n.amounts halloc
  exact hamt ▸ hsum

theorem rewriting_amounts_is_a_different_notice
    (n : Notice) (amounts : List (Partner × Int))
    (h : amounts ≠ n.amounts) :
    rewriteAmounts n amounts ≠ n := by
  intro heq
  apply h
  simpa [rewriteAmounts] using congrArg Notice.amounts heq

/- ── The load-bearing holdings ──────────────────────────────────────── -/

/-- **A silent 1/N is not a notice.** Nobody named the cut. -/
example : issue .call 250000 none = none := rfl

example : fromPosted .call none [⟨0, 250000⟩] = none := rfl

/-- 80/20 of a named 250_000 total is 200_000 / 50_000. `issue` is
pro-rata of a total, not a waterfall. -/
example :
    issue .call 250000 (some [⟨0, 80⟩, ⟨1, 20⟩])
      = some ⟨.call, 250000, [⟨0, 80⟩, ⟨1, 20⟩], [(0, 200000), (1, 50000)]⟩ := by
  decide

/-- **A partner-scoped LP call is not 80/20 of itself.** GetBook
cites the posted 250_000 on LP. Inventing GP 50_000 is the defect. -/
example :
    fromPosted .call (some [⟨0, 80⟩, ⟨1, 20⟩]) [⟨0, 250000⟩]
      = some ⟨.call, 250000, [⟨0, 80⟩, ⟨1, 20⟩], [(0, 250000)]⟩ := by
  decide

/-- The two constructions differ on the same inputs. Using `issue`
where `fromPosted` belongs invents a partner. -/
example :
    issue .call 250000 (some [⟨0, 80⟩, ⟨1, 20⟩])
      ≠ fromPosted .call (some [⟨0, 80⟩, ⟨1, 20⟩]) [⟨0, 250000⟩] := by
  decide

/-- A figure that will not divide is not an issued notice. -/
example : issue .distribution 101 (some [⟨0, 80⟩, ⟨1, 20⟩]) = none := by
  decide

/-- A zero total is not a notice. -/
example : issue .call 0 (some [⟨0, 80⟩, ⟨1, 20⟩]) = none := by
  decide

end Ratio.Partners
