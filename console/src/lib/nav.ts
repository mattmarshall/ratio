// Period NAV roll-forward: grouping the Investment chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a roll-forward that summed with `Number` would undo that
// on the figure that explains why NAV moved.
//
// ⭐ NAV IS ASSETS + LIABILITIES. Commitment and undrawn are equity, so they
// cancel in this filter — putting undrawn on the asset side would make an
// unfunded commitment look like cash that had arrived. HANDOFF.md.
//
// ⛔ A ZERO NAV ON AN EMPTY JOURNAL IS A FAKE. Beginning is unset when every
// account's beginning balance is 0 — there is no dated prefix before the
// window. Ending is unset when nothing dated has landed on or before the
// window end. A commitment-only prefix that nets to zero NAV is a real zero.
//
// ⭐ PLUGS CITE THE SAME ACCOUNTS `/capital` ALREADY NAMES. Contributions
// are period credits on Partner capital / Capital contributions;
// distributions are period debits on Partner capital / Distributions.
// Allocations and transfers are named when they moved. Unrealized stays
// unset until that account moved this window — the chart has the role;
// a silent 0.00 mark is the defect.

import { money } from "./format";
import { shown } from "./statement";
import type { Account } from "@/wire/types";

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

/** Raw NAV: assets + liabilities (liabilities credit-normal, negative). */
export function navOf(
  accounts: readonly Account[],
  at: (a: Account) => bigint,
): bigint {
  let nav = 0n;
  for (const a of accounts) {
    if (a.type === "ASSET" || a.type === "LIABILITY") nav += at(a);
  }
  return nav;
}

export function navShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

export function incomeShown(n: bigint | null): string {
  return n === null ? "—" : shown("income", n);
}

export function expenseShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

export function outflowShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

function isPartnerOrContrib(name: string): boolean {
  return name === "Capital contributions" || name.startsWith("Partner capital");
}

function isPartnerOrDist(name: string): boolean {
  return name === "Distributions" || name.startsWith("Partner capital");
}

function isUnrealized(name: string): boolean {
  return name === "Unrealized gain";
}

function isAllocation(name: string): boolean {
  return name === "Allocations";
}

function isTransfer(name: string): boolean {
  return name === "Capital transfers";
}

/**
 * Whole units on one account. Empty / missing is unset — not a fake
 * zero. `"0"` after a full redemption is a real zero.
 * `Ratio.Partners.no_movement_is_unset`.
 */
export function unitsOf(a: Account): bigint | null {
  const u = a.units;
  if (u === undefined || u === "") return null;
  return BigInt(u);
}

function isUnitAccount(name: string): boolean {
  return (
    name === "Capital contributions" ||
    name === "Distributions" ||
    name.startsWith("Partner capital")
  );
}

/**
 * Units in issue on the book: partner capital plus book-level
 * contribution / distribution legs. Null when no unit event has
 * posted — a PE-style contribution is not a silent 0.
 */
export function bookUnits(accounts: readonly Account[]): bigint | null {
  const posted = accounts
    .filter((a) => isUnitAccount(a.displayName))
    .map(unitsOf)
    .filter((u): u is bigint => u !== null);
  if (posted.length === 0) return null;
  return posted.reduce((n, u) => n + u, 0n);
}

export function unitsShown(n: bigint | null): string {
  return n === null ? "—" : n.toString();
}

export interface NavRollForward {
  /** Raw A+L at the start of the window, or null when the prefix cannot support it. */
  readonly beginning: bigint | null;
  /** Raw A+L as-of the window end, or null when no dated journal supports the cut. */
  readonly ending: bigint | null;
  /** ending − beginning when both cuts exist. */
  readonly delta: bigint | null;
  /**
   * Period credits on Partner capital / Capital contributions — the same
   * In column `/capital` cites. Null when those accounts are absent or
   * the ending cut is unset.
   */
  readonly contributions: bigint | null;
  /**
   * Period debits on Partner capital / Distributions — the same Out
   * column `/capital` cites. Null when those accounts are absent or
   * the ending cut is unset.
   */
  readonly distributions: bigint | null;
  /** Period income, stored raw (credit-normal, negative). Null when ending is unset. */
  readonly income: bigint | null;
  /** Period expenses, stored raw (debit-normal). Null when ending is unset. */
  readonly expense: bigint | null;
  /**
   * Period net of Unrealized gain (debit − credit). Null when the account
   * is missing or did not move this window — not a silent zero mark.
   */
  readonly unrealized: bigint | null;
  /** Period net of Allocations. Null when the account did not move. */
  readonly allocations: bigint | null;
  /** Period net of Capital transfers. Null when the account did not move. */
  readonly transfers: bigint | null;
  /**
   * ΔNAV + income + expense + all equity. Zero when the identity holds.
   * Null when any side of the identity is unset.
   */
  readonly residual: bigint | null;
}

/**
 * Roll a Loan-shaped Investment chart into a period NAV roll-forward.
 *
 * ListAccounts under `nav-*` puts period activity in debit/credit and
 * the as-of-end figure in balance, so beginning = ending − (debit − credit)
 * per account — the same identity the personal bridge and loan roll-forward
 * use. Commitment and undrawn are equity and do not enter {@link navOf}.
 */
export function navRollForward(accounts: readonly Account[]): NavRollForward {
  const beginningSet = accounts.some((a) => beginningOf(a) !== 0n);
  const endingSet = accounts.some((a) => raw(a.balance) !== 0n || moved(a));

  const beginning = beginningSet ? navOf(accounts, beginningOf) : null;
  const ending = endingSet ? navOf(accounts, (a) => raw(a.balance)) : null;
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
  const residual =
    delta !== null && income !== null && expense !== null
      ? delta + income + expense + equityRaw
      : null;

  const contribAccts = accounts.filter((a) => isPartnerOrContrib(a.displayName));
  const distAccts = accounts.filter((a) => isPartnerOrDist(a.displayName));
  const unrealAccts = accounts.filter((a) => isUnrealized(a.displayName));
  const allocAccts = accounts.filter((a) => isAllocation(a.displayName));
  const xferAccts = accounts.filter((a) => isTransfer(a.displayName));

  const contributions =
    endingSet && contribAccts.length > 0
      ? contribAccts.reduce((n, a) => n + raw(a.credit), 0n)
      : null;
  const distributions =
    endingSet && distAccts.length > 0
      ? distAccts.reduce((n, a) => n + raw(a.debit), 0n)
      : null;
  const unrealized =
    endingSet && unrealAccts.some(moved)
      ? unrealAccts.reduce((n, a) => n + periodNet(a), 0n)
      : null;
  const allocations =
    endingSet && allocAccts.some(moved)
      ? allocAccts.reduce((n, a) => n + periodNet(a), 0n)
      : null;
  const transfers =
    endingSet && xferAccts.some(moved)
      ? xferAccts.reduce((n, a) => n + periodNet(a), 0n)
      : null;

  return {
    beginning,
    ending,
    delta,
    contributions,
    distributions,
    income,
    expense,
    unrealized,
    allocations,
    transfers,
    residual,
  };
}
