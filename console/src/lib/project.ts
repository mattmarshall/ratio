// Project figure presentation: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a budget page that summed with `Number` would undo that
// on the one screen a project operator actually looks at.
//
// ⭐ ACTUALS ARE THE JOURNAL. Budget is a configuration total (`Book.budget`).
// Variance is baseline minus committed spend. A second accounting system
// would be a second answer to a question the trial balance already answers.

import { money } from "./format";
import type { Account } from "@/wire/types";

export function raw(s: string): bigint {
  return BigInt(s);
}

/** Debit-normal magnitude: assets and expenses show as stored. */
export function debitShown(n: bigint): string {
  return money(n.toString());
}

/** Credit-normal magnitude: a liability of −4000 raw is 40.00 shown. */
export function creditShown(n: bigint): string {
  return money((-n).toString());
}

export function isCash(a: Account): boolean {
  return /cash/i.test(a.displayName);
}

export function isWip(a: Account): boolean {
  return /work in progress/i.test(a.displayName);
}

export function ofType(
  accounts: readonly Account[],
  type: Account["type"],
): Account[] {
  return accounts.filter((a) => a.type === type);
}

export interface ProjectRollup {
  readonly cash: bigint;
  readonly wip: bigint;
  readonly wipDebit: bigint;
  readonly wipCredit: bigint;
  readonly costs: bigint;
  readonly funding: bigint;
  readonly revenue: bigint;
  readonly payables: bigint;
  /** costs.balance + wip.balance — incurred, not double-counted on recognize. */
  readonly incurred: bigint;
  /** incurred + unpaid payables. */
  readonly committed: bigint;
  /** Configuration total, or null when unset. */
  readonly baseline: bigint | null;
  /** baseline − committed when a baseline is set. */
  readonly variance: bigint | null;
}

/**
 * Roll the project chart up.
 *
 * WIP is the asset named "work in progress", or every non-cash asset if
 * the chart has been relabelled. Costs / funding / revenue / payables are
 * the expense, equity, income and liability accounts. Cash is named cash,
 * else the first asset that is not WIP.
 */
export function projectRollup(
  accounts: readonly Account[],
  budget: string,
): ProjectRollup {
  const assets = ofType(accounts, "ASSET");
  const wipAccounts = assets.filter(isWip);
  const capitalized = wipAccounts.length > 0 ? wipAccounts : assets.filter((a) => !isCash(a));
  const cashAccounts = assets.filter(isCash);

  const sumBal = (rows: readonly Account[]) =>
    rows.reduce((s, a) => s + raw(a.balance), 0n);
  const sumDebit = (rows: readonly Account[]) =>
    rows.reduce((s, a) => s + raw(a.debit), 0n);
  const sumCredit = (rows: readonly Account[]) =>
    rows.reduce((s, a) => s + raw(a.credit), 0n);

  const wip = sumBal(capitalized);
  const wipDebit = sumDebit(capitalized);
  const wipCredit = sumCredit(capitalized);
  const costs = sumBal(ofType(accounts, "EXPENSE"));
  const funding = sumBal(ofType(accounts, "EQUITY"));
  const revenue = sumBal(ofType(accounts, "REVENUE"));
  const payables = sumBal(ofType(accounts, "LIABILITY"));
  const cash = sumBal(cashAccounts.length > 0 ? cashAccounts : assets.filter((a) => !isWip(a)));
  const incurred = costs + wip;
  const committed = incurred + -payables;
  const baseline = budget.trim() === "" ? null : raw(budget);
  const variance = baseline === null ? null : baseline - committed;
  return {
    cash,
    wip,
    wipDebit,
    wipCredit,
    costs,
    funding,
    revenue,
    payables,
    incurred,
    committed,
    baseline,
    variance,
  };
}

/** WIP.debit = currently capitalized + recognized. */
export function wipFoots(r: ProjectRollup): boolean {
  return r.wipDebit === r.wip + r.wipCredit;
}
