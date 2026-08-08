// The console's data layer: TanStack Query over the transcoded gRPC API.
//
// Every hook maps to one `google.api.http` rule on `ratio.v1.Console`. The
// query keys are the RESOURCE NAMES the API itself uses — `["funds/demo",
// "breaks"]` rather than an invented cache-key scheme — so a key is always
// something the server would recognize, and invalidating "everything under
// this fund" is a prefix match rather than a convention to remember.

import {
  useMutation,
  useQueryClient,
  useQuery,
  type UseMutationResult,
  type UseQueryResult,
} from "@tanstack/react-query";
import type {
  Account,
  AdmitFactsRequest,
  AdmitFactsResponse,
  IngestDeliveryRequest,
  ListPositionsResponse,
  ListTemplatesResponse,
  Position,
  Template,
  IngestDeliveryResponse,
  Delivery,
  ListDeliveriesResponse,
  ListPendingFactsResponse,
  PendingFact,
  ApplyEventRequest,
  ApplyEventResponse,
  ListRulesResponse,
  Rule,
  Break,
  ChangeLogEntry,
  ConfigVersion,
  DiffConfigVersionsResponse,
  ListConfigVersionsResponse,
  Fund,
  NavStrike,
  ReplayNavStrikeResponse,
  ListNavStrikesResponse,
  ListBreaksResponse,
  ListChangeLogEntriesResponse,
  ListAccountsResponse,
  ListFundsResponse,
  ListPostingsResponse,
  Posting,
} from "./types.js";

/** Where the API lives, relative to wherever the console is served from. */
const BASE = "../v1";

async function get<T>(path: string): Promise<T> {
  return send<T>(path, undefined);
}

/// One request. `body` present means POST — the console API is GET everywhere
/// except `:applyEvent`, so the method follows from whether there is a body
/// rather than being passed around.
async function send<T>(path: string, body: unknown): Promise<T> {
  const r = await fetch(`${BASE}${path}`, {
    method: body === undefined ? "GET" : "POST",
    headers: body === undefined
      ? { accept: "application/json" }
      : { accept: "application/json", "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!r.ok) {
    // The BFF puts a sentence in `error`. Surfacing it beats "500" — the whole
    // point of the thing is that a figure can be accounted for, and that has
    // to include the figure not being there.
    let detail = `${r.status}`;
    try {
      const body = (await r.json()) as { error?: string };
      if (body.error) detail = body.error;
    } catch {
      /* a non-JSON error body is still an error */
    }
    throw new Error(detail);
  }
  return (await r.json()) as T;
}

/** How long a NAV-day figure stays fresh before it is refetched. */
const LIVE = {
  // A fund's state changes when someone posts or approves, which is rare but
  // consequential — so it is refetched on focus rather than polled.
  staleTime: 10_000,
  refetchOnWindowFocus: true,
} as const;

export function useFunds(): UseQueryResult<Fund[], Error> {
  return useQuery({
    queryKey: ["funds"],
    queryFn: () => get<ListFundsResponse>("/funds"),
    select: (r) => r.funds,
    ...LIVE,
  });
}

export function useFund(name: string | undefined): UseQueryResult<Fund, Error> {
  return useQuery({
    queryKey: [name ?? "", "self"],
    queryFn: () => get<Fund>(`/${name}`),
    enabled: !!name,
    ...LIVE,
  });
}

export function useBreaks(
  fund: string | undefined,
  filter: string,
): UseQueryResult<Break[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "breaks", filter],
    queryFn: () =>
      get<ListBreaksResponse>(
        `/${fund}/breaks${filter ? `?filter=${encodeURIComponent(filter)}` : ""}`,
      ),
    select: (r) => r.breaks,
    enabled: !!fund,
    // `placeholderData` keeps the previous list on screen while a filter
    // change loads. Without it every chip click blanks the queue, which reads
    // as "there is nothing here" for as long as the round trip takes.
    placeholderData: (prev) => prev,
    ...LIVE,
  });
}

export function useNavStrikes(
  fund: string | undefined,
): UseQueryResult<NavStrike[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "navStrikes"],
    queryFn: () => get<ListNavStrikesResponse>(`/${fund}/navStrikes`),
    select: (r) => r.navStrikes,
    enabled: !!fund,
    ...LIVE,
  });
}

/// Re-derive a strike. Disabled until asked: a replay is something an operator
/// DOES, and a page that quietly replayed every strike on load would be
/// asserting the proof rather than offering it.
export function useReplay(
  strike: string | undefined,
  enabled: boolean,
): UseQueryResult<ReplayNavStrikeResponse, Error> {
  return useQuery({
    queryKey: [strike ?? "", "replay"],
    queryFn: () => get<ReplayNavStrikeResponse>(`/${strike}:replay`),
    enabled: !!strike && enabled,
    // A proof does not go stale. Re-deriving the same prefix gives the same
    // answer, so refetching on focus would be pure noise.
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });
}

export function useChangeLog(
  fund: string | undefined,
): UseQueryResult<ChangeLogEntry[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "changeLogEntries"],
    queryFn: () => get<ListChangeLogEntriesResponse>(`/${fund}/changeLogEntries`),
    select: (r) => r.changeLogEntries,
    enabled: !!fund,
    ...LIVE,
  });
}

export function useConfigVersions(
  fund: string | undefined,
): UseQueryResult<ConfigVersion[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "configVersions"],
    queryFn: () => get<ListConfigVersionsResponse>(`/${fund}/configVersions`),
    // Newest first is how the server sends it and how the panel reads it.
    select: (r) => r.configVersions,
    enabled: !!fund,
    ...LIVE,
  });
}

