import "server-only";

/**
 * The API's cold-start 503, and how long to treat it as transient.
 *
 * ⛔ THIS IS NOT A MISSING SESSION AND NOT A FIGURE. After the JWT
 * authorizer accepted the bearer (#255), the first `/v1/books` a signed-in
 * operator made hit a cold Lambda. Book routes wait briefly then answer
 * 503 `{"error":"the journal is still hydrating"}` with Retry-After: 2
 * so `/healthz` is never starved. Deploy smoke already retries that body
 * up to 32s. The console painted the first 503 as a dead-end ("The API
 * is temporarily unavailable" / "Try again") while `/balance.json` on
 * the same function was already 200. Empty membership is 200 `[]`, not
 * this sentence.
 */

/** The API's hydrate refuse, verbatim from `watch.rs`. */
export const HYDRATING = "the journal is still hydrating";

/**
 * First try plus waits. Deploy smoke uses 16 × 2s; the RSC caps lower
 * so a stuck hydrate cannot hold the Vercel function until it times out
 * into `#441`. Exhausted retries still surface as Unavailable.
 */
export const HYDRATE_RETRY_LIMIT = 10;

export function isHydratingRefuse(status: number, error: string | undefined): boolean {
  return status === 503 && typeof error === "string" && error.includes("hydrating");
}

/** Retry-After is seconds. Absent or unparseable is the API's 2. Cap 5. */
export function hydrateWaitMs(retryAfter: string | null): number {
  const seconds = Number.parseInt(retryAfter ?? "", 10);
  if (Number.isFinite(seconds) && seconds > 0) {
    return Math.min(seconds, 5) * 1000;
  }
  return 2000;
}

export type FetchUntilReady = {
  fetch: typeof fetch;
  wait: (ms: number) => Promise<void>;
  random?: () => number;
};

const defaultWait = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Replay a fetch while the journal is still hydrating or API Gateway says
 * that no Lambda concurrency was available.
 *
 * ⭐ ONLY THE TWO OBSERVED TRANSIENT ANSWERS ARE RETRIED. A rolling 500,
 * an application 503, and a 401 are answers. Cloning the response keeps
 * `send()` able to read the last body.
 */
export async function fetchUntilReady(
  url: string,
  init: RequestInit,
  deps: FetchUntilReady = { fetch: globalThis.fetch.bind(globalThis), wait: defaultWait },
): Promise<Response> {
  let last: Response | undefined;
  for (let attempt = 0; attempt < HYDRATE_RETRY_LIMIT; attempt++) {
    last = await deps.fetch(url, init);
    const body = await peekError(last);
    const hydrating = isHydratingRefuse(last.status, body.error);
    const saturated = last.status === 503 && body.message === "Service Unavailable";
    if (!hydrating && !saturated) {
      return last;
    }
    if (attempt === HYDRATE_RETRY_LIMIT - 1) {
      return last;
    }
    const wait = hydrating
      ? hydrateWaitMs(last.headers.get("retry-after"))
      : gatewayWaitMs(attempt, deps.random ?? Math.random);
    await deps.wait(wait);
  }
  return last!;
}

/** Jitter prevents every saturated render from retrying on the same boundary. */
export function gatewayWaitMs(attempt: number, random: () => number = Math.random): number {
  const ceiling = Math.min(250 * 2 ** attempt, 2000);
  return Math.max(1, Math.ceil(ceiling * random()));
}

async function peekError(r: Response): Promise<{ error?: string; message?: string }> {
  try {
    return (await r.clone().json()) as { error?: string; message?: string };
  } catch {
    return {};
  }
}
