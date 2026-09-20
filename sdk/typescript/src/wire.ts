// The protobuf wire format for the ratio.v1 messages, by hand.
//
// Seven RPCs over five messages need two wire types — varints (every int64,
// int32 and enum) and length-delimited fields (every string and message) — so
// the gRPC transport needs `@grpc/grpc-js` and nothing else: no generated
// stubs, no proto files to ship, no build step between `npm install` and a
// call. Field numbers are the contract's (`proto/ratio/v1/*.proto`). Unknown
// fields are skipped, so a newer server does not break an older client.

import type {
  Account,
  AccountBalance,
  AccountType,
  ListAccountsResponse,
  ListTransactionsResponse,
  Posting,
  Side,
  Transaction,
  TrialBalance,
} from "./types.js";

const MASK64 = (1n << 64n) - 1n;
const ACCOUNT_TYPES: AccountType[] = [
  "ACCOUNT_TYPE_UNSPECIFIED", "ACCOUNT_TYPE_ASSET", "ACCOUNT_TYPE_LIABILITY",
  "ACCOUNT_TYPE_EQUITY", "ACCOUNT_TYPE_INCOME", "ACCOUNT_TYPE_EXPENSE",
];
const SIDES: Side[] = ["SIDE_UNSPECIFIED", "SIDE_DEBIT", "SIDE_CREDIT"];

// ── encoding ────────────────────────────────────────────────────────────────

function varint(n: bigint): number[] {
  const out: number[] = [];
  for (;;) {
    const b = Number(n & 0x7fn);
    n >>= 7n;
    if (n) out.push(b | 0x80);
    else {
      out.push(b);
      return out;
    }
  }
}

/** int64 / int32 / enum: two's complement 64-bit as a varint. Zero is proto3's default and not written. */
function intField(num: number, v: bigint | number | undefined): number[] {
  const n = v === undefined ? 0n : BigInt(v);
  if (n === 0n) return [];
  return [...varint(BigInt(num << 3)), ...varint(n & MASK64)];
}

/** A proto3 `optional` int64: written when present, zero included. */
function presentIntField(num: number, v: string | undefined): number[] {
  if (v === undefined) return [];
  return [...varint(BigInt(num << 3)), ...varint(BigInt(v) & MASK64)];
}

function lenField(num: number, payload: number[]): number[] {
  return [...varint(BigInt((num << 3) | 2)), ...varint(BigInt(payload.length)), ...payload];
}

function strField(num: number, s: string | undefined): number[] {
  if (!s) return [];
  return lenField(num, [...new TextEncoder().encode(s)]);
}

export function encodePosting(p: Posting): number[] {
  return [
    ...intField(1, BigInt(p.dim)),
    ...intField(2, BigInt(p.amount)),
    ...strField(3, p.currencyCode),
    ...strField(4, p.instrument),
    ...presentIntField(5, p.quantity),
  ];
}

export function encodeTransaction(t: Transaction): number[] {
  return [
    ...strField(1, t.name),
    ...t.postings.flatMap((p) => lenField(2, encodePosting(p))),
    ...strField(3, t.controlPlaneHash),
  ];
}

/** GetTransactionRequest, GetAccountRequest, GetTrialBalanceRequest: `name = 1`. */
export function encodeGetRequest(name: string): Uint8Array {
  return Uint8Array.from(strField(1, name));
}

/** ListTransactionsRequest / ListAccountsRequest: parent 1, page_size 2, page_token 3. */
export function encodeListRequest(parent: string, pageSize: number, pageToken: string): Uint8Array {
  return Uint8Array.from([...strField(1, parent), ...intField(2, pageSize), ...strField(3, pageToken)]);
}

export function encodeCreateTransactionRequest(parent: string, t: Transaction, transactionId: string): Uint8Array {
  return Uint8Array.from([...strField(1, parent), ...lenField(2, encodeTransaction(t)), ...strField(3, transactionId)]);
}

export function encodeAccount(a: Account): number[] {
  return [
    ...strField(1, a.name),
    ...intField(2, BigInt(a.dim)),
    ...strField(3, a.displayName),
    ...intField(4, Math.max(0, ACCOUNT_TYPES.indexOf(a.accountType))),
    ...intField(5, Math.max(0, SIDES.indexOf(a.normalSide ?? "SIDE_UNSPECIFIED"))),
  ];
}

export function encodeCreateAccountRequest(parent: string, a: Account, accountId: string): Uint8Array {
  return Uint8Array.from([...strField(1, parent), ...lenField(2, encodeAccount(a)), ...strField(3, accountId)]);
}

// ── decoding ────────────────────────────────────────────────────────────────

type Field = { num: number; wt: number; int: bigint; bytes: Uint8Array };

