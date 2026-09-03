// Household budget presentation: grouping the chart, not inventing one.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. `lib/format.ts` refuses to parse money
// into a double; a budget page that summed with `Number` would undo that
// on the one screen a household operator actually looks at.
//
// ⭐ ACTUALS ARE THE JOURNAL. Budget is a configuration total (`Book.budget`).
// Variance is baseline minus spent. A second accounting system would be a
// second answer to a question the trial balance already answers.

import { money } from "./format";
import type { Account, HouseholdEnvelope } from "@/wire/types";

export function raw(s: string): bigint {
  return BigInt(s);
}

/** Debit-normal magnitude: expenses show as stored. */
export function debitShown(n: bigint): string {
  return money(n.toString());
}

export function ofType(
  accounts: readonly Account[],
  type: Account["type"],
): Account[] {
  return accounts.filter((a) => a.type === type);
}

export interface EnvelopeRow {
  readonly dimension: string;
  readonly displayName: string;
  readonly actual: bigint;
  /** null when this category has no `[personal.envelope]` entry. */
  readonly planned: bigint | null;
  readonly variance: bigint | null;
}

export interface HouseholdRollup {
  readonly spent: bigint;
  /** Configuration total, or null when unset. */
  readonly baseline: bigint | null;
  /** baseline − spent when a baseline is set. */
  readonly variance: bigint | null;
  readonly envelopes: EnvelopeRow[];
}

/**
 * Roll the personal chart's expenses up against a configuration total.
 *
 * Planned envelopes are the sparse `[personal.envelope]` map. An expense
 * account with no envelope is unset for that category — not a fake zero.
 * The household total is `Book.budget` alone; envelopes are not summed
 * into a baseline nobody declared.
 */
export function householdRollup(
  accounts: readonly Account[],
  budget: string,
  envelopes: readonly HouseholdEnvelope[],
): HouseholdRollup {
  const expenses = ofType(accounts, "EXPENSE");
  const spent = expenses.reduce((s, a) => s + raw(a.balance), 0n);
  const plannedByDim = new Map(
    envelopes.map((e) => [e.dimension, raw(e.budget)] as const),
  );
  const rows: EnvelopeRow[] = expenses.map((a) => {
    const planned = plannedByDim.has(a.dimension)
      ? plannedByDim.get(a.dimension)!
      : null;
    return {
      dimension: a.dimension,
      displayName: a.displayName,
      actual: raw(a.balance),
      planned,
      variance: planned === null ? null : planned - raw(a.balance),
    };
  });
  const baseline = budget.trim() === "" ? null : raw(budget);
  const variance = baseline === null ? null : baseline - spent;
  return { spent, baseline, variance, envelopes: rows };
}
