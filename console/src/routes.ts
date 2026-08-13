// Every screen this console has, and what each one reads.
//
// ⛔ THIS IS THE ARTICLE OF FAITH THE OLD CONSOLE COULD NOT MAKE. The SPA was
// one URL, so a break could be found and not cited. Ratio's argument is that a
// figure names the journal prefix it was folded from and the configuration it
// ran under — that it can be CHECKED rather than trusted. A console whose
// answer to "which break?" is "scroll" is arguing against the product.
//
// So every resource on `ratio.console.v1.Console` has a URL, and
// `//console:route_manifest_test` holds this file to that from both sides:
//
//   * every route named here has a `page.tsx` (or `layout.tsx`) on disk, and
//   * every one of the 34 RPCs in console.proto is read by something here.
//
// ⚠ THE SECOND DIRECTION IS THE ONE THAT MATTERS. `//web:rendered_test` existed
// because "a field can be declared, transcoded, served, typechecked and mirrored
// while NO COMPONENT READS IT — that has already happened once." An RPC nobody
// reads is the same defect one level up, and this is where it gets caught.

export interface Route {
  /** The URL, as Next writes it. */
  readonly path: string;
  /** The file, relative to `src/app`. */
  readonly file: string;
  /** Names exported by `src/wire/client.ts` that this file calls.
   *
   * ⚠ At most THREE per file. Two timeouts stack behind every one of these —
   * the Lambda's 15s and the gateway's 15s — and `watch.rs` sends
   * `Connection: close`, so there is no keep-alive to amortize them. The
   * variable that decides whether a page renders is the NUMBER of upstream
   * calls, not the distance to them. The manifest test enforces the ceiling. */
  readonly reads: readonly string[];
}

