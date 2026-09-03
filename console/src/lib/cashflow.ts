// Household cash-flow statement: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a cash-flow walk that summed with `Number` would undo that
// on the figure that says where cash went.
//
// ⭐ CASH IS THE CONSERVED SPINE. Beginning cash plus classified movement
// equals ending cash, because conservation already says the period nets
// sum to zero: cash_net = −Σ(other nets). The operating / investing /
// financing split is a partition of those other nets, not a second ledger.
//
// ⭐ THE SPLIT THE CHART CAN SUPPORT. IAS 7 activity classes, reconstructed
// from each non-cash account's period net (cash from an account = −(debit −
// credit)):
//
//   operating  — REVENUE, EXPENSE, Credit cards (working capital)
//   investing  — Investments (the same account the net-worth bridge names
//                as a transfer)
//   financing  — named loans (`Book.loans`: principal and draws) and
//                Opening equity (household in/out)
//
// Credit cards sit in operating, not financing: they are the household's
// one current liability, not a named loan. A card charge is Dr expense /
// Cr cards and does not move cash; omitting the card plug would make a
// charge look like a cash outflow. Investments land in investing — that
// is the half of the bridge's "Transfers" that is an asset purchase/sale.
//
// ⛔ A ZERO CASH ON AN EMPTY JOURNAL IS A FAKE. Beginning is unset when
// every account's beginning balance is 0 — there is no dated prefix
// before the window. Ending is unset when nothing dated has landed on
// or before the window end. Origination that leaves cash at a real
// number is a figure; spending down to zero is a real zero.
//
// ⛔ AN UNCLASSIFIED MOVE IS A NAMED LINE, NOT A RESIDUAL BUCKET.
// Mortgage / Auto / Student activity on a book that never named
// `[personal.loan]` is not silently absorbed into financing. Asset
// purchases stay unset: chart_for(Personal) has no purchase account.

import { money } from "./format";
import type { Account, LoanSchedule } from "@/wire/types";

export function raw(s: string): bigint {
  return BigInt(s);
}

/** Beginning stored balance = ending − (period debit − period credit). */
export function beginningOf(a: Account): bigint {
  return raw(a.balance) - (raw(a.debit) - raw(a.credit));
}

function periodNet(a: Account): bigint {
  return raw(a.debit) - raw(a.credit);
}

function moved(a: Account): boolean {
  return raw(a.debit) !== 0n || raw(a.credit) !== 0n;
}

/** Cash from a non-cash account = −(period debit − period credit). */
export function cashFrom(a: Account): bigint {
  return -periodNet(a);
}

export function isCashAccount(a: Account): boolean {
  return a.type === "ASSET" && a.displayName.startsWith("Cash");
}

export function cashShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

export interface UnclassifiedLine {
  readonly dimension: string;
  readonly displayName: string;
  readonly cash: bigint;
}

export interface CashFlowStatement {
  /** Cash as-of the day before the window, or null when no dated prefix exists. */
  readonly beginning: bigint | null;
  /** Cash as-of the window end, or null when no dated journal supports the cut. */
  readonly ending: bigint | null;
  /** ending − beginning when both cuts exist. */
  readonly delta: bigint | null;
  /**
   * Income + expenses + credit-card working capital, as cash.
   * Null when the ending cut is unset.
   */
  readonly operating: bigint | null;
  /** −(period income net). Null when ending is unset. */
  readonly income: bigint | null;
  /** −(period expense net). Null when ending is unset. */
  readonly expense: bigint | null;
  /**
   * −(Credit cards period net). Null when that account is absent or
   * the ending cut is unset.
   */
  readonly creditCards: bigint | null;
  /**
   * −(Investments period net). Null when that account is absent or
   * the ending cut is unset. The same account the bridge names as a transfer.
   */
  readonly transfers: bigint | null;
  /** Always null — chart_for(Personal) has no purchase account. */
  readonly assetPurchases: bigint | null;
  /** Investing total. Null when ending is unset. */
  readonly investing: bigint | null;
  /** Named-loan principal paid (liability debit). Null when `Book.loans` is empty. */
  readonly principalPaid: bigint | null;
  /** Named-loan draws (liability credit). Null when `Book.loans` is empty. */
  readonly drawn: bigint | null;
  /**
   * −(Opening equity period net). Null when that account is absent or
   * the ending cut is unset.
   */
  readonly equity: bigint | null;
  /** Financing total. Null when ending is unset. */
  readonly financing: bigint | null;
  /** Accounts the split cannot name honestly. Empty when everything classified. */
  readonly unclassified: readonly UnclassifiedLine[];
  /**
   * ending − (beginning + operating + investing + financing + unclassified).
   * Zero when the identity holds. Null when either cash cut is unset.
   */
  readonly residual: bigint | null;
}

