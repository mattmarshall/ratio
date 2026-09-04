import { money } from "./format";
import { beginningOf, unitsOf } from "./nav";
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

/** One named weight. The total is the sum, not 100 and not the count. */
export interface PartnerShare {
  readonly partner: string;
  readonly weight: bigint;
}

export type PartnerCut = readonly PartnerShare[];

export type AllocationKind = "income" | "expense" | "unrealized";

/** A standing special: this partner's weight of this kind. */
export interface SpecialAllocation {
  readonly partner: string;
  readonly kind: AllocationKind;
  readonly weight: bigint;
}

/**
 * One partner's capital account statement for a window.
 *
 * Beginning / ending are credit-normal stocks. Contributions and
 * distributions are this partner's period credits and debits — the same
 * In / Out the activity table already cites — not an equal slice of the
 * book. Allocated income, expense, and unrealized stay unset without a
 * named partner cut — the Investment chart has no partner dim on those
 * accounts, and `allocate_*_lp` closes an exact amount into partner
 * capital (already in In / Out). A silent 0.00 share or a 50/50 split
 * of book NAV is the defect. A written `[[partner_cut]]` fills the
 * plugs; a figure that will not divide leaves them unset.
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
  /** Ending units on this partner. Null until a unit event posts. */
  readonly units: bigint | null;
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
 * Apply a named cut to a book figure.
 *
 * ⛔ EMPTY IS UNSET, NOT 1/N. `Ratio.Partners.no_cut_is_unset`.
 * A figure that will not divide returns null for every partner —
 * a partial fill would look exact for the ones that happened to
 * land. `Ratio.Partners.a_slice_is_exactly_pro_rata`.
 */
export function applyCut(
  figure: bigint | null,
  cut: PartnerCut | null | undefined,
): Map<string, bigint> | null {
  if (figure === null) return null;
  if (!cut || cut.length === 0) return null;
  const seen = new Set<string>();
  let total = 0n;
  for (const s of cut) {
    if (s.weight <= 0n) return null;
    if (!s.partner || seen.has(s.partner)) return null;
    seen.add(s.partner);
    total += s.weight;
  }
  if (total <= 0n) return null;
  const out = new Map<string, bigint>();
  for (const s of cut) {
    const prod = figure * s.weight;
    if (prod % total !== 0n) return null;
    out.set(s.partner, prod / total);
  }
  return out;
}

/**
 * The cut that applies to a kind: standing specials if any were
 * named, otherwise the default. `Ratio.Partners.cutFor`.
 */
export function cutForKind(
  kind: AllocationKind,
  cut: PartnerCut | null | undefined,
  specials: readonly SpecialAllocation[] | null | undefined,
): PartnerCut | null {
  const named = (specials ?? []).filter((s) => s.kind === kind);
  if (named.length === 0) return cut && cut.length > 0 ? cut : null;
  return named.map((s) => ({ partner: s.partner, weight: s.weight }));
}

/**
 * Partner-cut of a book income / expense / unrealized figure.
 *
 * ⛔ NEVER EQUAL-SPLIT, NEVER A FAKE ZERO. An empty cut is unset —
 * dividing by the partner count invents a share nobody posted.
 * Returning `0n` would be the other lie: a measured zero share of a
 * figure that did move. `Ratio.Partners.no_cut_is_unset`.
 */
export function allocatedPlug(
  bookFigure: bigint | null,
  cut: PartnerCut | null | undefined,
  partner: string,
): bigint | null {
  const shares = applyCut(bookFigure, cut);
  if (!shares) return null;
  return shares.get(partner) ?? null;
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
  partnerCut?: PartnerCut | null,
  specials?: readonly SpecialAllocation[] | null,
): PartnerCapitalAccount[] {
  const partners = partnersOf(accounts as Account[]);
  const bookHasPrefix = cut === "period" && prefixSet(accounts);
  const bookHasEnding =
    cut === "inception" ? partners.some(isPosted) : endingSet(accounts);

  // Book-level plugs exist so a caller can see we refused them. They
  // do not enter any partner line without a named cut.
  const bookIncome = periodType(accounts, "REVENUE");
  const bookExpense = periodType(accounts, "EXPENSE");
  const bookUnreal = accounts
    .filter((a) => a.displayName === "Unrealized gain")
    .reduce((s, a) => s + (raw(a.debit) - raw(a.credit)), 0n);
  const incomePlug = bookHasEnding ? creditNormal(bookIncome) : null;
  const expensePlug = bookHasEnding ? bookExpense : null;
  const unrealPlug = bookHasEnding &&
    accounts.some((a) => a.displayName === "Unrealized gain" && moved(a))
    ? creditNormal(bookUnreal)
    : null;
  const incomeCut = cutForKind("income", partnerCut, specials);
  const expenseCut = cutForKind("expense", partnerCut, specials);
  const unrealCut = cutForKind("unrealized", partnerCut, specials);

  return partners.map((a) => {
    const posted = isPosted(a);
    const grain = partnerGrain(a.displayName);
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
      grain,
      accountName: a.name,
      beginning,
      contributions,
      distributions,
      allocatedIncome: allocatedPlug(incomePlug, incomeCut, grain),
      allocatedExpense: allocatedPlug(expensePlug, expenseCut, grain),
      unrealized: allocatedPlug(unrealPlug, unrealCut, grain),
      ending,
      units: unitsOf(a),
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
