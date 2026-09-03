import type { Account } from "@/wire/types";

/**
 * Partner capital and the named activity accounts `chart_for(Investment)`
 * writes — not unrealized gain, and not a return.
 *
 * In / Out / Ending are the account's own credit, debit, and credit-normal
 * balance. A contribution credits partner capital; a distribution debits it;
 * an allocation and a transfer do the same with a different counter-account.
 * The books still tie because every one of those is a conserved posting.
 *
 * ⭐ COMMITMENT / UNDRAWN ARE A SEPARATE FIGURE. They conserve with partner
 * capital (a call is one four-leg entry) but they are not money that arrived.
 * Folding them into ending capital would make a commitment look like a
 * contribution. Unset stays unset: `postingCount === "0"` is not a callable
 * zero, and a fully-drawn commitment (`postingCount > 0`, remaining 0) is.
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

export function isCommitmentAccount(displayName: string): boolean {
  return displayName.startsWith("Commitments");
}

export function isUndrawnAccount(displayName: string): boolean {
  return displayName.startsWith("Undrawn commitments");
}

/** Credit − debit. Equity is credit-normal; `Account.balance` is debit − credit. */
export function endingCapital(a: Account): bigint {
  return BigInt(a.credit) - BigInt(a.debit);
}

/** Debit − credit. Undrawn is debit-normal equity. */
export function remainingUndrawn(a: Account): bigint {
  return BigInt(a.debit) - BigInt(a.credit);
}

export function isPosted(a: Account): boolean {
  return a.postingCount !== "0" && a.postingCount !== "";
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

export function commitmentsOf(accounts: Account[]): Account[] {
  return accounts.filter((a) => isCommitmentAccount(a.displayName));
}

export function undrawnOf(accounts: Account[]): Account[] {
  return accounts.filter((a) => isUndrawnAccount(a.displayName));
}

/**
 * Book capital is the sum of partner endings plus unallocated activity.
 *
 * ⛔ A TEST THAT ONLY CHECKS THE BOOKS TIE WILL NOT SEE A WRONG PARTNER.
 * This identity is the figure: take one partner's ending out and it fails.
 * Commitment / undrawn are not in this sum.
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

/**
 * Remaining undrawn, partner by partner.
 *
 * `null` when no commitment or undrawn posting has been recorded for that
 * partner — not a callable zero. A fully-drawn line (`posted` and remaining
 * 0) is a real zero.
 */
export function undrawnFigure(accounts: Account[]): bigint | null {
  const lines = undrawnOf(accounts);
  if (lines.length === 0) return null;
  if (!lines.some(isPosted)) return null;
  return lines.filter(isPosted).reduce((n, a) => n + remainingUndrawn(a), 0n);
}

/**
 * Remaining commitment (credit-normal) on posted commitment accounts.
 *
 * When commit/call post as a pair this equals {@link undrawnFigure}. A
 * book that only contributed has no posted commitment, so both are null.
 */
export function remainingCommitment(accounts: Account[]): bigint | null {
  const lines = commitmentsOf(accounts);
  if (lines.length === 0) return null;
  if (!lines.some(isPosted)) return null;
  return lines.filter(isPosted).reduce((n, a) => n + endingCapital(a), 0n);
}

/** Posted remaining commitment equals posted remaining undrawn. */
export function commitmentIdentityHolds(accounts: Account[]): boolean {
  return remainingCommitment(accounts) === undrawnFigure(accounts);
}
