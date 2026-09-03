// The console API's wire types.
//
// Hand-written to mirror `proto/ratio/v1/console.proto` under proto3's
// canonical JSON mapping: lowerCamelCase fields, enums as their names, and
// EVERY int64 as a string.
//
// ⛔ Checked, not trusted. `//proto:mirrors_test` reads console.proto and
// asserts every message field appears here with the right name — the same
// discipline as `//crates/ratio-console:transcode_test`, and for the same
// reason: a hand-written mirror that nothing compares is a mirror that drifts.
//
// ⚠ THIS FILE'S PATH IS LOAD-BEARING. `proto/BUILD.bazel` names it by label
// (`//console:src/wire/types.ts`) and `console/BUILD.bazel` exports it. Moving
// it without moving both is a test that silently stops running.
//
// Why not generated: protobuf-es would mean a second codegen toolchain (buf or
// protoc-gen-es) for six messages that change when the product does. The check
// costs less than the generator and fails just as loudly.

/** Money and counts. A string because the server sends int64 as a string. */
export type Int64 = string;

export type BookKind = "PERSONAL" | "INVESTMENT" | "PROJECT" | "UNSPECIFIED";

export type FundState =
  | "AWAITING_PRICES"
  | "BLOCKED"
  | "IN_REVIEW"
  | "STRUCK"
  | "UNSPECIFIED";

/**
 * A journal and its content-addressed configuration.
 *
 * ⭐ THE UNIT THAT OWNS THE BOOKS. A fund and a WorkOS organization are
 * optional layers, not parents. Kind selects the chart, not a fork of the
 * kernel.
 */
export interface Book {
  name: string;
  displayName: string;
  kind: BookKind;
  currencyCode: string;
  /** funds/id, or empty when the book is independent. */
  fund: string;
  /** WorkOS organization id, or empty. */
  organization: string;
  defaultView: string;
  entryCount: Int64;
  configDigest: string;
  trialBalanceDifference: Int64;
}

export interface ListBooksResponse {
  books: Book[];
  nextPageToken: string;
}

export interface CreateBookRequest {
  book: Book;
  bookId: string;
}

export type Severity = "LOW" | "MEDIUM" | "HIGH" | "UNSPECIFIED";

export type ActorKind = "PERSON" | "MODEL" | "UNSPECIFIED";

/**
 * How a view decides the day it recognises an entry on.
 *
 * ⛔ `RECORDED` IS NOT A SETTLEMENT CONVENTION. It is the journal's own
 * order, consulting no date, which is what every book has always done — and
 * rendering it as "T+0" asserts an election nobody made. `View.declared` is
 * what separates the two.
 */
export type ViewBasis = "RECORDED" | "TRADE" | "SETTLEMENT" | "UNSPECIFIED";

export interface Fund {
  name: string;
  displayName: string;
  currencyCode: string;
  /**
   * Where the fund is in its NAV day, and how many breaks are open — BOTH AS
   * `defaultView` SEES THEM.
   *
   * ⛔ Both are view-dependent, and they answer for one view so the fund rail
   * costs one call rather than one per fund. Never render either without
   * `defaultView` beside it: a figure that depends on a recognition convention
   * and does not name it is the defect the view split exists to prevent.
   */
  state: FundState;
  openBreakCount: Int64;
  /**
   * Debits minus credits — the same zero in EVERY view.
   *
   * ⭐ Fund-level, and that is the check that the line between Fund and View is
   * drawn right. A view keeps or drops whole entries and each entry conserves,
   * so the difference cannot move. The two COLUMN totals can, and live on
   * `View`.
   */
  trialBalanceDifference: Int64;
  entryCount: Int64;
  /** Facts read off a file that cannot post yet. Non-zero blocks the NAV. */
  pendingFactCount: Int64;
  configDigest: string;
  /** The view `state` and `openBreakCount` answer for, as a view id. */
  defaultView: string;
  /** How many views this fund declares. Always at least one. */
  viewCount: Int64;
  /**
   * Which lots a sale gives up, as the active configuration declares it.
   *
   * A term of an administration agreement: four methods give four different
   * taxable incomes from one holding and one trade, with nothing on the balance
   * sheet moving. Empty when the book has no configuration.
   *
   * ⚠ Fund-level, not per view — a view overrides TIMING only. Views still
   * reach different realized gains under one election, because each has
   * recognised a different set of open lots when a sale arrives.
   */
  lotMethod: string;
  /**
   * ⛔ Whether the configuration DECLARES that method, or merely gets it. A rule
   * set that says nothing is relieved oldest-first by custom, not by election —
   * printing `lotMethod` as an agreed term without checking this asserts a
   * decision nobody made.
   */
  lotMethodDeclared: boolean;
  /** Days held for a gain to be long-term. A jurisdiction's number, not 365. */
  longTermDays: Int64;
}

