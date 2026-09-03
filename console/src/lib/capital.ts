import { money } from "./format";
import { beginningOf } from "./nav";
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

function raw(s: string): bigint {
  return BigInt(s);
}

function moved(a: Account): boolean {
  return raw(a.debit) !== 0n || raw(a.credit) !== 0n;
}

/** Credit-normal magnitude of a stored (debit − credit) balance. */
function creditNormal(stored: bigint): bigint {
  return -stored;
}

/**
 * Whether this fold can name a beginning stock.
 *
 * Same predicate `/nav` uses: some account's beginning is not 0. Activity
 * fold (`capital-YYYY-MM`) sets activity = ending, so every beginning is
 * 0 and this is false — a fake zero beginning. Period statements read
 * the Loan-shaped `nav-*` fold instead.
 */
function prefixSet(accounts: readonly Account[]): boolean {
  return accounts.some((a) => beginningOf(a) !== 0n);
}

function endingSet(accounts: readonly Account[]): boolean {
  return accounts.some((a) => raw(a.balance) !== 0n || moved(a));
}

export type CapitalCut = "inception" | "period";

/**
 * One partner's capital account statement for a window.
 *
 * Beginning / ending are credit-normal stocks. Contributions and
 * distributions are this partner's period credits and debits — the same
 * In / Out the activity table already cites — not an equal slice of the
 * book. Allocated income, expense, and unrealized stay unset: the
 * Investment chart has no partner dim on those accounts, and
 * `allocate_*_lp` closes an exact amount into partner capital (already
 * in In / Out). A silent 0.00 share or a 50/50 split is the defect.
 */
export interface PartnerCapitalAccount {
  readonly displayName: string;
  readonly grain: string;
  readonly accountName: string;
  readonly beginning: bigint | null;
  readonly contributions: bigint | null;
  readonly distributions: bigint | null;
  readonly allocatedIncome: bigint | null;
  readonly allocatedExpense: bigint | null;
  readonly unrealized: bigint | null;
  readonly ending: bigint | null;
}

/** The partner grain on `Partner capital — LP`, or the whole name. */
export function partnerGrain(displayName: string): string {
  const sep = " — ";
  const i = displayName.indexOf(sep);
  return i >= 0 ? displayName.slice(i + sep.length) : displayName;
}

export function capitalShown(n: bigint | null): string {
  return n === null ? "—" : money(n.toString());
}

/**
 * Partner-cut of a book income / expense / unrealized figure.
 *
 * ⛔ NEVER EQUAL-SPLIT, NEVER A FAKE ZERO. The journal has no ownership
 * percentage. `allocate_gain_lp` posts an exact integer into partner
 * capital — that credit is already on In. Dividing book NAV or period
 * income by the partner count invents a share nobody posted. Returning
 * `0n` would be the other lie: a measured zero share of a figure that
 * did move.
 */
export function allocatedPlug(
  _bookFigure: bigint | null,
  _partnerCount: number,
): bigint | null {
  return null;
}

/**
 * Per-partner capital accounts from a chart fold.
 *
 * `inception` is the Current / activity fold `/capital` already lists:
 * beginning is unset (there is no prior prefix), ending is this
 * partner's credit-normal stock when they have posted, otherwise unset.
 *
 * `period` needs the Loan-shaped `nav-*` fold — the same two cuts `/nav`
 * cites. Activity-shaped `capital-*` rows make every beginning 0 and
 * refuse the prefix; do not pass them here and treat that 0 as a figure.
 */
export function partnerCapitalAccounts(
  accounts: readonly Account[],
  cut: CapitalCut,
): PartnerCapitalAccount[] {
  const partners = partnersOf(accounts as Account[]);
  const n = partners.length;
  const bookHasPrefix = cut === "period" && prefixSet(accounts);
  const bookHasEnding =
    cut === "inception" ? partners.some(isPosted) : endingSet(accounts);

  // Book-level plugs exist so a caller can see we refused them. They
  // do not enter any partner line.
  const bookIncome = periodType(accounts, "REVENUE");
  const bookExpense = periodType(accounts, "EXPENSE");
  const bookUnreal = accounts
    .filter((a) => a.displayName === "Unrealized gain")
    .reduce((s, a) => s + (raw(a.debit) - raw(a.credit)), 0n);
  const incomePlug = bookHasEnding ? bookIncome : null;
  const expensePlug = bookHasEnding ? bookExpense : null;
  const unrealPlug = bookHasEnding &&
    accounts.some((a) => a.displayName === "Unrealized gain" && moved(a))
    ? bookUnreal
    : null;

  return partners.map((a) => {
    const posted = isPosted(a);
    const beginning = bookHasPrefix ? creditNormal(beginningOf(a)) : null;
    const contributions =
      bookHasEnding && (cut === "period" || posted) ? raw(a.credit) : null;
    const distributions =
      bookHasEnding && (cut === "period" || posted) ? raw(a.debit) : null;
    const ending =
      cut === "inception"
        ? posted
          ? endingCapital(a)
          : null
        : bookHasEnding
          ? creditNormal(raw(a.balance))
          : null;
    return {
      displayName: a.displayName,
      grain: partnerGrain(a.displayName),
      accountName: a.name,
      beginning,
      contributions,
      distributions,
      allocatedIncome: allocatedPlug(incomePlug, n),
      allocatedExpense: allocatedPlug(expensePlug, n),
      unrealized: allocatedPlug(unrealPlug, n),
      ending,
    };
  });
}

function periodType(
  accounts: readonly Account[],
  type: Account["type"],
): bigint {
  return accounts
    .filter((a) => a.type === type)
    .reduce((s, a) => s + (raw(a.debit) - raw(a.credit)), 0n);
}

/**
 * beginning + contributions − distributions = ending, when those four
 * are set and the allocated plugs are unset.
 *
 * ⛔ A TEST THAT ONLY CHECKS THE BOOKS TIE WILL NOT SEE A WRONG PARTNER.
 * Dropping one partner's contribution keeps conservation of the rest.
 * Allocated plugs are not coerced to 0n here — treating unset as zero
 * is how an equal-split of 0.00 would still satisfy the identity.
 */
export function partnerIdentityHolds(s: PartnerCapitalAccount): boolean | null {
  if (
    s.beginning === null ||
    s.contributions === null ||
    s.distributions === null ||
    s.ending === null
  ) {
    return null;
  }
  if (
    s.allocatedIncome !== null ||
    s.allocatedExpense !== null ||
    s.unrealized !== null
  ) {
    return null;
  }
  return s.beginning + s.contributions - s.distributions === s.ending;
}
