import type { Account } from "@/wire/types";

/**
 * Partner capital and the named activity accounts `chart_for(Investment)`
 * writes — not unrealized gain, and not a return.
 *
 * In / Out / Ending are the account's own credit, debit, and credit-normal
 * balance. A contribution credits partner capital; a distribution debits it;
 * an allocation and a transfer do the same with a different counter-account.
 * The books still tie because every one of those is a conserved posting.
 */

export function isCapitalAccount(displayName: string): boolean {
  return (
    displayName === "Capital contributions" ||
    displayName === "Distributions" ||
    displayName === "Allocations" ||
    displayName === "Capital transfers" ||
    displayName.startsWith("Partner capital")
  );
}

/** Credit − debit. Equity is credit-normal; `Account.balance` is debit − credit. */
export function endingCapital(a: Account): bigint {
  return BigInt(a.credit) - BigInt(a.debit);
}

export function partnersOf(accounts: Account[]): Account[] {
  return accounts.filter((a) => a.displayName.startsWith("Partner capital"));
}

export function activityOf(accounts: Account[]): Account[] {
  return accounts.filter(
    (a) =>
      a.displayName === "Capital contributions" ||
      a.displayName === "Distributions" ||
      a.displayName === "Allocations" ||
      a.displayName === "Capital transfers",
  );
}

/**
 * Book capital is the sum of partner endings plus unallocated activity.
 *
 * ⛔ A TEST THAT ONLY CHECKS THE BOOKS TIE WILL NOT SEE A WRONG PARTNER.
 * This identity is the figure: take one partner's ending out and it fails.
 */
export function bookCapital(accounts: Account[]): bigint {
  return accounts.filter((a) => isCapitalAccount(a.displayName)).reduce(
    (n, a) => n + endingCapital(a),
    0n,
  );
}

export function identityHolds(accounts: Account[]): boolean {
  const partners = partnersOf(accounts).reduce((n, a) => n + endingCapital(a), 0n);
  const activity = activityOf(accounts).reduce((n, a) => n + endingCapital(a), 0n);
  return partners + activity === bookCapital(accounts);
}
