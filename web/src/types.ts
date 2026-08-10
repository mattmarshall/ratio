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
  /** Facts read off a file that cannot post yet. Non-zero blocks the NAV. */
  pendingFactCount: Int64;
  configDigest: string;
  /**
   * Which lots a sale gives up, as the active configuration declares it.
   *
   * A term of an administration agreement: four methods give four different
   * taxable incomes from one holding and one trade, with nothing on the balance
   * sheet moving. Empty when the book has no configuration.
   */
  lotMethod: string;
  /**
   * ⛔ Whether the configuration DECLARES that method, or merely gets it. A rule
   * set that says nothing is relieved oldest-first by custom, not by election —
   * printing `lotMethod` as an agreed term without checking this asserts a
   * decision nobody made.
   */
  lotMethodDeclared: boolean;
  /**
   * ⛔ CREDIT-NORMAL — a gain reads NEGATIVE. Print it through `gainOf` and
   * never raw, or every profitable fund shows a minus sign.
   *
   * Empty (not "0") when the chart names no realized-gain role: a fund that
   * realized nothing and a fund whose engine cannot tell are different answers.
   */
  realizedGain: Int64;
  basisRelieved: Int64;
  /** Parts of `realizedGain`, same sign convention. The three sum to it. */
  shortTermGain: Int64;
  longTermGain: Int64;
  /** Disposals no holding period could be established for. The remainder. */
  unclassifiedGain: Int64;
  /** Days held for a gain to be long-term. A jurisdiction's number, not 365. */
  longTermDays: Int64;
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
  /**
   * Why this figure cannot be read at face value, if it cannot. Empty means it
   * stands on its own.
   *
   * ⛔ ON the strike rather than beside it: a valuation point is never
   * restated, so a late corporate action cannot correct the NAVs it should
   * have been in — and a qualification a caller fetches separately is one a
   * caller renders the figure without.
   */
  qualification: string[];
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

/**
 * A corporate action, as announced.
 *
 * ⛔ ANNOUNCEMENT IS NOT APPLICATION. An action is here from the moment
 * somebody tells us, and `applied` says whether the book has moved — a list
 * showing only applied actions would hide exactly the ones a NAV was struck
 * without.
 *
 * ⛔ AND IT IS NOT IDEMPOTENT, unlike everything else this console can do.
 * Marking twice posts nothing; a two-for-one applied twice quadruples the
 * position and the trial balance goes on tying. `applied` is not a status
 * badge, it is the idempotence.
 */
export interface CorporateAction {
  name: string;
  instrument: string;
  /** Units received / units given up. A 2-for-1 is `2` and `1`. */
  numerator: Int64;
  denominator: Int64;
  /** The same ratio the way a person says it: `2-for-1`. */
  form: string;
  /** The day it takes effect. A NAV struck on or after it should include it. */
  exDate: CalendarDate | null;
  /** When we were told — NOT when it took effect. RFC 3339. */
  announceTime: string;
  applied: boolean;
  /** Where it landed in the journal. Zero when unapplied. */
  journalPosition: Int64;
  /**
   * Where the ANNOUNCEMENT sits in the journal. Zero means it does not.
   *
   * ⛔ Zero is not "unknown", it is "pinned by nothing". An announcement in the
   * journal is inside the prefix every later strike pins, so a replay
   * re-derives the same figure forever. One in a side plane is pinned by no
   * strike — a replay would read whatever arrived since.
   */
  announcePosition: Int64;
  /**
   * ⭐ The NAV strikes this action was NOT in, as resource names.
   *
   * The reverse of `NavStrike.qualification`: a strike knows what qualifies it,
   * and only the action knows the full extent of what it disturbed. This list
   * can never be emptied — a valuation point is never restated — so it is the
   * permanent record of what arriving late cost.
   */
  qualifiedNavStrikes: string[];
}

