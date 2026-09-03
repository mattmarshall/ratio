// Household statement presentation: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a balance sheet that summed with `Number` would undo that
// on the one screen a household actually looks at.

import { money } from "./format";
import type { Account, AccountType } from "@/wire/types";

export type SheetSection = "asset" | "liability" | "equity" | "income" | "expense";

export function sectionOf(t: AccountType): SheetSection | null {
  switch (t) {
    case "ASSET":
      return "asset";
    case "LIABILITY":
      return "liability";
    case "EQUITY":
      return "equity";
    case "REVENUE":
      return "income";
    case "EXPENSE":
      return "expense";
    default:
      return null;
  }
}

/** Debits minus credits, as BigInt. */
export function rawBalance(a: Account): bigint {
  return BigInt(a.balance);
}

function add(a: bigint, b: bigint): bigint {
  return a + b;
}

/** Credit-normal magnitude: a liability of −4000 raw is 40.00 shown. */
export function shown(section: SheetSection, raw: bigint): string {
  if (section === "asset" || section === "expense") return money(raw.toString());
  return money((-raw).toString());
}

export interface SheetTotals {
  readonly assets: bigint;
  readonly liabilities: bigint;
  readonly equity: bigint;
  readonly income: bigint;
  readonly expenses: bigint;
  /** Income − expenses, credit-normal like equity. */
  readonly surplus: bigint;
}

export function sheetTotals(accounts: readonly Account[]): SheetTotals {
  let assets = 0n;
  let liabilities = 0n;
  let equity = 0n;
  let income = 0n;
  let expenses = 0n;
  for (const a of accounts) {
    const s = sectionOf(a.type);
    const b = rawBalance(a);
    if (s === "asset") assets = add(assets, b);
    else if (s === "liability") liabilities = add(liabilities, b);
    else if (s === "equity") equity = add(equity, b);
    else if (s === "income") income = add(income, b);
    else if (s === "expense") expenses = add(expenses, b);
  }
  // Surplus as a credit-normal residual: income (credit, negative raw) minus
  // expenses (debit, positive raw), so a profitable period is negative raw
  // the way equity is.
  const surplus = income + expenses;
  return { assets, liabilities, equity, income, expenses, surplus };
}

/**
 * A = L + E + surplus, in raw debit-minus-credit.
 *
 * Assets raw + liabilities raw + equity raw + income raw + expense raw = 0
 * when the books tie, so assets raw = −(L+E+I+X) = L_shown + E_shown + surplus_shown
 * once credit-normal accounts are flipped. Compared in raw: assets === −(L+E+surplus).
 */
export function sheetFoots(t: SheetTotals): boolean {
  return t.assets + t.liabilities + t.equity + t.surplus === 0n;
}

export function ofType(
  accounts: readonly Account[],
  section: SheetSection,
): Account[] {
  return accounts.filter((a) => sectionOf(a.type) === section);
}