/**
 * Roll a Loan-shaped personal chart into a period cash-flow statement.
 *
 * ListAccounts under `cashflow-*` puts period activity in debit/credit and
 * the as-of-end figure in balance, so beginning = ending − (debit − credit)
 * per account — the same identity the bridge and loan roll-forward use.
 */
export function cashFlowStatement(
  accounts: readonly Account[],
  loans: readonly LoanSchedule[],
): CashFlowStatement {
  const beginningSet = accounts.some((a) => beginningOf(a) !== 0n);
  const endingSet = accounts.some((a) => raw(a.balance) !== 0n || moved(a));

  const cashAccounts = accounts.filter(isCashAccount);
  const cashAt = (at: (a: Account) => bigint): bigint =>
    cashAccounts.reduce((n, a) => n + at(a), 0n);

  const beginning = beginningSet ? cashAt(beginningOf) : null;
  const ending = endingSet ? cashAt((a) => raw(a.balance)) : null;
  const delta =
    beginning !== null && ending !== null ? ending - beginning : null;

  const loanDims = new Set(loans.map((l) => l.dimension));
  const inv = accounts.find((a) => a.displayName === "Investments");
  const cards = accounts.find((a) => a.displayName === "Credit cards");
  const opening = accounts.find((a) => a.displayName === "Opening equity");

  let incomeRaw = 0n;
  let expenseRaw = 0n;
  const unclassified: UnclassifiedLine[] = [];
  for (const a of accounts) {
    if (isCashAccount(a)) continue;
    if (a.type === "REVENUE") {
      incomeRaw += periodNet(a);
      continue;
    }
    if (a.type === "EXPENSE") {
      expenseRaw += periodNet(a);
      continue;
    }
    if (a.displayName === "Credit cards") continue;
    if (a.displayName === "Investments") continue;
    if (a.displayName === "Opening equity") continue;
    if (loanDims.has(a.dimension)) continue;
    // ⛔ A PRIOR BALANCE IS NOT THIS WINDOW'S CASH. Only period activity
    // is a cash-flow line; leftover sheet balances belong on `/sheet`.
    if (moved(a)) {
      unclassified.push({
        dimension: a.dimension,
        displayName: a.displayName,
        cash: cashFrom(a),
      });
    }
  }

  const income = endingSet ? -incomeRaw : null;
  const expense = endingSet ? -expenseRaw : null;
  const creditCards = endingSet && cards ? cashFrom(cards) : null;
  const transfers = endingSet && inv ? cashFrom(inv) : null;
  const equity = endingSet && opening ? cashFrom(opening) : null;

  let principalPaid: bigint | null = null;
  let drawn: bigint | null = null;
  if (loans.length > 0) {
    const byDim = new Map(accounts.map((a) => [a.dimension, a] as const));
    principalPaid = 0n;
    drawn = 0n;
    for (const loan of loans) {
      const liability = byDim.get(loan.dimension);
      if (!liability) continue;
      principalPaid += raw(liability.debit);
      drawn += raw(liability.credit);
    }
  }

  const operating =
    endingSet
      ? (income ?? 0n) + (expense ?? 0n) + (creditCards ?? 0n)
      : null;
  const investing = endingSet ? (transfers ?? 0n) : null;
  const loanCash =
    principalPaid !== null && drawn !== null
      ? drawn - principalPaid
      : 0n;
  const financing =
    endingSet ? loanCash + (equity ?? 0n) : null;

  const unclassifiedCash = unclassified.reduce((n, l) => n + l.cash, 0n);
  const residual =
    beginning !== null &&
    ending !== null &&
    operating !== null &&
    investing !== null &&
    financing !== null
      ? ending - (beginning + operating + investing + financing + unclassifiedCash)
      : null;

  return {
    beginning,
    ending,
    delta,
    operating,
    income,
    expense,
    creditCards,
    transfers,
    assetPurchases: null,
    investing,
    principalPaid,
    drawn,
    equity,
    financing,
    unclassified,
    residual,
  };
}
