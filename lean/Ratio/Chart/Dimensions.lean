set_option warningAsError true
/-! `Ratio.Chart.Dimensions` — what a chart of accounts is, on a kernel that
conserves vectors.

`Ratio.Core` is more general than the ledger built on it. `Conserves v` is
`v = Vec.zero` — zero in EVERY dimension — and `Dim` is described there as "the
conserved-quantity basis". The running kernel collapsed that to one number: sum
every posting's amount, require zero.

⛔ AND THAT COLLAPSE LOSES MULTI-CURRENCY. Measured:

    postings   [USD +100.00, EUR −100.00]
    sum        0            ⇒ "balanced"
    USD nets   +100.00
    EUR nets   −100.00

Neither currency balances. Nothing was exchanged, no rate was recorded, and the
entry passed the door of a ledger whose one claim is that it cannot go out of
balance. `Ratio.Core.Conserves` would have refused it; the specialization to a
single total is what admitted it.

⭐ SO THE ARCHITECTURE IS A CLASSIFICATION, AND IT IS THE WHOLE DESIGN. Every
label a posting carries is exactly one of three things, and confusing them is
where the failures above come from:

  CONSERVED    the quantity that must net to zero. CURRENCY is one. Two of them
               are two independent conservation laws, not one law over a sum.
  PARTITIONING ACCOUNT, instrument, share class, counterparty. These say WHERE
               the conserved quantity sits. They do not net to zero — an account
               that netted zero on every entry would be an account nothing ever
               moved through — and a breakdown by one of them ROLLS UP to the
               level above.
  MEASURED     QUANTITY. Units are not conserved by a fund's book at all: buying
               a hundred shares creates a hundred shares in it, because the
               counterparty is outside. Claiming units conserve would make every
               purchase unbalanced.

A chart of accounts is the partitioning dimension people argue about. It is not
the conserved one, and the kernel has never said it was.

Lean core + `omega`, no Mathlib. -/

namespace Ratio.Chart

/-- Everything a posting is labelled with. -/
structure Key where
  /-- ⛔ PARTITIONS. Where the value sits. -/
  account : Nat
  /-- ⛔ CONSERVES. Its own conservation law, independent of every other. -/
  currency : String
  /-- ⛔ PARTITIONS, one level below the account. -/
  instrument : Option String
deriving DecidableEq, Repr

structure Posting where
  key : Key
  amount : Int
  /-- ⛔ MEASURED, NOT CONSERVED. -/
  quantity : Int
deriving DecidableEq, Repr

/-- The net of one currency across a transaction. -/
def netIn (c : String) : List Posting → Int
  | [] => 0
  | p :: rest => (if p.key.currency = c then p.amount else 0) + netIn c rest

/-- The net of one account, whatever the currency. -/
def netOf (a : Nat) : List Posting → Int
  | [] => 0
  | p :: rest => (if p.key.account = a then p.amount else 0) + netOf a rest

/-- The net of one account IN one currency. -/
def netOfIn (a : Nat) (c : String) : List Posting → Int
  | [] => 0
  | p :: rest =>
    (if p.key.account = a ∧ p.key.currency = c then p.amount else 0) + netOfIn a c rest

/-- Everything, added up regardless of what it is denominated in.

⚠ THE NUMBER THE RUNNING KERNEL CHECKS, and the one the counterexample below is
about. It is meaningful only when a book holds one currency. -/
def flatTotal : List Posting → Int
  | [] => 0
  | p :: rest => p.amount + flatTotal rest

/-- A transaction conserves value iff EVERY currency nets to zero.

`Ratio.Core.Conserves` with `Dim` instantiated at the currency. -/
def Conserves (ps : List Posting) : Prop := ∀ c : String, netIn c ps = 0

/- ── The hole ──────────────────────────────────────────────────────────── -/

private def usd (a : Nat) (amt : Int) : Posting :=
  ⟨⟨a, "USD", none⟩, amt, 0⟩
private def eur (a : Nat) (amt : Int) : Posting :=
  ⟨⟨a, "EUR", none⟩, amt, 0⟩

/-- **⛔ A FLAT TOTAL OF ZERO IS NOT CONSERVATION.**

A hundred dollars debited and a hundred euros credited. The sum is zero and the
running kernel calls it balanced. Nothing was exchanged, no rate was recorded,
and each currency is out by a hundred.

This is the specialization failing, not the kernel: `Ratio.Core.Conserves` asks
for zero in every dimension and would have refused it. -/
theorem a_flat_total_hides_a_currency_mismatch :
    flatTotal [usd 1 10000, eur 2 (-10000)] = 0
    ∧ netIn "USD" [usd 1 10000, eur 2 (-10000)] ≠ 0 := by
  constructor
  · decide
  · decide

/-- **And conserving every currency is strictly stronger**: the same shape, done
properly, needs four legs — the two sides of a real exchange. -/
theorem a_real_exchange_conserves_both :
    netIn "USD" [usd 1 10000, usd 2 (-10000), eur 3 9000, eur 4 (-9000)] = 0
    ∧ netIn "EUR" [usd 1 10000, usd 2 (-10000), eur 3 9000, eur 4 (-9000)] = 0 := by
  constructor <;> decide

/- ── Where the rate goes ───────────────────────────────────────────────── -/

/-- A hundred dollars sold for ninety euros, through account 5.

Both sides of the exchange land in ONE account, which is what makes the rate a
recorded fact rather than an inference from two unrelated legs. -/
private def exchange : List Posting :=
  [usd 2 (-10000), usd 5 10000, eur 5 (-9000), eur 3 9000]

/-- **⭐ AN EXCHANGE CONSERVES EVERY CURRENCY, AND ITS CONVERSION ACCOUNT DOES
NOT NET TO ZERO.**