/**
 * One lens over one journal: which entries it recognises, and what follows.
 *
 * ⭐ A view is a recognition PREDICATE, not a second book. Every figure below
 * depends on which entries are in scope, which is why none of them is on
 * `Fund`.
 */
export interface View {
  name: string;
  displayName: string;
  basis: ViewBasis;
  /**
   * Open days from trade to settlement. `"0"` unless `basis` is settlement.
   *
   * ⚠ The configuration spells this `settles_in`; AIP-140 refuses a preposition
   * in a wire field name, so the contract spells it out. Same number.
   */
  settlementOpenDays: Int64;
  calendar: string;
  holidayCount: Int64;
  /**
   * ⛔ Whether the configuration DECLARES this view, or whether it is the one
   * every book has by default. `BASIS_RECORDED` with `declared: false` is the
   * absence of an election and must never be rendered as an agreed basis —
   * the same trap as `lotMethodDeclared`.
   */
  declared: boolean;
  /** `null` on a recorded-basis view, which has no cut — and null is the
   * honest answer there, not the epoch. */
  recognisedThrough: CalendarDate | null;
  /**
   * Entries this view cannot place: no trade date, or a pinned configuration
   * that does not declare it. ⛔ Shown, never silently dropped.
   */
  unplaceableEntryCount: Int64;
  netAssetValue: Int64;
  /** ⚠ View-level, unlike `Fund.trialBalanceDifference` — fewer entries
   * recognised means smaller columns, while the difference stays zero. */
  totalDebit: Int64;
  totalCredit: Int64;
  openDifference: Int64;
  openBreakCount: Int64;
  state: FundState;
  /**
   * ⛔ CREDIT-NORMAL — a gain reads NEGATIVE. Print it through `gainOf` and
   * never raw, or every profitable fund shows a minus sign.
   *
   * Empty (not "0") when the chart names no realized-gain role.
   */
  realizedGain: Int64;
  basisRelieved: Int64;
  /** Parts of `realizedGain`, same sign convention. The three sum to it. */
  shortTermGain: Int64;
  longTermGain: Int64;
  /** Disposals no holding period could be established for. The remainder. */
  unclassifiedGain: Int64;
  /**
   * ⛔ THE SCALE ARGUMENT, AS TWO NUMBERS SIDE BY SIDE. Striking a NAV touches
   * the first and not the second.
   */
  openLotCount: Int64;
  positionCount: Int64;
  /**
   * How long the fold that strikes this view's NAV took — proto3 renders a
   * Duration as seconds with an `s` suffix, e.g. `"0.000005291s"`.
   *
   * ⛔ THE MAINTAINED FOLD, never the cold build. Two curves.
   */
  navStrike: string;
  /**
   * ⭐ The journal prefix every figure above was folded from — THE SAME NUMBER
   * ON EVERY VIEW OF ONE FUND. One pass feeds them all, so two views put side
   * by side differ by a recognition convention and never by staleness.
   */
  journalPosition: Int64;
}

/**
 * Billed vs earned, retainage, and cost by work-package account.
 *
 * Empty billed / earned / retainage / phase budget means unset — not a
 * fake zero. Cost `"0"` on a seeded phase is a true zero (nothing posted).
 */
export interface ProjectProgressResponse {
  name: string;
  billed: Int64;
  earned: Int64;
  billedMinusEarned: Int64;
  retainageReceivable: Int64;
  retainagePayable: Int64;
  phases: PhaseCost[];
}

/** Cost (and optional authorized spend) on one work-package account. */
export interface PhaseCost {
  account: string;
  displayName: string;
  cost: Int64;
  /** Empty when `[project.phase]` omits this account. `"0"` is a set baseline. */
  budget: Int64;
}

/** One entry two views disagree about, and what it is worth. */
export interface RecognitionDifference {
  entryId: string;
  memo: string;
  /** `null` when the record carries none — which is why a view may be unable
   * to place it. */
  tradeDate: CalendarDate | null;
  /** The day each side recognises it on. `null` where that side cannot. */
  recognisedHere: CalendarDate | null;
  recognisedThere: CalendarDate | null;
  /**
   * ⛔ What this entry contributes to `difference` — NOT to either NAV. The
   * sign follows from which list it is in, so the two lists sum to `difference`
   * by construction and the screen renders the arithmetic instead of asserting
   * it.
   */
  netAssetValueEffect: Int64;
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
  tolerance: Tolerance | null;
  explanation: BreakExplanation | null;
}

export interface BreakExplanation {
  text: string;
  actor: string;
  acceptTime: string;
  difference: Int64;
  configDigest: string;
  journalPosition: Int64;
  journalDigest: string;
  qualification: string[];
}

