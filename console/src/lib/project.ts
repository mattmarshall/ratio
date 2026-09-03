// Project figure presentation: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a budget or billing page that summed with `Number` would
// undo that on the screens a project operator actually looks at.
//
// ⭐ ACTUALS ARE THE JOURNAL. Book-level budget is a configuration total
// (`Book.budget`) — the original contract. Billing is billed vs earned,
// retainage, and cost by phase from `projectProgress`. Approved change
// orders are a conserved equity pair on the same chart; they adjust the
// revised contract without rewriting that key. Remaining to bill is
// revised − billed; collections vs billed is cash against AR (billed −
// outstanding receivable − retainage held). Those stay two URLs;
// change orders, remaining-to-bill, and collections compose onto both
// rather than a third chrome list.

import { money } from "./format";
import type { Account } from "@/wire/types";

export function raw(s: string): bigint {
  return BigInt(s);
}

/** Empty is unset — not a fake zero. `"0"` is a set figure of nothing. */
export function figure(minor: string): string {
  return minor === "" ? "—" : money(minor);
}

/** Debit-normal magnitude: assets and expenses show as stored. */
export function debitShown(n: bigint): string;
/** Debit-normal wire string; empty is unset. */
export function debitShown(minor: string): string;
export function debitShown(n: bigint | string): string {
  if (typeof n === "string") return figure(n);
  return money(n.toString());
}

