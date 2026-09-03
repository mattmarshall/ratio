// Household loan roll-forward: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a loan page that summed with `Number` would undo that on
// the one screen a household operator actually looks at.
//
// ⭐ ACTUALS ARE THE JOURNAL. Which liabilities have a schedule is
// `[personal.loan]` on the configuration (`Book.loans`). Beginning, principal,
// interest and ending are the trial balance in the period the chips name.
// A second amortization table would be a second answer to a question the
// journal already answers.

import { money } from "./format";
import type { Account, LoanSchedule } from "@/wire/types";

export function raw(s: string): bigint {
  return BigInt(s);
}

/** Credit-normal magnitude: a liability of 100 reads as 100, not −100. */
export function liabilityShown(n: bigint): string {
  return money((-n).toString());
}

export function debitShown(n: bigint): string {
  return money(n.toString());
}

export interface LoanRow {
  readonly dimension: string;
  readonly interestDimension: string;
  readonly displayName: string;
  /** Debit-normal stored beginning = ending − (period debit − period credit). */
  readonly beginning: bigint;
  readonly principalPaid: bigint;
  /** New draws (credits to the liability) this window. */
  readonly drawn: bigint;
  readonly interestPaid: bigint;
  readonly ending: bigint;
}

export interface LoanRollup {
  readonly rows: LoanRow[];
}

/**
 * Roll each declared loan against period activity and the as-of-end balance.
 *
 * ⛔ A LIABILITY NOT IN `Book.loans` IS NOT A ROW. The chart still has the
 * seeded mortgage/auto/student accounts — showing them at zero on a book
 * that never named a schedule is the silent-zero this figure exists to
 * refuse. An interest account with no activity is a real $0, and is shown.
 *
 * ListAccounts under `loan-*` puts period activity in debit/credit and the
 * as-of-end figure in balance, so beginning = ending − (debit − credit).
 */
export function loanRollup(
  accounts: readonly Account[],
  loans: readonly LoanSchedule[],
): LoanRollup {
  const byDim = new Map(accounts.map((a) => [a.dimension, a] as const));
  const rows: LoanRow[] = [];
  for (const loan of loans) {
    const liability = byDim.get(loan.dimension);
    const interest = byDim.get(loan.interest);
    if (!liability) continue;
    const ending = raw(liability.balance);
    const periodNet = raw(liability.debit) - raw(liability.credit);
    const beginning = ending - periodNet;
    rows.push({
      dimension: loan.dimension,
      interestDimension: loan.interest,
      displayName: liability.displayName,
      beginning,
      principalPaid: raw(liability.debit),
      drawn: raw(liability.credit),
      interestPaid: interest ? raw(interest.debit) : 0n,
      ending,
    });
  }
  return { rows };
}
