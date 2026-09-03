// Period-close roll-forward: beginning retained earnings → surplus → ending.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a roll-forward that summed with `Number` would undo that
// on the figure that carries taxable income into equity.
//
// ⛔ UNSET STAYS UNSET. A missing beginning prefix, a missing close
// posting, or a missing equity destination is `null`, not a measured
// zero. `Ratio.Close.missing_beginning_is_unset`.
// `Ratio.Close.missing_surplus_is_unset`.
//
// A preview of an open period may show unclosed surplus, but it is
// provisional — the period is not closed.

import { beginningOf, raw } from "./nav";
import { shown } from "./statement";
import type { Account, CalendarDate, PeriodClose } from "@/wire/types";

/** `YYYY-MM-DD` for comparison and the resource id. A CalendarDate is a
 *  day, not an instant — do not put it through `Date`. */
export function closedYmd(d: CalendarDate | null | undefined): string {
  if (!d) return "";
  const y = String(d.year).padStart(4, "0");
  const m = String(d.month).padStart(2, "0");
  const day = String(d.day).padStart(2, "0");
  return `${y}-${m}-${day}`;
}

function periodNet(a: Account): bigint {
  return raw(a.debit) - raw(a.credit);
}

function moved(a: Account): boolean {
  return raw(a.debit) !== 0n || raw(a.credit) !== 0n;
}

export function equityShown(n: bigint | null): string {
  return n === null ? "—" : shown("equity", n);
}

/** Last calendar day of a `YYYY` or `YYYY-MM` window. */
export function windowEnd(spec: string): string {
  if (/^\d{4}$/.test(spec)) return `${spec}-12-31`;
  const m = /^(\d{4})-(\d{2})$/.exec(spec);
  if (!m) return spec;
  const y = Number(m[1]);
  const mo = Number(m[2]);
  const last = new Date(Date.UTC(y, mo, 0)).getUTCDate();
  return `${spec}-${String(last).padStart(2, "0")}`;
}

/** The newest close that covers this window's last day, if any. */
export function coveringClose(
  closes: readonly PeriodClose[],
  spec: string,
): PeriodClose | null {
  const end = windowEnd(spec);
  return closes.find((c) => closedYmd(c.closedDate) >= end) ?? null;
}

function destOf(
  accounts: readonly Account[],
  destDim: string | null,
): Account | undefined {
  if (destDim) {
    const byDim = accounts.find((a) => a.dimension === destDim);
    if (byDim) return byDim;
  }
  return accounts.find((a) => a.displayName === "Retained earnings");
}

export interface CloseRollForward {
  /** Raw RE at the start of the window, or null when the prefix cannot support it. */
  readonly beginning: bigint | null;
  /**
   * Period surplus (income + expenses, raw). From the close record when
   * the window is closed; from I/E activity when it is not. Null when
   * neither cut exists.
   */
  readonly surplus: bigint | null;
  /**
   * Dest activity that is not the surplus. Null when nothing else moved
   * — not a silent zero adjustment.
   */
  readonly adjustments: bigint | null;
  /** Raw RE as-of the window end, or null when no dated journal supports the cut. */
  readonly ending: bigint | null;
  /** beginning + surplus + adjustments − ending when every cut is present. */
  readonly residual: bigint | null;
  readonly closed: boolean;
  readonly destination: Account | undefined;
}

/**
 * Roll a Loan-shaped chart into a retained-earnings roll-forward.
 *
 * ListAccounts under `close-*` puts period activity in debit/credit and
 * the as-of-end figure in balance. Beginning = ending − (debit − credit)
 * per account — the same identity the personal bridge already uses.
 *
 * ⭐ BEGINNING IS UNSET WHEN NO DATED PREFIX PRECEDES THE WINDOW. A first
 * period whose other accounts also start at 0 has no beginning cut to
 * cite — not a measured zero RE. After a prior close, dest beginning is
 * the prior ending and some account's beginning is nonzero.
 */
export function closeRollForward(
  accounts: readonly Account[],
  close: PeriodClose | null,
): CloseRollForward {
  const beginningSet = accounts.some((a) => beginningOf(a) !== 0n);
  const endingSet = accounts.some((a) => raw(a.balance) !== 0n || moved(a));
  const dest = destOf(accounts, close?.equityDestination || null);

  const beginning = dest && beginningSet ? beginningOf(dest) : null;

  let incomeRaw = 0n;
  let expenseRaw = 0n;
  for (const a of accounts) {
    const net = periodNet(a);
    if (a.type === "REVENUE") incomeRaw += net;
    else if (a.type === "EXPENSE") expenseRaw += net;
  }
  const activitySurplus = endingSet ? incomeRaw + expenseRaw : null;

  // ⛔ A RECORDED CLOSE WITH AN EMPTY SURPLUS IS UNSET, NOT ACTIVITY.
  // Falling through to I/E would turn "nothing rolled" into a measured
  // zero the moment any other account moved this window.
  const recorded =
    close && close.surplus !== "" ? BigInt(close.surplus) : null;
  const surplus = close ? recorded : activitySurplus;

  const destActivity = dest && endingSet ? periodNet(dest) : null;
  const adjustments =
    destActivity !== null && recorded !== null && destActivity !== recorded
      ? destActivity - recorded
      : null;

  const ending = dest && endingSet ? raw(dest.balance) : null;

  const residual =
    beginning !== null && surplus !== null && ending !== null
      ? beginning + surplus + (adjustments ?? 0n) - ending
      : null;

  return {
    beginning,
    surplus,
    adjustments,
    ending,
    residual,
    closed: close !== null,
    destination: dest,
  };
}
