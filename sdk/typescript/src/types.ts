// The kernel API's wire types.
//
// Hand-written to mirror `proto/ratio/v1/ledger.proto` and `chart.proto` under
// proto3's canonical JSON mapping: lowerCamelCase fields, enums as their
// names, and EVERY int64 as a string — because a JavaScript number loses
// integers past 2^53 and an amount is exact or it is wrong.
//
// ⛔ Checked, not trusted. `//proto:sdk_mirrors_test` reads both protos and
// asserts every message field appears here with the right name — the same
// discipline as the console's `wire/types.ts`, and for the same reason: a
// hand-written mirror that nothing compares is a mirror that drifts.

/** Money and counts: an int64 as a decimal string. See {@link int64}. */
export type Int64 = string;

/** An int64 from whatever you have. A number must be a safe integer. */
export function int64(v: bigint | number | string): Int64 {
  if (typeof v === "bigint") return v.toString();
  if (typeof v === "number") {
    if (!Number.isSafeInteger(v)) throw new RangeError(`${v} is not a safe integer; pass a bigint`);
    return v.toString();
  }
  if (!/^-?\d+$/.test(v.trim())) throw new RangeError(`${JSON.stringify(v)} is not a 64-bit integer`);
  return BigInt(v.trim()).toString();
}

/** The exact value of an {@link Int64}. */
export function toBigInt(v: Int64 | undefined): bigint {
  return v === undefined || v === "" ? 0n : BigInt(v);
}

export type AccountType =
  | "ACCOUNT_TYPE_UNSPECIFIED"
  | "ACCOUNT_TYPE_ASSET"
  | "ACCOUNT_TYPE_LIABILITY"
  | "ACCOUNT_TYPE_EQUITY"
  | "ACCOUNT_TYPE_INCOME"
  | "ACCOUNT_TYPE_EXPENSE";

export type Side = "SIDE_UNSPECIFIED" | "SIDE_DEBIT" | "SIDE_CREDIT";

/**
 * One component of a transaction's integer vector.
 *
 * `dim` is the conserved dimension (the account); `amount` is exact minor units.
 * `currencyCode` names the conservation law the amount is under — two currencies are
 * two laws, not one law over a sum; absent is the book's untyped group.
 * `instrument` partitions further; `quantity` is measured, not conserved.
 */
export interface Posting {
  dim: Int64;
  amount: Int64;
  currencyCode?: string;
  instrument?: string;
  quantity?: Int64;
}

/** A balanced transaction, as the journal holds it. */
export interface Transaction {
  name?: string;
  postings: Posting[];
  /** On create: pin to the configuration in force, or be refused. On read: the digest the entry was posted under. */
  controlPlaneHash?: string;
}

export interface ListTransactionsResponse {
  transactions: Transaction[];
  nextPageToken?: string;
}

/** A named conserved dimension. `normalSide` is derived by the server from `accountType`, never sent. */
export interface Account {
  name?: string;
  dim: Int64;
  displayName: string;
  accountType: AccountType;
  normalSide?: Side;
}

export interface ListAccountsResponse {
  accounts: Account[];
  nextPageToken?: string;
}

/** One (account, currency) row of a trial balance. */
export interface AccountBalance {
  account: string;
  debits: Int64;
  credits: Int64;
  currencyCode?: string;
}

/** Totals per side and their difference — zero for any book the kernel admitted. */
export interface TrialBalance {
  name: string;
  debits: Int64;
  credits: Int64;
  difference: Int64;
  accountBalances: AccountBalance[];
  controlPlaneHash?: string;
}

/** A posting with no currency, instrument or quantity: the book's untyped group. */
export function posting(dim: bigint | number, amount: bigint | number, currency?: string): Posting {
  const p: Posting = { dim: int64(dim), amount: int64(amount) };
  if (currency) p.currencyCode = currency;
  return p;
}