export interface ListCorporateActionsResponse {
  corporateActions: CorporateAction[];
  nextPageToken: string;
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

export type RuleKind =
  | "ACCRUAL"
  | "DIVIDEND"
  | "MARK"
  | "TRADE"
  | "UNSPECIFIED";

/** One rule of the configuration in force. */
export interface Rule {
  name: string;
  ruleId: string;
  kind: RuleKind;
  description: string;
  /** The rule as the rules screen shows it. */
  form: string;
  /** The accounts it posts to, in leg order. */
  accounts: string[];
}

export interface ListRulesResponse {
  rules: Rule[];
  nextPageToken: string;
}

export interface EntryPosting {
  account: string;
  displayName: string;
  amount: Int64;
}

export interface Entry {
  name: string;
  entryId: string;
  memo: string;
  configDigest: string;
  postings: EntryPosting[];
}

/**
 * What recording an event produced.
 *
 * ⛔ `amount` on the REQUEST is a decimal string as typed — "250000.00" — and
 * is parsed on the server. The browser never does arithmetic on money.
 */
export interface ApplyEventRequest {
  ruleId: string;
  eventId: string;
  amount: string;
  days: string;
  validateOnly: boolean;
}

export interface ApplyEventResponse {
  entry: Entry | null;
  validateOnly: boolean;
  netAssetValue: Int64;
  previousNetAssetValue: Int64;
}

/** A file received on the data plane. */
export interface Delivery {
  name: string;
  digest: string;
  origin: string;
  receiveTime: string;
  byteCount: Int64;
  factCount: Int64;
  pendingFactCount: Int64;
}

export interface ListDeliveriesResponse {
  deliveries: Delivery[];
  nextPageToken: string;
}

/**
 * Why a reference did not resolve.
 *
 * ABSENT and AMBIGUOUS take different remedies — add the instrument, versus
 * de-duplicate the master — so they are never collapsed into "unresolved".
 */
export type Blocker = "ABSENT" | "AMBIGUOUS" | "UNSPECIFIED";

export interface PendingFact {
  name: string;
  reference: string;
  kind: string;
  blocker: Blocker;
  detail: string;
  deliveryDigest: string;
  row: Int64;
  templateId: string;
}

export interface ListPendingFactsResponse {
  pendingFacts: PendingFact[];
  nextPageToken: string;
}

/** A row the template could not map. Per row, never per file. */
export interface RejectedRow {
  row: Int64;
  reason: string;
}

export interface IngestDeliveryRequest {
  templateId: string;
  content: string;
  origin: string;
  validateOnly: boolean;
}

export interface IngestDeliveryResponse {
  deliveryDigest: string;
  rowCount: Int64;
  factCount: Int64;
  newFactCount: Int64;
  readyCount: Int64;
  rejected: RejectedRow[];
  pending: PendingFact[];
  validateOnly: boolean;
}

export interface AdmitFactsRequest {
  validateOnly: boolean;
}

export interface AdmitFactsResponse {
  postedCount: Int64;
  /** Reference data — a price file, an FX file — posts nothing by design. */
  recordedCount: Int64;
  pendingCount: Int64;
  refused: string[];
  netAssetValue: Int64;
  previousNetAssetValue: Int64;
  validateOnly: boolean;
}

/** A mapping template in force. */
export interface Template {
  name: string;
  templateId: string;
  factKind: string;
  /** The template as a person reads it. */
  form: string;
  /** False for reference data, which is recorded and never posted. */
  posts: boolean;
}

export interface ListTemplatesResponse {
  templates: Template[];
  nextPageToken: string;
}

/** A calendar date. Not a timestamp — a valuation date has no timezone. */
export interface CalendarDate {
  year: number;
  month: number;
  day: number;
}

/**
 * What the fund holds in one instrument, in one account.
 *
 * ⛔ `instrument` is EMPTY for value in the account that is not attributed to
 * any instrument. That is a real row, not a missing one — the rows sum to the
 * accounts, and a view that filtered it out would disagree with the trial
 * balance by exactly the amount it hid.
 */
export interface Position {
  name: string;
  account: string;
  accountLabel: string;
  instrument: string;
  instrumentLabel: string;
  /** Whole units. A measure, not a conserved quantity. */
  quantity: Int64;
  /** Minor units. Value does conserve. */
  value: Int64;
  /**
   * The date this was last marked at, NULL if it never has been.
   *
   * ⛔ Two positions can show the same number for opposite reasons — one
   * because that is what it is worth, the other because that is what it cost
   * and nobody has priced it. A screen that does not distinguish them is
   * reporting an opinion as a fact.
   */
  /**
   * How many open tax lots stand behind this figure.
   *
   * ⛔ The number that does NOT appear in a NAV, shown beside one that does.
   * A position is a chart and its lots are a history; the whole scale argument
   * is that valuing the first does not touch the second.
   */
  openLotCount: Int64;
  markDate: CalendarDate | null;
}

export interface ListPositionsResponse {
  positions: Position[];
  nextPageToken: string;
}

/**
 * One open tax lot: what was bought, when, and what it cost.
 *
 * ⛔ THE HISTORY BEHIND A POSITION, and the one read whose cost grows with it.
 * A fund's positions are a chart of a few hundred lines whatever its age; its
 * lots are every purchase it still holds. Fetched per position, on demand, and
 * there is deliberately no way to ask for every lot in the fund at once.
 */
export interface Lot {
  name: string;
  /** Acquisition order — the journal position of the entry that opened it. */
  sequence: Int64;
  units: Int64;
  /** ⛔ COST, NOT VALUE. A lot is never revalued. */
  cost: Int64;
  /**
   * ⛔ NULL when the entry that opened it carried no trade date, and the
   * holding-period methods REFUSE such a holding rather than guessing. Both
   * defaults are wrong in opposite directions.
   */
  acquired: CalendarDate | null;
}

export interface ListLotsResponse {
  lots: Lot[];
  nextPageToken: string;
}

/** One position's valuation. */
export interface Mark {
  instrument: string;
  instrumentLabel: string;
  quantity: Int64;
  /** What the book held it at, what it is worth, and the difference posted. */
  carrying: Int64;
  market: Int64;
  movement: Int64;
  price: Int64;
  priceDate: CalendarDate | null;
}

export interface MarkPositionsRequest {
  valuationDate: CalendarDate;
  validateOnly: boolean;
}

export interface MarkPositionsResponse {
  marks: Mark[];
  /** ⛔ NOT marked at zero. Zero says "worth what it cost"; these are unvalued. */
  unpriced: Mark[];
  inexact: string[];
  postedCount: Int64;
  netAssetValue: Int64;
  previousNetAssetValue: Int64;
  validateOnly: boolean;
}