/** Credit-normal magnitude: a liability of −4000 raw is 40.00 shown. */
export function creditShown(n: bigint): string;
/** Credit-normal wire string already stored as a credit; empty is unset. */
export function creditShown(minor: string): string;
export function creditShown(n: bigint | string): string {
  if (typeof n === "string") return figure(n);
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

/** Credit − debit. Approved change orders are credit-normal equity. */
export function creditNormal(a: Account): bigint {
  return raw(a.credit) - raw(a.debit);
}

export function isPosted(a: Account): boolean {
  return a.postingCount !== "0" && a.postingCount !== "";
}

export function isApprovedChangeOrder(displayName: string): boolean {
  return (
    displayName === "Approved change orders" ||
    displayName.startsWith("Approved change orders — ")
  );
}

export function isChangeOrderAuthorization(displayName: string): boolean {
  return (
    displayName === "Change-order authorization" ||
    displayName.startsWith("Change-order authorization — ")
  );
}

export function isChangeOrderAccount(displayName: string): boolean {
  return isApprovedChangeOrder(displayName) || isChangeOrderAuthorization(displayName);
}

/** Funding received — not the change-order memorandum pair. */
export function isFundingAccount(a: Account): boolean {
  return a.type === "EQUITY" && !isChangeOrderAccount(a.displayName);
}

/**
 * Work-package key for an approved-change-order account.
 *
 * Unpartitioned `"Approved change orders"` maps to `""` (the `Project costs`
 * expense). `"Approved change orders — Site and mobilization"` maps to that
 * phase's display name, so cost-by-phase and COs share one grain.
 */
export function changeOrderPhase(displayName: string): string | null {
  if (displayName === "Approved change orders") return "";
  const prefix = "Approved change orders — ";
  if (displayName.startsWith(prefix)) return displayName.slice(prefix.length);
  return null;
}

export function phaseKeyForExpense(displayName: string): string {
  return displayName === "Project costs" ? "" : displayName;
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
  /** Configuration total, or null when unset. The original contract. */
  readonly baseline: bigint | null;
  /**
   * Credit-normal approved change orders, or null when none have posted.
   * ⛔ NOT A FAKE ZERO. `postingCount === "0"` on every CO account is unset.
   */
  readonly approved: bigint | null;
  /** baseline + (approved ?? 0) when a baseline is set; otherwise null. */
  readonly revised: bigint | null;
  /** revised − committed when a revised contract is set. */
  readonly variance: bigint | null;
}

/**
 * Roll the project chart up.
 *
 * WIP is the asset named "work in progress", or every non-cash asset if
 * the chart has been relabelled. Costs / funding / revenue / payables are
 * the expense, equity, income and liability accounts. Cash is named cash,
 * else the first asset that is not WIP.
 *
 * ⚠ BILLING FIGURES ARE NOT THIS ROLL-UP. Progress billings, retainage and
 * unbilled receivables live on `/billing` via `projectProgress`. Remaining
 * to bill and collections compose there from this roll-up's revised
 * contract plus those billing cuts. Folding them into committed spend
 * here would make `/budget` answer a different question.
 *
 * ⚠ CHANGE ORDERS ARE NOT FUNDING AND NOT COST. They are excluded from
 * `funding` the way commitments are excluded from book capital. Variance
 * is against the revised contract, not a mutated baseline.
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
  const funding = sumBal(accounts.filter(isFundingAccount));
  const revenue = sumBal(ofType(accounts, "REVENUE"));
  const payables = sumBal(ofType(accounts, "LIABILITY"));
  const cash = sumBal(cashAccounts.length > 0 ? cashAccounts : assets.filter((a) => !isWip(a)));
  const incurred = costs + wip;
  const committed = incurred + -payables;
  const baseline = budget.trim() === "" ? null : raw(budget);
  const approved = approvedChangeOrders(accounts);
  const revised = revisedContract(baseline, approved);
  const variance = revised === null ? null : revised - committed;
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
    approved,
    revised,
    variance,
  };
}

/** WIP.debit = currently capitalized + recognized. */
export function wipFoots(r: ProjectRollup): boolean {
  return r.wipDebit === r.wip + r.wipCredit;
}

/**
 * Credit-normal approved change orders across work packages.
 *
 * `null` when no approved-change-order account has a posting — not a
 * contract of zero. A posted net of nothing (approve then deduct the
 * same amount) is a real zero.
 */
export function approvedChangeOrders(accounts: readonly Account[]): bigint | null {
  const lines = accounts.filter((a) => isApprovedChangeOrder(a.displayName));
  if (lines.length === 0) return null;
  if (!lines.some(isPosted)) return null;
  return lines.filter(isPosted).reduce((n, a) => n + creditNormal(a), 0n);
}

/**
 * Window activity on approved-change-order accounts.
 *
 * For a Loan-shaped fold, debit/credit are the window. `null` when nothing
 * posted in-window — not a fake zero of changes this month.
 */
export function changeOrdersInWindow(accounts: readonly Account[]): bigint | null {
  const lines = accounts.filter((a) => isApprovedChangeOrder(a.displayName));
  if (!lines.some(isPosted)) return null;
  return lines.filter(isPosted).reduce((n, a) => n + creditNormal(a), 0n);
}

/** Revised = original + approved when the original is set; otherwise unset. */
export function revisedContract(
  baseline: bigint | null,
  approved: bigint | null,
): bigint | null {
  if (baseline === null) return null;
  return baseline + (approved ?? 0n);
}

/**
 * Remaining to bill: revised − billed.
 *
 * ⛔ UNSET STAYS UNSET. An unknown baseline cannot produce a remainder.
 * An unbilled job is not billed-zero — treating billed as 0 would print
 * the whole contract as remaining and look like a measured leftover.
 * A posted billed of nothing against a set revised is a real zero.
 */
export function remainingToBill(
  revised: bigint | null,
  billed: string,
): bigint | null {
  if (revised === null || billed === "") return null;
  return revised - raw(billed);
}

/** Debit-normal Accounts receivable, or null when that account has not posted. */
export function accountsReceivable(accounts: readonly Account[]): bigint | null {
  const a = accounts.find((x) => x.displayName === "Accounts receivable");
  if (!a || !isPosted(a)) return null;
  return raw(a.balance);
}

/**
 * Cash collected against billed AR: billed − AR − retainage held.
 *
 * ⛔ NOT A FAKE ZERO. Unset billed cannot support collections-vs-billed.
 * Unset AR cannot either — progress_bill always posts the receivable, so
 * a billed figure with no AR posting is a chart the identity cannot read.
 * Retainage that has never been held is 0 for the subtraction (no hold
 * is not an unknown hold). Billed and uncollected is a real zero.
 */
export function collectedAgainstBilled(
  billed: string,
  ar: bigint | null,
  retainageReceivable: string,
): bigint | null {
  if (billed === "" || ar === null) return null;
  const held = retainageReceivable === "" ? 0n : raw(retainageReceivable);
  return raw(billed) - ar - held;
}

/**
 * Uncollected billed: AR + retainage receivable.
 *
 * Same refusal as `collectedAgainstBilled` — the two partition billed
 * when both are set. `collected + outstanding === billed`.
 */
export function outstandingAgainstBilled(
  billed: string,
  ar: bigint | null,
  retainageReceivable: string,
): bigint | null {
  if (billed === "" || ar === null) return null;
  const held = retainageReceivable === "" ? 0n : raw(retainageReceivable);
  return ar + held;
}

/**
 * Approved change orders for one work-package expense account.
 *
 * `Project costs` pairs with the unpartitioned CO account. A phase with
 * no posting stays unset, not a silent zero against that phase's budget.
 */
export function phaseApproved(
  accounts: readonly Account[],
  expenseDisplayName: string,
): bigint | null {
  const want =
    expenseDisplayName === "Project costs"
      ? "Approved change orders"
      : `Approved change orders — ${expenseDisplayName}`;
  const a = accounts.find((x) => x.displayName === want);
  if (!a || !isPosted(a)) return null;
  return creditNormal(a);
}
