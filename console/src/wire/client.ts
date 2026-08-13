import "server-only";

// The console's one route to the API, and the only place a bearer is attached.
//
// ⛔ SERVER ONLY, AND THAT IS THE WHOLE ARCHITECTURE. The browser never calls
// AWS: it calls this app, this app calls the API, and the token lives in an
// httpOnly cookie the page cannot read. Three things follow, and each is worth
// keeping:
//
//   * There is no CORS on this path. A server-to-server `fetch` sends no
//     `Origin`, so the gateway's `CorsConfiguration` is never consulted. The
//     ABSENCE of `authorization` from `AllowHeaders` in `deploy/app.yaml` is
//     therefore load-bearing — it is what makes a browser-direct call
//     impossible, and it should stay absent.
//   * An XSS on this origin cannot read the token, because `document.cookie`
//     cannot.
//   * A server component can await a figure, which is the reason this app is
//     server-rendered at all.
//
// ⛔ EVERY TEMPLATE BELOW IS THE `google.api.http` RULE, VERBATIM.
// `//console:route_manifest_test` reads them out of `console.proto` and asserts
// this file names exactly those and no others — the same discipline as
// `crates/ratio-console/src/transcode.rs`'s `ROUTES`, which is checked against
// the same contract from the other side. Hand-written and unchecked would be a
// 404 found by a customer; hand-written and checked is a failing build.

import type {
  Account,
  AdmitFactsRequest,
  AdmitFactsResponse,
  ApplyEventRequest,
  ApplyEventResponse,
  Break,
  ChangeLogEntry,
  ConfigVersion,
  CorporateAction,
  Delivery,
  DiffConfigVersionsResponse,
  Fund,
  IngestDeliveryRequest,
  IngestDeliveryResponse,
  ListAccountsResponse,
  ListBreaksResponse,
  ListChangeLogEntriesResponse,
  ListConfigVersionsResponse,
  ListCorporateActionsResponse,
  ListDeliveriesResponse,
  ListFundsResponse,
  ListLotsResponse,
  ListNavStrikesResponse,
  ListPendingFactsResponse,
  ListPositionsResponse,
  ListPostingsResponse,
  ListRulesResponse,
  ListTemplatesResponse,
  Lot,
  MarkPositionsRequest,
  MarkPositionsResponse,
  NavStrike,
  PendingFact,
  Position,
  Posting,
  ReplayNavStrikeResponse,
  Rule,
  Template,
} from "./types.js";

/**
 * The server refused for lack of a verified session.
 *
 * ⛔ 401 IS A REAL ANSWER, NOT A TRANSPORT FAILURE. `RATIO_AUTH=required` makes
 * the API fail closed: a `/v1` request the gateway attached no verified identity
 * to is refused even if the authorizer were removed. A page catches this and
 * redirects to sign-in rather than rendering an error row.
 */
export class AuthError extends Error {
  constructor() {
    super("sign in required");
    this.name = "AuthError";
  }
}

/** No such resource — or one this subject may not see. */
export class NotFound extends Error {
  constructor(detail: string) {
    super(detail);
    this.name = "NotFound";
  }
}

/**
 * The server declined, and said why in a sentence.
 *
 * ⛔ THE SENTENCE IS THE POINT. `ratio-console` puts prose in `error`, and
 * surfacing it beats surfacing a status code — the whole claim of this product
 * is that a figure can be accounted for, and that has to include the figure not
 * being there. A caller renders `.message`, never `.status`.
 */
export class Refused extends Error {
  readonly status: number;
  constructor(status: number, detail: string) {
    super(detail);
    this.name = "Refused";
    this.status = status;
  }
}

/** Where the API lives. No default: a console pointed at nothing should say so
 *  at the first request rather than quietly render empty screens. */
function origin(): string {
  const o = process.env.RATIO_API_ORIGIN;
  if (!o) throw new Error("RATIO_API_ORIGIN is not set");
  return o.replace(/\/+$/, "");
}

/** How a caller supplies the verified identity for one request. */
export interface Caller {
  /** The Cognito **id** token, or null on a local run with no identity provider.
   *
   * ⛔ THE ID TOKEN, NOT THE ACCESS TOKEN, AND THE REFLEX IS TO GET THIS WRONG.
   * The gateway's JWT authorizer accepts either — an id token's `aud` is the
   * client id, which is what `Audience` validates — but only the id token
   * carries the `email` claim the tenant boundary matches on at
   * `Console::book_path`. A Cognito access token has `sub` and `client_id` and
   * NO email, so every signed-in person would land on an empty fund rail.
   *
   * ⚠ AND THE DEMO WOULD NOT SHOW IT. `RATIO_DEMO_OPEN=1` grants any
   * authenticated caller every fund, so this mistake is invisible in production
   * and surfaces the day tenancy is switched on for a real customer. */
  idToken: string | null;
}