function readVarint(b: Uint8Array, i: number): [bigint, number] {
  let shift = 0n;
  let n = 0n;
  for (;;) {
    if (i >= b.length) throw new Error("truncated varint");
    const c = b[i++];
    n |= BigInt(c & 0x7f) << shift;
    if (!(c & 0x80)) return [n, i];
    shift += 7n;
    if (shift > 70n) throw new Error("varint too long");
  }
}

/** Every field: `int` for varints, `bytes` for length-delimited; fixed-width fields are skipped. */
export function* fields(b: Uint8Array): Generator<Field> {
  let i = 0;
  while (i < b.length) {
    const [tag, j] = readVarint(b, i);
    i = j;
    const num = Number(tag >> 3n);
    const wt = Number(tag & 7n);
    if (wt === 0) {
      const [v, k] = readVarint(b, i);
      i = k;
      yield { num, wt, int: v, bytes: new Uint8Array() };
    } else if (wt === 2) {
      const [len, k] = readVarint(b, i);
      i = k;
      const n = Number(len);
      if (i + n > b.length) throw new Error("truncated field");
      yield { num, wt, int: 0n, bytes: b.subarray(i, i + n) };
      i += n;
    } else if (wt === 1) i += 8;
    else if (wt === 5) i += 4;
    else throw new Error(`unsupported wire type ${wt}`);
  }
}

const i64 = (v: bigint): string => (v >= 1n << 63n ? v - (1n << 64n) : v).toString();
const str = (b: Uint8Array): string => new TextDecoder().decode(b);

export function decodePosting(b: Uint8Array): Posting {
  const p: Posting = { dim: "0", amount: "0" };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 0) p.dim = i64(f.int);
    else if (f.num === 2 && f.wt === 0) p.amount = i64(f.int);
    else if (f.num === 3 && f.wt === 2) p.currencyCode = str(f.bytes);
    else if (f.num === 4 && f.wt === 2) p.instrument = str(f.bytes);
    else if (f.num === 5 && f.wt === 0) p.quantity = i64(f.int);
  }
  return p;
}

export function decodeTransaction(b: Uint8Array): Transaction {
  const t: Transaction = { postings: [] };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 2) t.name = str(f.bytes);
    else if (f.num === 2 && f.wt === 2) t.postings.push(decodePosting(f.bytes));
    else if (f.num === 3 && f.wt === 2) t.controlPlaneHash = str(f.bytes);
  }
  return t;
}

export function decodeListTransactionsResponse(b: Uint8Array): ListTransactionsResponse {
  const r: ListTransactionsResponse = { transactions: [] };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 2) r.transactions.push(decodeTransaction(f.bytes));
    else if (f.num === 2 && f.wt === 2) r.nextPageToken = str(f.bytes);
  }
  return r;
}

export function decodeAccount(b: Uint8Array): Account {
  const a: Account = { dim: "0", displayName: "", accountType: "ACCOUNT_TYPE_UNSPECIFIED" };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 2) a.name = str(f.bytes);
    else if (f.num === 2 && f.wt === 0) a.dim = i64(f.int);
    else if (f.num === 3 && f.wt === 2) a.displayName = str(f.bytes);
    else if (f.num === 4 && f.wt === 0) a.accountType = ACCOUNT_TYPES[Number(f.int)] ?? "ACCOUNT_TYPE_UNSPECIFIED";
    else if (f.num === 5 && f.wt === 0) a.normalSide = SIDES[Number(f.int)] ?? "SIDE_UNSPECIFIED";
  }
  return a;
}

export function decodeListAccountsResponse(b: Uint8Array): ListAccountsResponse {
  const r: ListAccountsResponse = { accounts: [] };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 2) r.accounts.push(decodeAccount(f.bytes));
    else if (f.num === 2 && f.wt === 2) r.nextPageToken = str(f.bytes);
  }
  return r;
}

export function decodeAccountBalance(b: Uint8Array): AccountBalance {
  const r: AccountBalance = { account: "", debits: "0", credits: "0" };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 2) r.account = str(f.bytes);
    else if (f.num === 2 && f.wt === 0) r.debits = i64(f.int);
    else if (f.num === 3 && f.wt === 0) r.credits = i64(f.int);
    else if (f.num === 4 && f.wt === 2) r.currencyCode = str(f.bytes);
  }
  return r;
}

export function decodeTrialBalance(b: Uint8Array): TrialBalance {
  const t: TrialBalance = { name: "", debits: "0", credits: "0", difference: "0", accountBalances: [] };
  for (const f of fields(b)) {
    if (f.num === 1 && f.wt === 2) t.name = str(f.bytes);
    else if (f.num === 2 && f.wt === 0) t.debits = i64(f.int);
    else if (f.num === 3 && f.wt === 0) t.credits = i64(f.int);
    else if (f.num === 4 && f.wt === 0) t.difference = i64(f.int);
    else if (f.num === 5 && f.wt === 2) t.accountBalances.push(decodeAccountBalance(f.bytes));
    else if (f.num === 6 && f.wt === 2) t.controlPlaneHash = str(f.bytes);
  }
  return t;
}
