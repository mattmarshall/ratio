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
  totalDebit: Int64;
  totalCredit: Int64;
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

export interface NavStrike {
  name: string;
  /** RFC 3339. proto3 renders a Timestamp as a string. */
  valuationTime: string;
  actor: string;
  journalPosition: Int64;
  journalDigest: string;
  netAssetValue: Int64;
  trialBalanceDifference: Int64;
  configDigest: string;
}

export interface ReplayNavStrikeResponse {
  name: string;
  /** The journal prefix still hashes as it did: history was not rewritten. */
  historyIntact: boolean;
  /** The fold landed on the same figures: the engine is deterministic. */
  reproduced: boolean;
  netAssetValue: Int64;
  journalDigest: string;
}

export interface ListNavStrikesResponse {
  navStrikes: NavStrike[];
  nextPageToken: string;
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

export type RuleChangeKind = "ADDED" | "CHANGED" | "REMOVED" | "UNSPECIFIED";

/** One promoted configuration, oldest first — `sequence` 1 is the first. */
export interface ConfigVersion {
  name: string;
  digest: string;
  sequence: Int64;
  active: boolean;
  /** Empty for a version promoted before the changelog existed. Not guessed. */
  actor: string;
  approveTime: string;
  subject: string;
  rules: string[];
}

export interface ListConfigVersionsResponse {
  configVersions: ConfigVersion[];
  nextPageToken: string;
}

export interface RuleChange {
  ruleId: string;
  kind: RuleChangeKind;
  baseForm: string;
  form: string;
}

export interface DiffConfigVersionsResponse {
  baseDigest: string;
  digest: string;
  changes: RuleChange[];
}

export type AccountType =
  | "ASSET"
  | "EQUITY"
  | "EXPENSE"
  | "LIABILITY"
  | "REVENUE"
  | "UNSPECIFIED";

/** One line of the trial balance. */
export interface Account {
  name: string;
  displayName: string;
  dimension: Int64;
  type: AccountType;
  debit: Int64;
  credit: Int64;
  /** Debits minus credits, signed and NOT flipped to the normal side. */
  balance: Int64;
  /** Sitting on the side its type calls abnormal. Legal, worth a look. */
  abnormal: boolean;
  postingCount: Int64;
}

export interface ListAccountsResponse {
  accounts: Account[];
  nextPageToken: string;
}

export interface Posting {
  name: string;
  entryId: string;
  memo: string;
  amount: Int64;
  runningBalance: Int64;
  configDigest: string;
}

export interface ListPostingsResponse {
  postings: Posting[];
  nextPageToken: string;
}
