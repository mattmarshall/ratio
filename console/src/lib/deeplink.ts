// A string somebody pasted, turned into a URL on this console — or refused.
//
// ⭐ THE WHOLE MODULE IS A STRING FUNCTION AND MAKES NO CALL. The contract has
// no search RPC, `wire/client.ts` is `server-only`, and `proxy.ts` sets
// `connect-src 'self'` — so nothing here can ask whether a resource exists. What
// it can do is refuse to guess, and say which of the two it is doing.

/**
 * The collection in a resource name → the segment(s) in its URL.
 *
 * ⛔ EIGHT OF THESE DISAGREE, AND THAT IS EXACTLY WHAT DRIFTS. `console.proto`
 * says `navStrikes` and the URL says `strikes`; `changeLogEntries` → `changes`;
 * `configVersions` → `config`; `corporateActions` → `actions`; and the four
 * data-plane collections are all nested under `data/`. Every other screen in this
 * console builds its href from parts it already holds, so this is the only place
 * a whole resource name is translated — and therefore the only place the rename
 * can be wrong.
 *
 * ⭐ `entries` LANDS ON `/books/{book}/entries/{entry}` — the page #52 asked
 * for. The contract name stays `funds/{fund}/entries/{entry}`; the rewrite
 * below is the same one every other book job takes.
 */
export const COLLECTION_TO_SEGMENT: Record<string, string> = {
  books: "books",
  funds: "funds",
  views: "views",
  breaks: "breaks",
  accounts: "accounts",
  postings: "postings",
  positions: "positions",
  lots: "lots",
  rules: "rules",
  entries: "entries",
  navStrikes: "strikes",
  changeLogEntries: "changes",
  configVersions: "config",
  corporateActions: "actions",
  deliveries: "data/deliveries",
  pendingFacts: "data/pending",
  facts: "data/facts",
  templates: "data/templates",
};

/**
 * A full AIP resource name → the console URL for it, or `null`.
 *
 * ⭐ EXACT, AND THE ONLY TIER THAT IS. The string itself says which resource it
 * names, so nothing is inferred — and this is the case that actually happens,
 * because it is what the API returns and what a log line or a message carries.
 *
 * ⚠ Returns `null` rather than a best effort on anything it does not recognise.
 * A wrong URL offered confidently is worse than no URL, and the caller has a
 * second tier for the genuinely ambiguous case.
 */
export function hrefForResourceName(name: string): string | null {
  const trimmed = name.trim().replace(/^\/+/, "").replace(/\/+$/, "");
  if (!trimmed.startsWith("funds/") && !trimmed.startsWith("books/")) return null;

  const parts = trimmed.split("/");
  // Resource names are collection/id pairs all the way down. An odd count is a
  // collection with nothing in it — `funds/f/breaks` names the list, not a break.
  if (parts.length === 0 || parts.length % 2 !== 0) return null;

  const out: string[] = [];
  for (let i = 0; i < parts.length; i += 2) {
    const collection = parts[i];
    const id = parts[i + 1];
    if (!collection || !id) return null;
    const segment = COLLECTION_TO_SEGMENT[collection];
    if (segment === undefined) return null;
    out.push(segment, encodeURIComponent(id));
  }

  const href = `/${out.join("/")}`;
  // A fund resource name with children is a job. Jobs live on the book.
  // `funds/{fund}` alone is the filing page and stays under `/funds`.
  return href.replace(/^\/funds\/([^/]+)\//, "/books/$1/");
}

/** One route a bare id might name. Never a claim that it does. */
export interface Candidate {
  /** Stable across keystrokes for one id, so kbar re-registers rather than churns. */
  readonly key: string;
  readonly label: string;
  readonly href: string;
  /** ⚠ COMMA-separated. kbar splits this field on commas, not spaces. */
  readonly keywords: string;
}

/**
 * Every route a bare id could name, under one fund and one book of record.
 *
 * ⛔ EVERY CANDIDATE, RANKED BY NOTHING. The id namespaces in this contract are
 * not distinguishable and it is not close: a break is `cash-usd-2026-02-26` and a
 * NAV strike is `2026-02-26`, so a date is both; an account is `1010` and so is a
 * lot; four collections use the same slug shape; and the two hex ids differ only
 * in a length nothing declares. A heuristic here would be the console asserting
 * a fact it has not checked — the move this repository refuses everywhere else,
 * where a figure that will not divide is refused rather than rounded.
 *
 * The operator disambiguates by typing a second word: kbar searches with
 * `useTokenSearch` and `tokenMatch: "all"`, so `ACME position` keeps only the
 * candidate carrying both tokens. This narrows. It never decides.
 *
 * ⭐ `lots` AND `postings` ARE ABSENT BECAUSE THEIR URLS TAKE TWO IDS. A lot is
 * addressed under its position and a posting under its account, so a bare id
 * cannot reach either. That is a fact about the route tree, not an omission.
 */
export function candidatesForId(
  book: string,
  view: string,
  id: string,
): readonly Candidate[] {
  const raw = id.trim();
  // ⚠ One character matches everything and helps nobody; an id with a slash in
  // it is a resource name, which `hrefForResourceName` answers exactly.
  if (raw.length < 2 || raw.includes("/")) return [];
  const e = encodeURIComponent(raw);

  const scoped = `/books/${book}/views/${view}`;
  const plain = `/books/${book}`;

  return [
    { key: "breaks", label: `Open “${raw}” as an exception`,
      href: `${scoped}/breaks/${e}`, keywords: "break,exception,queue,difference" },
    { key: "positions", label: `Open “${raw}” as a position`,
      href: `${scoped}/positions/${e}`, keywords: "position,instrument,holding,lots" },
    { key: "accounts", label: `Open “${raw}” as an account`,
      href: `${scoped}/accounts/${e}`, keywords: "account,ledger,trial balance,dimension" },
    { key: "entries", label: `Open “${raw}” as a journal entry`,
      href: `${plain}/entries/${e}`, keywords: "entry,journal,posting,provenance" },
    { key: "strikes", label: `Open “${raw}” as a NAV strike`,
      href: `${scoped}/strikes/${e}`, keywords: "strike,nav,valuation,date" },
    { key: "rules", label: `Open “${raw}” as a rule`,
      href: `${plain}/rules/${e}`, keywords: "rule,posting rule,configuration" },
    { key: "changes", label: `Open “${raw}” as a change-log entry`,
      href: `${plain}/changes/${e}`, keywords: "change,change log,amendment,proposal" },
    { key: "config", label: `Open “${raw}” as a configuration version`,
      href: `${plain}/config/${e}`, keywords: "configuration,version,digest" },
    { key: "actions", label: `Open “${raw}” as a corporate action`,
      href: `${plain}/actions/${e}`, keywords: "corporate action,dividend,split" },
    { key: "deliveries", label: `Open “${raw}” as a delivery`,
      href: `${plain}/data/deliveries/${e}`, keywords: "delivery,data,ingest,file" },
    { key: "pending", label: `Open “${raw}” as a pending fact`,
      href: `${plain}/data/pending/${e}`, keywords: "pending,fact,unadmitted,data" },
    { key: "facts", label: `Open “${raw}” as a fact`,
      href: `${plain}/data/facts/${e}`, keywords: "fact,price,rate,fx,provenance,data" },
    { key: "templates", label: `Open “${raw}” as a template`,
      href: `${plain}/data/templates/${e}`, keywords: "template,mapping,data" },
  ];
}