export const ROUTES: readonly Route[] = [
  // ── Chrome ───────────────────────────────────────────────────────────────
  { path: "/", file: "page.tsx", reads: ["listFunds"] },
  { path: "/signin", file: "signin/page.tsx", reads: [] },
  { path: "/funds", file: "funds/page.tsx", reads: ["listFunds"] },
  // The rail. Every fund-scoped page renders inside it.
  { path: "(layout)/funds", file: "funds/layout.tsx", reads: ["listFunds"] },
  // One GetFund for the header and the four stat tiles, so no child repeats it.
  {
    path: "(layout)/funds/[fund]",
    file: "funds/[fund]/layout.tsx",
    reads: ["getFund"],
  },
  {
    path: "/funds/[fund]",
    file: "funds/[fund]/page.tsx",
    reads: ["getFund", "listChangeLogEntries"],
  },

  // ── Exceptions ───────────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/breaks",
    file: "funds/[fund]/breaks/page.tsx",
    reads: ["listBreaks"],
  },
  {
    path: "/funds/[fund]/breaks/[break]",
    file: "funds/[fund]/breaks/[break]/page.tsx",
    reads: ["getBreak"],
  },

  // ── The chart ────────────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/accounts",
    file: "funds/[fund]/accounts/page.tsx",
    reads: ["listAccounts"],
  },
  {
    path: "/funds/[fund]/accounts/[account]",
    file: "funds/[fund]/accounts/[account]/page.tsx",
    reads: ["getAccount", "listPostings"],
  },
  {
    path: "/funds/[fund]/accounts/[account]/postings/[posting]",
    file: "funds/[fund]/accounts/[account]/postings/[posting]/page.tsx",
    reads: ["getPosting"],
  },

  // ── Positions ────────────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/positions",
    file: "funds/[fund]/positions/page.tsx",
    reads: ["listPositions"],
  },
  {
    path: "/funds/[fund]/positions/[position]",
    file: "funds/[fund]/positions/[position]/page.tsx",
    reads: ["getPosition", "listLots"],
  },
  {
    path: "/funds/[fund]/positions/[position]/lots/[lot]",
    file: "funds/[fund]/positions/[position]/lots/[lot]/page.tsx",
    reads: ["getLot"],
  },

  // ── NAV ──────────────────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/strikes",
    file: "funds/[fund]/strikes/page.tsx",
    reads: ["listNavStrikes"],
  },
  {
    path: "/funds/[fund]/strikes/[strike]",
    file: "funds/[fund]/strikes/[strike]/page.tsx",
    reads: ["getNavStrike"],
  },
  // A URL for a proof. The replay folds the prefix the strike pinned and says
  // whether the history is intact — that is a citation, not a page state.
  {
    path: "/funds/[fund]/strikes/[strike]/replay",
    file: "funds/[fund]/strikes/[strike]/replay/page.tsx",
    reads: ["getNavStrike", "replayNavStrike"],
  },

  // ── Configuration ────────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/config",
    file: "funds/[fund]/config/page.tsx",
    reads: ["listConfigVersions"],
  },
  {
    path: "/funds/[fund]/config/[version]",
    file: "funds/[fund]/config/[version]/page.tsx",
    reads: ["getConfigVersion"],
  },
  {
    path: "/funds/[fund]/config/[version]/diff",
    file: "funds/[fund]/config/[version]/diff/page.tsx",
    reads: ["getConfigVersion", "diffConfigVersions"],
  },

  // ── Rules ────────────────────────────────────────────────────────────────
  //
  // ⛔ TWO LISTS, AND THEY DO NOT MERGE. Active rules come from `listRules`;
  // unapproved drafts come from the change log, where `ratio-console` emits
  // them with `configDigest: "proposal"` and `actorKind: MODEL`. The gap
  // between the two lists is exactly what a person's approval bought, and
  // merging them erases it.
  //
  // ⛔ AND THERE IS NO APPROVE BUTTON. `approve_rule` is absent from the MCP
  // tool list on purpose and `//demo:rehearse_test` asserts it; approval is
  // `ratio approve` at a terminal. A screen offering a second way round the
  // fence would make the fence worthless.
  {
    path: "/funds/[fund]/rules",
    file: "funds/[fund]/rules/page.tsx",
    reads: ["listRules", "listChangeLogEntries"],
  },
  {
    path: "/funds/[fund]/rules/[rule]",
    file: "funds/[fund]/rules/[rule]/page.tsx",
    reads: ["getRule"],
  },

  // ── The data plane ───────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/data",
    file: "funds/[fund]/data/page.tsx",
    reads: ["listDeliveries", "listPendingFacts"],
  },
  {
    path: "/funds/[fund]/data/deliveries/[delivery]",
    file: "funds/[fund]/data/deliveries/[delivery]/page.tsx",
    reads: ["getDelivery"],
  },
  {
    path: "/funds/[fund]/data/pending/[fact]",
    file: "funds/[fund]/data/pending/[fact]/page.tsx",
    reads: ["getPendingFact"],
  },
  {
    path: "/funds/[fund]/data/templates",
    file: "funds/[fund]/data/templates/page.tsx",
    reads: ["listTemplates"],
  },
  {
    path: "/funds/[fund]/data/templates/[template]",
    file: "funds/[fund]/data/templates/[template]/page.tsx",
    reads: ["getTemplate"],
  },

  // ── Corporate actions ────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/actions",
    file: "funds/[fund]/actions/page.tsx",
    reads: ["listCorporateActions"],
  },
  {
    path: "/funds/[fund]/actions/[action]",
    file: "funds/[fund]/actions/[action]/page.tsx",
    reads: ["getCorporateAction"],
  },

  // ── The change log ───────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/changes",
    file: "funds/[fund]/changes/page.tsx",
    reads: ["listChangeLogEntries"],
  },
  {
    path: "/funds/[fund]/changes/[entry]",
    file: "funds/[fund]/changes/[entry]/page.tsx",
    reads: ["getChangeLogEntry"],
  },

  // ── The four writes ──────────────────────────────────────────────────────
  {
    path: "/funds/[fund]/record",
    file: "funds/[fund]/record/page.tsx",
    reads: ["listRules", "applyEvent"],
  },
  {
    path: "/funds/[fund]/ingest",
    file: "funds/[fund]/ingest/page.tsx",
    reads: ["listTemplates", "ingestDelivery", "admitFacts"],
  },
  {
    path: "/funds/[fund]/mark",
    file: "funds/[fund]/mark/page.tsx",
    reads: ["listPositions", "markPositions"],
  },
];
