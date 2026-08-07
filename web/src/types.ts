// The console API's wire types.
//
// Hand-written to mirror `proto/ratio/v1/console.proto` under proto3's
// canonical JSON mapping: lowerCamelCase fields, enums as their names, and
// EVERY int64 as a string.
//
// ⛔ Checked, not trusted. `//web:types_test` reads console.proto and asserts
// every message field appears here with the right name — the same discipline
// as `//crates/ratio-console:transcode_test`, and for the same reason: a
// hand-written mirror that nothing compares is a mirror that drifts.
//
// Why not generated: protobuf-es would mean a second codegen toolchain (buf or
// protoc-gen-es) for six messages that change when the product does. The check
// costs less than the generator and fails just as loudly.

/** Money and counts. A string because the server sends int64 as a string. */
export type Int64 = string;

export type FundState =
  | "AWAITING_PRICES"
  | "BLOCKED"
  | "IN_REVIEW"
  | "STRUCK"
  | "UNSPECIFIED";

export type Severity = "LOW" | "MEDIUM" | "HIGH" | "UNSPECIFIED";

export type ActorKind = "PERSON" | "MODEL" | "UNSPECIFIED";

export interface Fund {
  name: string;
  displayName: string;
  currencyCode: string;
  state: FundState;
  netAssetValue: Int64;
  trialBalanceDifference: Int64;
  openDifference: Int64;
  entryCount: Int64;
  openBreakCount: Int64;
  configDigest: string;
}

export interface BreakPosting {
  entryId: string;
  memo: string;
  amount: Int64;
  configDigest: string;
}

export interface Break {
  name: string;
  account: string;
  accountDimension: Int64;
  severity: Severity;
  explained: boolean;
  cause: string;
  ratioAmount: Int64;
  reportedAmount: Int64;
  difference: Int64;
  postings: BreakPosting[];
  configDigest: string;
}

export interface ChangeLogEntry {
  name: string;
  actor: string;
  actorKind: ActorKind;
  action: string;
  subject: string;
  detail: string;
  configDigest: string;
}

export interface ListFundsResponse {
  funds: Fund[];
  nextPageToken: string;
}
export interface ListBreaksResponse {
  breaks: Break[];
  nextPageToken: string;
}
export interface ListChangeLogEntriesResponse {
  changeLogEntries: ChangeLogEntry[];
  nextPageToken: string;
}