/// What changed between two configurations. Like a replay, this is a derived
/// answer about the past: the same two digests always diff the same way, so it
/// never goes stale.
export function useConfigDiff(
  version: string | undefined,
  base: string,
): UseQueryResult<DiffConfigVersionsResponse, Error> {
  return useQuery({
    queryKey: [version ?? "", "diff", base],
    queryFn: () =>
      get<DiffConfigVersionsResponse>(
        `/${version}:diff${base ? `?base=${encodeURIComponent(base)}` : ""}`,
      ),
    enabled: !!version,
    staleTime: Infinity,
    refetchOnWindowFocus: false,
  });
}

export function useAccounts(
  fund: string | undefined,
  filter: string,
): UseQueryResult<Account[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "accounts", filter],
    queryFn: () =>
      get<ListAccountsResponse>(
        `/${fund}/accounts${filter ? `?filter=${encodeURIComponent(filter)}` : ""}`,
      ),
    select: (r) => r.accounts,
    enabled: !!fund,
    placeholderData: (prev) => prev,
    ...LIVE,
  });
}

/// The lines behind one figure. Fetched when an account is opened rather than
/// with the trial balance: a book has one row per account and many postings
/// per row, and loading every line to show a column of totals would be the
/// wrong trade in exactly the case that matters — a big book.
export function usePostings(
  account: string | undefined,
): UseQueryResult<Posting[], Error> {
  return useQuery({
    queryKey: [account ?? "", "postings"],
    queryFn: () => get<ListPostingsResponse>(`/${account}/postings`),
    select: (r) => r.postings,
    enabled: !!account,
    ...LIVE,
  });
}

export function useRules(
  fund: string | undefined,
): UseQueryResult<Rule[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "rules"],
    queryFn: () => get<ListRulesResponse>(`/${fund}/rules`),
    select: (r) => r.rules,
    enabled: !!fund,
    ...LIVE,
  });
}

/**
 * Record an event.
 *
 * The only mutation in the console. On a real commit every figure under this
 * fund is stale — the NAV, the trial balance, the strikes, the change log — so
 * the whole prefix is invalidated rather than a list of the queries someone
 * remembered. A preview invalidates nothing, because it changed nothing.
 */
export function useApplyEvent(
  fund: string | undefined,
): UseMutationResult<ApplyEventResponse, Error, ApplyEventRequest> {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: ApplyEventRequest) =>
      send<ApplyEventResponse>(`/${fund}:applyEvent`, req),
    onSuccess: (res) => {
      if (res.validateOnly) return;
      qc.invalidateQueries({ queryKey: [fund ?? ""] });
      qc.invalidateQueries({ queryKey: ["funds"] });
    },
  });
}

export function useDeliveries(
  fund: string | undefined,
): UseQueryResult<Delivery[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "deliveries"],
    queryFn: () => get<ListDeliveriesResponse>(`/${fund}/deliveries`),
    select: (r) => r.deliveries,
    enabled: !!fund,
    ...LIVE,
  });
}

/// Facts that cannot post yet. Recomputed server-side on every call — a fact
/// clears when the master changes, not when something re-reads the file, so
/// there is nothing here to invalidate on.
export function usePendingFacts(
  fund: string | undefined,
): UseQueryResult<PendingFact[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "pendingFacts"],
    queryFn: () => get<ListPendingFactsResponse>(`/${fund}/pendingFacts`),
    select: (r) => r.pendingFacts,
    enabled: !!fund,
    ...LIVE,
  });
}

/**
 * Read a file into facts.
 *
 * `validateOnly` runs the same code path and records nothing, so what a
 * preview shows is what lands. A real read invalidates everything under the
 * fund: the deliveries, the pending queue, and the fund's own blocking state.
 */
export function useIngest(
  fund: string | undefined,
): UseMutationResult<IngestDeliveryResponse, Error, IngestDeliveryRequest> {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: IngestDeliveryRequest) =>
      send<IngestDeliveryResponse>(`/${fund}:ingest`, req),
    onSuccess: (res) => {
      if (res.validateOnly) return;
      qc.invalidateQueries({ queryKey: [fund ?? ""] });
      qc.invalidateQueries({ queryKey: ["funds"] });
    },
  });
}

/** Post every fact that fully resolves. */
export function useAdmit(
  fund: string | undefined,
): UseMutationResult<AdmitFactsResponse, Error, AdmitFactsRequest> {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (req: AdmitFactsRequest) =>
      send<AdmitFactsResponse>(`/${fund}:admit`, req),
    onSuccess: (res) => {
      if (res.validateOnly) return;
      qc.invalidateQueries({ queryKey: [fund ?? ""] });
      qc.invalidateQueries({ queryKey: ["funds"] });
    },
  });
}

export function useTemplates(
  fund: string | undefined,
): UseQueryResult<Template[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "templates"],
    queryFn: () => get<ListTemplatesResponse>(`/${fund}/templates`),
    select: (r) => r.templates,
    enabled: !!fund,
    ...LIVE,
  });
}

export function usePositions(
  fund: string | undefined,
): UseQueryResult<Position[], Error> {
  return useQuery({
    queryKey: [fund ?? "", "positions"],
    queryFn: () => get<ListPositionsResponse>(`/${fund}/positions`),
    select: (r) => r.positions,
    enabled: !!fund,
    ...LIVE,
  });
}
