// Household net-worth bridge: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a bridge that summed with `Number` would undo that on the
// figure that explains why net worth moved.
//
// ⭐ ΔNW IS INCOME MINUS EXPENSES (AND EQUITY). Principal paid, a cash→
// investments transfer, and a card payment move the sheet and leave net
// worth still. Adding any of them to the identity is the household
// spreadsheet error this page exists to refuse.
//
// ⛔ A ZERO NW ON AN EMPTY JOURNAL IS A FAKE. Beginning is unset when every
// account's beginning balance is 0 — there is no dated prefix before the
// window. Ending is unset when nothing dated has landed on or before the
// window end. Origination that nets to zero NW is a real zero: some
// account's beginning is not 0.

import { money } from "./format";
import { shown } from "./statement";
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

/** Raw net worth: assets + liabilities (liabilities credit-normal, negative). */
export function netWorthOf(
  accounts: readonly Account[],
  at: (a: Account) => bigint,
): bigint {
  let nw = 0n;
  for (const a of accounts) {
    if (a.type === "ASSET" || a.type === "LIABILITY") nw += at(a);
  }
  return nw;
}

export function nwShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

export function incomeShown(n: bigint | null): string {
  return n === null ? "—" : shown("income", n);
}

export function expenseShown(n: bigint | null): string {
  return n === null ? "—" : shown("expense", n);
}

export function debitShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

export interface NetWorthBridge {
  /** Raw A+L at the start of the window, or null when the prefix cannot support it. */
  readonly beginning: bigint | null;
  /** Raw A+L as-of the window end, or null when no dated journal supports the cut. */
  readonly ending: bigint | null;
  /** ending − beginning when both cuts exist. */
  readonly delta: bigint | null;
  /** Period income, stored raw (credit-normal, negative). Null when ending is unset. */
  readonly income: bigint | null;
  /** Period expenses, stored raw (debit-normal). Null when ending is unset. */
  readonly expense: bigint | null;
  /** Period equity net (debit − credit). Null when ending is unset. */
  readonly equity: bigint | null;
  /**
   * ΔNW − (−(income + expense + equity)). Zero when the identity holds.
   * Null when any side of the identity is unset.
   */
  readonly residual: bigint | null;
  /** Named-loan principal paid this window. Null when `Book.loans` is empty. */
  readonly principalPaid: bigint | null;
  /**
   * Investments period debit+credit plus credit-card payments.
   * Null when the chart has neither account, or the ending cut is unset.
   */
  readonly transfers: bigint | null;
  /** Always null — chart_for(Personal) has no purchase account. */
  readonly assetPurchases: bigint | null;
}

/**
 * Roll a Loan-shaped personal chart into a net-worth bridge.
 *
 * ListAccounts under `bridge-*` puts period activity in debit/credit and
 * the as-of-end figure in balance, so beginning = ending − (debit − credit)
 * per account — the same identity the loan roll-forward uses.
 */
export function netWorthBridge(
  accounts: readonly Account[],
  loans: readonly LoanSchedule[],
): NetWorthBridge {
  const beginningSet = accounts.some((a) => beginningOf(a) !== 0n);
  const endingSet = accounts.some(
    (a) => raw(a.balance) !== 0n || moved(a),
  );

  const beginning = beginningSet
    ? netWorthOf(accounts, beginningOf)
    : null;
  const ending = endingSet ? netWorthOf(accounts, (a) => raw(a.balance)) : null;
  const delta =
    beginning !== null && ending !== null ? ending - beginning : null;

  let incomeRaw = 0n;
  let expenseRaw = 0n;
  let equityRaw = 0n;
  for (const a of accounts) {
    const net = periodNet(a);
    if (a.type === "REVENUE") incomeRaw += net;
    else if (a.type === "EXPENSE") expenseRaw += net;
    else if (a.type === "EQUITY") equityRaw += net;
  }

  const income = endingSet ? incomeRaw : null;
  const expense = endingSet ? expenseRaw : null;
  const equity = endingSet ? equityRaw : null;
  const residual =
    delta !== null && income !== null && expense !== null && equity !== null
      ? delta + income + expense + equity
      : null;

  let principalPaid: bigint | null = null;
  if (loans.length > 0) {
    const byDim = new Map(accounts.map((a) => [a.dimension, a] as const));
    principalPaid = 0n;
    for (const loan of loans) {
      const liability = byDim.get(loan.dimension);
      if (liability) principalPaid += raw(liability.debit);
    }
  }

  const inv = accounts.find((a) => a.displayName === "Investments");
  const cards = accounts.find((a) => a.displayName === "Credit cards");
  const transfers =
    endingSet && (inv || cards)
      ? (inv ? raw(inv.debit) + raw(inv.credit) : 0n) +
        (cards ? raw(cards.debit) : 0n)
      : null;

  return {
    beginning,
    ending,
    delta,
    income,
    expense,
    equity,
    residual,
    principalPaid,
    transfers,
    assetPurchases: null,
  };
}