async function send<T>(
  caller: Caller,
  path: string,
  body?: unknown,
): Promise<T> {
  const headers: Record<string, string> = { accept: "application/json" };
  if (body !== undefined) headers["content-type"] = "application/json";
  if (caller.idToken) headers.authorization = `Bearer ${caller.idToken}`;

  const r = await fetch(`${origin()}/v1${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    // ⛔ NEVER CACHED. The API already says `Cache-Control: no-store` on every
    // response, but Next's defaults have moved between majors and a NAV served
    // from a build-time cache is the worst failure available to this product.
    // Say it here rather than inherit it.
    cache: "no-store",
  });

  if (r.status === 401) throw new AuthError();
  if (!r.ok) {
    let detail = `${r.status}`;
    try {
      const e = (await r.json()) as { error?: string };
      if (e.error) detail = e.error;
    } catch {
      /* a non-JSON error body is still an error */
    }
    if (r.status === 404) throw new NotFound(detail);
    throw new Refused(r.status, detail);
  }
  return (await r.json()) as T;
}

/** `?filter=…`, or nothing. Kept here so no call site hand-rolls encoding. */
function q(params: Record<string, string | undefined>): string {
  const p = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) if (v) p.set(k, v);
  const s = p.toString();
  return s ? `?${s}` : "";
}

// ── Funds ──────────────────────────────────────────────────────────────────
// GET /v1/funds
export const listFunds = (c: Caller) =>
  send<ListFundsResponse>(c, `/funds`);
// GET /v1/{name=funds/*}
export const getFund = (c: Caller, fund: string) =>
  send<Fund>(c, `/funds/${fund}`);

// ── Breaks ─────────────────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/breaks
export const listBreaks = (c: Caller, fund: string, filter?: string) =>
  send<ListBreaksResponse>(c, `/funds/${fund}/breaks${q({ filter })}`);
// GET /v1/{name=funds/*/breaks/*}
export const getBreak = (c: Caller, fund: string, id: string) =>
  send<Break>(c, `/funds/${fund}/breaks/${id}`);

// ── Configuration ──────────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/configVersions
export const listConfigVersions = (c: Caller, fund: string) =>
  send<ListConfigVersionsResponse>(c, `/funds/${fund}/configVersions`);
// GET /v1/{name=funds/*/configVersions/*}
export const getConfigVersion = (c: Caller, fund: string, id: string) =>
  send<ConfigVersion>(c, `/funds/${fund}/configVersions/${id}`);
// GET /v1/{name=funds/*/configVersions/*}:diff
export const diffConfigVersions = (
  c: Caller,
  fund: string,
  id: string,
  base?: string,
) =>
  send<DiffConfigVersionsResponse>(
    c,
    `/funds/${fund}/configVersions/${id}:diff${q({ base })}`,
  );

// ── Rules and templates ────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/rules
export const listRules = (c: Caller, fund: string) =>
  send<ListRulesResponse>(c, `/funds/${fund}/rules`);
// GET /v1/{name=funds/*/rules/*}
export const getRule = (c: Caller, fund: string, id: string) =>
  send<Rule>(c, `/funds/${fund}/rules/${id}`);
// GET /v1/{parent=funds/*}/templates
export const listTemplates = (c: Caller, fund: string) =>
  send<ListTemplatesResponse>(c, `/funds/${fund}/templates`);
// GET /v1/{name=funds/*/templates/*}
export const getTemplate = (c: Caller, fund: string, id: string) =>
  send<Template>(c, `/funds/${fund}/templates/${id}`);

// ── The data plane ─────────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/deliveries
export const listDeliveries = (c: Caller, fund: string) =>
  send<ListDeliveriesResponse>(c, `/funds/${fund}/deliveries`);
// GET /v1/{name=funds/*/deliveries/*}
export const getDelivery = (c: Caller, fund: string, id: string) =>
  send<Delivery>(c, `/funds/${fund}/deliveries/${id}`);
// GET /v1/{parent=funds/*}/pendingFacts
export const listPendingFacts = (c: Caller, fund: string) =>
  send<ListPendingFactsResponse>(c, `/funds/${fund}/pendingFacts`);
// GET /v1/{name=funds/*/pendingFacts/*}
export const getPendingFact = (c: Caller, fund: string, id: string) =>
  send<PendingFact>(c, `/funds/${fund}/pendingFacts/${id}`);

// ── Positions and lots ─────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/positions
export const listPositions = (c: Caller, fund: string) =>
  send<ListPositionsResponse>(c, `/funds/${fund}/positions`);
// GET /v1/{name=funds/*/positions/*}
export const getPosition = (c: Caller, fund: string, id: string) =>
  send<Position>(c, `/funds/${fund}/positions/${id}`);
/**
 * The open tax lots behind one position.
 *
 * ⛔ UNBOUNDED, AND THE URL MADE IT WORSE. `nextPageToken` is declared on every
 * list response and left empty by every implementation, so this reads the whole
 * lot book. The SPA guarded it by only firing when a position was expanded;
 * giving a position its own URL makes the read unconditional on page load.
 * Callers MUST put this behind a `<Suspense>` child segment so the page shell
 * renders without waiting for it — see `positions/[position]/page.tsx`.
 */
// GET /v1/{parent=funds/*/positions/*}/lots
export const listLots = (c: Caller, fund: string, position: string) =>
  send<ListLotsResponse>(c, `/funds/${fund}/positions/${position}/lots`);
// GET /v1/{name=funds/*/positions/*/lots/*}
export const getLot = (c: Caller, fund: string, position: string, id: string) =>
  send<Lot>(c, `/funds/${fund}/positions/${position}/lots/${id}`);

// ── The chart ──────────────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/accounts
export const listAccounts = (c: Caller, fund: string, filter?: string) =>
  send<ListAccountsResponse>(c, `/funds/${fund}/accounts${q({ filter })}`);
// GET /v1/{name=funds/*/accounts/*}
export const getAccount = (c: Caller, fund: string, id: string) =>
  send<Account>(c, `/funds/${fund}/accounts/${id}`);
/** The lines behind one figure. ⛔ Unbounded — see `listLots`. */
// GET /v1/{parent=funds/*/accounts/*}/postings
export const listPostings = (c: Caller, fund: string, account: string) =>
  send<ListPostingsResponse>(c, `/funds/${fund}/accounts/${account}/postings`);
// GET /v1/{name=funds/*/accounts/*/postings/*}
export const getPosting = (
  c: Caller,
  fund: string,
  account: string,
  id: string,
) => send<Posting>(c, `/funds/${fund}/accounts/${account}/postings/${id}`);

// ── NAV ────────────────────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/navStrikes
export const listNavStrikes = (c: Caller, fund: string) =>
  send<ListNavStrikesResponse>(c, `/funds/${fund}/navStrikes`);
// GET /v1/{name=funds/*/navStrikes/*}
export const getNavStrike = (c: Caller, fund: string, id: string) =>
  send<NavStrike>(c, `/funds/${fund}/navStrikes/${id}`);
/** Re-derive a strike from the journal prefix it pinned. A proof does not go
 *  stale: the same prefix always folds to the same answer. */
// GET /v1/{name=funds/*/navStrikes/*}:replay
export const replayNavStrike = (c: Caller, fund: string, id: string) =>
  send<ReplayNavStrikeResponse>(c, `/funds/${fund}/navStrikes/${id}:replay`);

// ── Corporate actions ──────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/corporateActions
export const listCorporateActions = (c: Caller, fund: string) =>
  send<ListCorporateActionsResponse>(c, `/funds/${fund}/corporateActions`);
// GET /v1/{name=funds/*/corporateActions/*}
export const getCorporateAction = (c: Caller, fund: string, id: string) =>
  send<CorporateAction>(c, `/funds/${fund}/corporateActions/${id}`);

// ── The change log ─────────────────────────────────────────────────────────
// GET /v1/{parent=funds/*}/changeLogEntries
export const listChangeLogEntries = (c: Caller, fund: string) =>
  send<ListChangeLogEntriesResponse>(c, `/funds/${fund}/changeLogEntries`);
// GET /v1/{name=funds/*/changeLogEntries/*}
export const getChangeLogEntry = (c: Caller, fund: string, id: string) =>
  send<ChangeLogEntry>(c, `/funds/${fund}/changeLogEntries/${id}`);

// ── The four writes ────────────────────────────────────────────────────────
//
// ⛔ EVERY ONE TAKES `validateOnly`, AND THAT IS NOT A CONVENIENCE. A preview
// runs the identical code path and records nothing, so what a screen shows is
// what lands. A caller that offers "post" without offering "preview first" has
// removed the property these four were shaped around.

// POST /v1/{parent=funds/*}:applyEvent
export const applyEvent = (c: Caller, fund: string, req: ApplyEventRequest) =>
  send<ApplyEventResponse>(c, `/funds/${fund}:applyEvent`, req);
// POST /v1/{parent=funds/*}:ingest
export const ingestDelivery = (
  c: Caller,
  fund: string,
  req: IngestDeliveryRequest,
) => send<IngestDeliveryResponse>(c, `/funds/${fund}:ingest`, req);
// POST /v1/{parent=funds/*}:admit
export const admitFacts = (c: Caller, fund: string, req: AdmitFactsRequest) =>
  send<AdmitFactsResponse>(c, `/funds/${fund}:admit`, req);
// POST /v1/{parent=funds/*}:mark
export const markPositions = (
  c: Caller,
  fund: string,
  req: MarkPositionsRequest,
) => send<MarkPositionsResponse>(c, `/funds/${fund}:mark`, req);