Both halves matter and they say different things.

The first is the law: neither currency moved in aggregate, so nothing was
created by the exchange.

The second is the POINT. Account 5 is left holding `+10000 USD` and `−9000 EUR`,
which is not an error and must not be netted away — it is the RATE, recorded as
two figures the way it was actually struck. `netOf 5` is `1000`, a number in no
currency at all, and it is meaningless: the flat total across currencies is the
quantity `a_flat_total_hides_a_currency_mismatch` already showed says nothing.

⚠ THIS IS WHERE AN FX GAIN WILL LIVE, and it is deliberately not computed here.
Separating a foreign-currency gain into its security component and its currency
component needs a decision about whether a lot carries a rate at acquisition,
which is a modelling question rather than a missing theorem. -/
theorem an_exchange_conserves_and_leaves_the_rate_behind :
    netIn "USD" exchange = 0
    ∧ netIn "EUR" exchange = 0
    ∧ netOfIn 5 "USD" exchange = 10000
    ∧ netOfIn 5 "EUR" exchange = -9000 := by
  refine ⟨?_, ?_, ?_, ?_⟩ <;> decide

/-- **⛔ AND DROPPING THE CONVERSION ACCOUNT BREAKS THE LAW, NOT THE BOOKKEEPING.**

The tempting shortcut is two legs: cash out in one currency, cash in in the
other. It has a flat total near zero and it conserves NEITHER currency — the
dollars left and no dollar entry says where, the euros arrived from nowhere.

⚠ Note this is a DIFFERENT failure from the one above. There the flat total was
exactly zero and hid the mismatch; here it is not even zero, and a kernel
checking only the sum would refuse this one while admitting that one. Which is
the argument for checking per currency rather than picking a better sum. -/
theorem a_two_leg_exchange_conserves_neither_currency :
    netIn "USD" [usd 2 (-10000), eur 3 9000] ≠ 0
    ∧ netIn "EUR" [usd 2 (-10000), eur 3 9000] ≠ 0 := by
  constructor <;> decide

/- ── Accounts partition; they do not conserve ─────────────────────────── -/

/-- **⛔ AN ACCOUNT DOES NOT NET TO ZERO, AND MUST NOT.**

Cash goes down by a hundred and investments go up by a hundred. The transaction
conserves; neither account does. An account that netted zero on every entry
would be one nothing ever moved through.

Stated because "balanced" is loose talk that means two different things — the
transaction is balanced, the accounts are not — and a chart of accounts is the
partitioning dimension, never the conserved one. -/
theorem an_account_does_not_conserve :
    netIn "USD" [usd 1 10000, usd 2 (-10000)] = 0
    ∧ netOf 1 [usd 1 10000, usd 2 (-10000)] ≠ 0 := by
  constructor <;> decide

/-- **An instrument breakdown rolls up to its account.**

The value attributed to each instrument, plus what is attributed to none, is the
account. So adding a partitioning dimension cannot change what the level above
says — which is what makes it safe to add one.

`Ratio.Ingest.positions_roll_up` proves the general form over the data plane;
this is the same statement in the chart's own vocabulary. -/
private def held (a : Nat) (i : String) (amt : Int) (q : Int) : Posting :=
  ⟨⟨a, "USD", some i⟩, amt, q⟩

def netOfInstrument (a : Nat) (i : Option String) : List Posting → Int
  | [] => 0
  | p :: rest =>
    (if p.key.account = a ∧ p.key.instrument = i then p.amount else 0)
      + netOfInstrument a i rest

theorem an_instrument_breakdown_rolls_up :
    netOfInstrument 1 (some "VTI") [held 1 "VTI" 6000 10, held 1 "VOO" 4000 5, usd 1 500]
      + netOfInstrument 1 (some "VOO") [held 1 "VTI" 6000 10, held 1 "VOO" 4000 5, usd 1 500]
      + netOfInstrument 1 none [held 1 "VTI" 6000 10, held 1 "VOO" 4000 5, usd 1 500]
      = netOf 1 [held 1 "VTI" 6000 10, held 1 "VOO" 4000 5, usd 1 500] := by
  decide

/- ── Quantity is measured ─────────────────────────────────────────────── -/

def netQuantity : List Posting → Int
  | [] => 0
  | p :: rest => p.quantity + netQuantity rest

/-- **⛔ UNITS ARE NOT CONSERVED BY A FUND'S BOOK.**

Buying a hundred shares for ten thousand: value conserves — cash out, investments
in — and a hundred shares appear where there were none, because the counterparty
who gave them up is outside the book.

⚠ SO A "CONSERVATION CHECK" OVER QUANTITY WOULD REFUSE EVERY PURCHASE. Units are
measured and partitioned, never conserved, and the distinction is why
`PostingRecord.quantity` says so out loud rather than being another amount. -/
theorem units_are_not_conserved_by_a_purchase :
    netIn "USD" [held 1 "VTI" 10000 100, usd 2 (-10000)] = 0
    ∧ netQuantity [held 1 "VTI" 10000 100, usd 2 (-10000)] ≠ 0 := by
  constructor <;> decide

/-- **And a total over units is not a quantity anybody holds.**

Selling ten of one instrument and buying ten of another nets zero units while
moving two positions. Units are only meaningful PER INSTRUMENT, which is why the
quantity lives beside the instrument label rather than standing on its own. -/
theorem a_total_over_units_is_not_a_holding :
    netQuantity [held 1 "VTI" (-6000) (-10), held 1 "VOO" 6000 10] = 0
    ∧ netOfInstrument 1 (some "VTI") [held 1 "VTI" (-6000) (-10), held 1 "VOO" 6000 10] ≠ 0 := by
  constructor <;> decide

end Ratio.Chart
