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
//   * every one of the 38 RPCs in console.proto is read by something here.
//
// ⚠ THAT COUNT IS PROSE AND NOTHING CHECKS IT, WHICH IS WHY IT WAS WRONG BY
// THREE. It read 34 while the contract carried 37 — the RPCs arrived, the test
// went on enforcing the property in both directions, and only the sentence
// drifted. AGENTS.md's "a comment nothing tests will drift" with a number in it.
// The test below is the thing that matters; this line is a reader's orientation.
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
  { path: "/", file: "page.tsx", reads: [] },
  { path: "/signin", file: "signin/page.tsx", reads: [] },
  { path: "/books", file: "books/page.tsx", reads: ["listBooks"] },
  { path: "(layout)/books", file: "books/layout.tsx", reads: ["listBooks", "listFunds"] },
  { path: "/books/new", file: "books/new/page.tsx", reads: ["createBook"] },
  { path: "/projects", file: "projects/page.tsx", reads: ["listBooks"] },
  {
    path: "(layout)/projects",
    file: "projects/layout.tsx",
    reads: ["listBooks", "listFunds"],
  },
  {
    path: "/books/[book]",
    file: "books/[book]/page.tsx",
    reads: ["getBook", "getView"],
  },
  {
    path: "(layout)/books/[book]",
    file: "books/[book]/layout.tsx",
    reads: ["getBook", "listViews"],
  },
  {
    path: "/books/[book]/views/[view]",
    file: "books/[book]/views/[view]/page.tsx",
    reads: ["getView"],
  },
  { path: "/funds", file: "funds/page.tsx", reads: ["listFunds"] },
  // The rail. Every fund-scoped page renders inside it.
  { path: "(layout)/funds", file: "funds/layout.tsx", reads: ["listFunds"] },
  // One GetFund for the header and the four stat tiles, so no child repeats it.
  {
    path: "(layout)/funds/[fund]",
    file: "funds/[fund]/layout.tsx",
    reads: ["getFund", "listViews"],
  },
  // The view layer. ⛔ A LITERAL `views` SEGMENT, not `/books/[book]/[view]`.
  // Next resolves static segments before dynamic ones, so a view a book happens
  // to name `config` or `rules` would silently shadow that screen — and it
  // makes the URL character-for-character the resource name, which is what this
  // file exists to argue for.
  {
    path: "(layout)/books/[book]/views/[view]",
    file: "books/[book]/views/[view]/layout.tsx",
    reads: ["getView"],
  },
  // ⚠ Where the view-dependent screens USED to live without a view segment.
  // A redirect, not a deletion: these URLs have been sent to people, and the
  // whole argument for this console is that a figure can be sent rather than
  // described. Old `/funds/{fund}/…` job URLs redirect here via next.config.
  {
    path: "/books/[book]/breaks",
    file: "books/[book]/breaks/page.tsx",
    reads: ["getFund"],
  },
  {
    path: "/books/[book]/accounts",
    file: "books/[book]/accounts/page.tsx",
    reads: ["getFund"],
  },
  {
    path: "/books/[book]/positions",
    file: "books/[book]/positions/page.tsx",
    reads: ["getFund"],
  },
  {
    path: "/books/[book]/strikes",
    file: "books/[book]/strikes/page.tsx",
    reads: ["getFund"],
  },
  // What two books of record over one journal disagree about, entry by entry.
  {
    path: "/books/[book]/views/[view]/reconcile",
    file: "books/[book]/views/[view]/reconcile/page.tsx",
    reads: ["reconcileViews"],
  },
  {
    path: "/funds/[fund]",
    file: "funds/[fund]/page.tsx",
    reads: ["getFund", "getView", "listChangeLogEntries"],
  },

  // ── Exceptions ───────────────────────────────────────────────────────────
  {
    path: "/books/[book]/views/[view]/breaks",
    file: "books/[book]/views/[view]/breaks/page.tsx",
    reads: ["listBreaks"],
  },
  {
    path: "/books/[book]/views/[view]/breaks/[break]",
    file: "books/[book]/views/[view]/breaks/[break]/page.tsx",
    reads: ["getBreak"],
  },

  // ── The chart ────────────────────────────────────────────────────────────
  {
    path: "/books/[book]/views/[view]/accounts",
    file: "books/[book]/views/[view]/accounts/page.tsx",
    reads: ["listAccounts"],
  },
  {
    path: "/books/[book]/views/[view]/accounts/[account]",
    file: "books/[book]/views/[view]/accounts/[account]/page.tsx",
    reads: ["getAccount", "listPostings"],
  },
  {
    path: "/books/[book]/views/[view]/accounts/[account]/postings/[posting]",
    file: "books/[book]/views/[view]/accounts/[account]/postings/[posting]/page.tsx",
    reads: ["getPosting"],
  },

  // ── Positions ────────────────────────────────────────────────────────────
  {
    path: "/books/[book]/views/[view]/positions",
    file: "books/[book]/views/[view]/positions/page.tsx",
    reads: ["listPositions"],
  },
  {
    path: "/books/[book]/views/[view]/positions/[position]",
    file: "books/[book]/views/[view]/positions/[position]/page.tsx",
    reads: ["getPosition", "listLots"],
  },
  {
    path: "/books/[book]/views/[view]/positions/[position]/lots/[lot]",
    file: "books/[book]/views/[view]/positions/[position]/lots/[lot]/page.tsx",
    reads: ["getLot"],
  },

  // ── NAV ──────────────────────────────────────────────────────────────────
  {
    path: "/books/[book]/views/[view]/strikes",
    file: "books/[book]/views/[view]/strikes/page.tsx",
    reads: ["listNavStrikes"],
  },
  {
    path: "/books/[book]/views/[view]/strikes/[strike]",
    file: "books/[book]/views/[view]/strikes/[strike]/page.tsx",
    reads: ["getNavStrike"],
  },
  // A URL for a proof. The replay folds the prefix the strike pinned and says
  // whether the history is intact — that is a citation, not a page state.
  {
    path: "/books/[book]/views/[view]/strikes/[strike]/replay",
    file: "books/[book]/views/[view]/strikes/[strike]/replay/page.tsx",
    reads: ["getNavStrike", "replayNavStrike"],
  },
  // A URL for a derivation. What the strike DID, step by step, beside what the
  // same question costs off the maintained totals — and what the plans not
  // taken would have cost.
  //
  // ⛔ NOT A PLAN THE ENGINE CHOSE, and the screen says so in as many words.
  // Nothing in Ratio selects between the two paths; a caller picks one by
  // calling it. Serving a diagram that implied otherwise would be a picture of
  // a structure nothing produces.
  {
    path: "/books/[book]/views/[view]/strikes/[strike]/plan",
    file: "books/[book]/views/[view]/strikes/[strike]/plan/page.tsx",
    reads: ["getNavStrike", "explainNavStrike"],
  },

  // ── Configuration ────────────────────────────────────────────────────────
  {
    path: "/books/[book]/config",
    file: "books/[book]/config/page.tsx",
    reads: ["listConfigVersions"],
  },
  {
    path: "/books/[book]/config/[version]",
    file: "books/[book]/config/[version]/page.tsx",
    reads: ["getConfigVersion"],
  },
  {
    path: "/books/[book]/config/[version]/diff",
    file: "books/[book]/config/[version]/diff/page.tsx",
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
    path: "/books/[book]/rules",
    file: "books/[book]/rules/page.tsx",
    reads: ["listRules", "listChangeLogEntries"],
  },
  {
    path: "/books/[book]/rules/[rule]",
    file: "books/[book]/rules/[rule]/page.tsx",
    reads: ["getRule"],
  },

  // ── The data plane ───────────────────────────────────────────────────────
  {
    path: "/books/[book]/data",
    file: "books/[book]/data/page.tsx",
    reads: ["listDeliveries", "listPendingFacts"],
  },
  {
    path: "/books/[book]/data/deliveries/[delivery]",
    file: "books/[book]/data/deliveries/[delivery]/page.tsx",
    reads: ["getDelivery"],
  },
  {
    path: "/books/[book]/data/pending/[fact]",
    file: "books/[book]/data/pending/[fact]/page.tsx",
    reads: ["getPendingFact"],
  },
  {
    path: "/books/[book]/data/templates",
    file: "books/[book]/data/templates/page.tsx",
    reads: ["getBook", "listTemplates"],
  },
  {
    path: "/books/[book]/data/templates/[template]",
    file: "books/[book]/data/templates/[template]/page.tsx",
    reads: ["getTemplate"],
  },

  // ── Corporate actions ────────────────────────────────────────────────────
  {
    path: "/books/[book]/actions",
    file: "books/[book]/actions/page.tsx",
    reads: ["listCorporateActions"],
  },
  {
    path: "/books/[book]/actions/[action]",
    file: "books/[book]/actions/[action]/page.tsx",
    reads: ["getCorporateAction"],
  },

  // ── The change log ───────────────────────────────────────────────────────
  {
    path: "/books/[book]/changes",
    file: "books/[book]/changes/page.tsx",
    reads: ["listChangeLogEntries"],
  },
  {
    path: "/books/[book]/changes/[entry]",
    file: "books/[book]/changes/[entry]/page.tsx",
    reads: ["getChangeLogEntry"],
  },

  // ── The four writes ──────────────────────────────────────────────────────
  //
  // ⚠ AND ONE OF THEM TWICE. `/record` is `ApplyEvent` with the contract's own
  // shape — a rule and an amount — and `/trade` is the same RPC asked for in
  // the terms a trade actually happens in: an instrument, units, a price and a
  // day, with the consideration derived. Two screens over one method is a
  // deliberate trade: the primitive stays reachable for the kinds of event that
  // have no better form, and the one an operator does daily gets a workflow.
  {
    path: "/books/[book]/record",
    file: "books/[book]/record/page.tsx",
    reads: ["listRules", "applyEvent"],
  },
  // ⚠ AT THE CEILING, AND `?view=` IS WHY IT FITS. The holdings panel needs to
  // know which book its units and carrying values were read in; taking that from
  // the query rather than from the fund's default saves the `getFund` that would
  // make this four.
  {
    path: "/books/[book]/trade",
    file: "books/[book]/trade/page.tsx",
    reads: ["listRules", "listPositions", "applyEvent"],
  },
  {
    path: "/books/[book]/ingest",
    file: "books/[book]/ingest/page.tsx",
    reads: ["listTemplates", "ingestDelivery", "admitFacts"],
  },
  {
    path: "/books/[book]/mark",
    file: "books/[book]/mark/page.tsx",
    reads: ["getFund", "listPositions", "markPositions"],
  },
];