export interface Tolerance {
  belowNotice: Int64;
  blocksNav: Int64;
  declared: boolean;
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
  /**
   * The view this NAV was struck in, as a view id.
   *
   * ⛔ A valuation point has ONE answer — per view. Two views striking one
   * day are two answers to two questions; without this they are
   * indistinguishable from a restatement, which is refused.
   */
  view: string;
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
 * Which of the two answers to "what is this fund worth" a step belongs to.
 *
 * `RECORDED` is the fold that produced the strike and grows with the journal;
 * `MAINTAINED` is the same figure off totals somebody keeps, and is flat in
 * fragmentation. ⛔ Both are always on the page: `ratio bench` reports two
 * curves and both must be quoted.
 */
export type PlanGroup = "UNSPECIFIED" | "RECORDED" | "MAINTAINED";

/**
 * ⛔ `UNREAD` IS NOT `REJECTED`. A rejected step would have worked and cost
 * more. An unread one is never touched — twenty million tax lots are not in a
 * NAV's cost at all — and a client that renders them alike loses the only claim
 * on this screen that is a theorem.
 */
export type PlanRole =
  | "UNSPECIFIED"
  | "CHOSEN"
  | "REJECTED"
  | "REFUSAL"
  | "UNREAD";

export type PlanEdgeKind = "UNSPECIFIED" | "FLOW" | "REFUSAL" | "UNREAD";

/**
 * One step of a NAV strike's plan.
 *
 * ⛔ EVERY FIGURE IS AN `Int64`, AND AN EMPTY ONE MEANS THERE IS NO FIGURE —
 * never "0". `realizedGain` above carries the same convention. Here it is
 * load-bearing twice: an unmeasured step rendered as `0` reads as "instant",
 * and a step nothing costs rendered as `0` reads as "free".
 */
export interface PlanNode {
  id: string;
  /** What the step does, in the operator style EXPLAIN uses. */
  operator: string;
  /** The one line that qualifies it — the equivalent of an index name. */
  detail: string;
  group: PlanGroup;
  role: PlanRole;
  /** The theorem or the function this cost comes from. Never empty. */
  cites: string;
  /** Why it costs what it costs. Rendered, not decoration. */
  note: string[];
  /** From the proved model or the strike's record. Empty where nothing costs it. */
  estimatedReads: Int64;
  /**
   * `estimatedReads` times the calibrated rate, as a proto3 Duration —
   * `"0.000004436s"`. ⛔ `null` where nothing costs the step: a Duration is a
   * message, so absence is structural here rather than an empty string.
   */
  estimatedDuration: string | null;
  /** What an instrumented re-fold saw. Empty / null unless the plan was analyzed. */
  actualRows: Int64;
  actualDuration: string | null;
}

export interface PlanEdge {
  /**
   * ⛔ `source`/`target`, not `from`/`to`. `from` is on AIP-140's reserved-word
   * list, so the contract cannot spell it that way — and the Rust model spells
   * it the same, so nothing in between has to translate.
   */
  source: string;
  target: string;
  kind: PlanEdgeKind;
  /** Rows that travelled it. Empty when nothing measured them. */
  rows: Int64;
}

/**
 * The fund's shape, as the estimate was taken over it.
 *
 * ⛔ `securities` AND `lotsPer` ARE SEPARATE AND MUST STAY SO. 500 × 40,000 and
 * 10,000 × 2,000 are both twenty million open lots and are not the same fund —
 * one price is read per SECURITY, so they differ twentyfold in the term that
 * grows with the chart.
 */
export interface PlanDials {
  securities: Int64;
  currencies: Int64;
  /** ⚠ An average, rounded down. The action term multiplies it. */
  lotsPer: Int64;
  /** Announced and not yet rewritten. */
  openActions: Int64;
  accounts: Int64;
  /** One per (dimension, currency) — NOT one per security. */
  totalRows: Int64;
  openLots: Int64;
}

/**
 * How a NAV strike is computed, and what the alternatives would have cost.
 *
 * ⚠ Named for the method rather than for itself: AIP-136 requires a custom
 * method's response to be `<Rpc>Response` or the resource it operates on, and
 * the resource here is `NavStrike`.
 */
export interface ExplainNavStrikeResponse {
  name: string;
  view: string;
  nodes: PlanNode[];
  edges: PlanEdge[];
  /** Null when `estimateRefusal` is set. */
  dials: PlanDials | null;
  /**
   * Why there is no estimate, when there is none.
   *
   * ⛔ The maintained projection folds the whole journal with no cut, so it
   * cannot supply a shape for a trade- or settlement-basis view. Rendering the
   * recorded view's shape under another view's name is what this refuses.
   */
  estimateRefusal: string;
  /**
   * ⛔ When true, the actuals are this machine re-deriving the prefix NOW — not
   * what the original strike cost. Nothing was recorded at strike time.
   */
  analyzed: boolean;
  nanosPerRead: Int64;
  /** Where that rate came from. Rendered verbatim beside any duration. */
  provenance: string;
  /**
   * ⛔ ALL THREE, ALWAYS, EVEN WHEN THE REJECTED STEPS ARE COLLAPSED.
   * `chosenReads` is the flattering number and showing it alone is the
   * overclaim `ratio bench` exists to make hard.
   *
   * ⚠ None of them includes the capital term, which cannot be counted without
   * the chart roles.
   */
  chosenReads: Int64;
  /** The same period end with its open actions applied by rewriting the lots. */
  rewriteReads: Int64;
  /** Folding every open lot instead of reading the maintained totals. */
  scanReads: Int64;
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
/**
 * ⚠ No `defaultView` here — AIP-132 admits only the list and its page token in
 * a List response, and `Fund.defaultView` already carries it. A caller that
 * needs both already has the fund in hand.
 */
export interface ListViewsResponse {
  views: View[];
  nextPageToken: string;
}
export interface ReconcileViewsResponse {
  name: string;
  against: string;
  netAssetValue: Int64;
  againstNetAssetValue: Int64;
  /** This view's NAV minus the other's. */
  difference: Int64;
  /**
   * ⭐ The entries each side recognises and the other does not. These two
   * lists account for `difference` EXACTLY — that is a theorem
   * (`Ratio.Views.two_views_differ_by_exactly_what_is_in_flight`), so a
   * screen can show the arithmetic rather than assert it.
   */
  recognisedHere: RecognitionDifference[];
  recognisedThere: RecognitionDifference[];
  /**
   * Entries NEITHER view can place. ⛔ Shown, not omitted: leaving them out
   * makes a difference look fully explained when it is not.
   */
  unplaceable: RecognitionDifference[];
  /** The prefix both sides were folded from. One number, one pass. */
  journalPosition: Int64;
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
  /**
   * The same account, one row per currency it actually holds — untranslated.
   *
   * `debit`/`credit`/`balance` above are ONE figure per account, so they are
   * translated: an account holding dollars and euros reports their converted
   * sum. The rate is a judgment; the denominations are a fact. Empty on a
   * single-currency fund.
   */
  currencyTotals: CurrencyTotal[];
}

/** One account's activity in one denomination, before any translation. */
export interface CurrencyTotal {
  /**
   * ISO 4217, or empty for a posting that named no currency. Empty is its own
   * group and NOT the fund's currency.
   */
  currencyCode: string;
  /** Minor units in THIS currency — not converted. */
  debit: Int64;
  credit: Int64;
  /** Debits minus credits, in this currency. Signed, like `balance`. */
  balance: Int64;
  /**
   * What this denomination was multiplied by to reach the translated figure,
   * in hundredths. Empty for the fund's own currency, which has no rate fact.
   */
  rate: Int64;
  /**
   * The rate fact this translation cites. Empty for the base currency and for
   * an untyped leg — both translate at par without a fact.
   */
  rateFact: string;
  deliveryDigest: string;
  /** The config digest that fact pinned — not whichever is in force now. */
  configDigest: string;
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

export interface ListEntriesResponse {
  entries: Entry[];
  nextPageToken: string;
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
  /**
   * ⛔ WITHOUT THESE THREE AN EVENT MOVES VALUE AND NOTHING ELSE. The
   * projection's walk skips any posting that does not carry BOTH an instrument
   * and a quantity, so a trade sent without them opens no tax lot and relieves
   * none — while the entry balances, the trial balance ties, and the NAV moves
   * by the right amount. Nothing downstream objects; the position's unit count
   * is simply somebody else's.
   */
  instrument: string;
  /** Whole units. ⛔ POSITIVE on both sides — the rule decides the direction. */
  quantity: string;
  /** The day it was dealt. ⛔ What a lot's holding period is established from. */
  tradeDate: CalendarDate | null;
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

/**
 * A fact the data plane has recorded — a price, an FX rate, a trade.
 *
 * ⛔ APPEND-ONLY. A correction is a new fact with its own provenance, never an
 * edit of this one. `superseded` is derived from the log so both stay readable.
 */
export interface Fact {
  name: string;
  kind: string;
  reference: string;
  assertion: string;
  deliveryDigest: string;
  row: Int64;
  templateId: string;
  configDigest: string;
  superseded: boolean;
}

export interface ListFactsResponse {
  facts: Fact[];
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
  /**
   * The price fact the last mark cited. Empty if this position has never been
   * marked — then the value is cost, and there is no price to open.
   */
  priceFact: string;
  deliveryDigest: string;
  configDigest: string;
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
  /** The fact that supplied the price, so a mark preview names its evidence. */
  priceFact: string;
  deliveryDigest: string;
  configDigest: string;
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
