import { describe, expect, it, vi } from "vitest";
import {
  fetchUntilReady,
  hydrateWaitMs,
  HYDRATE_RETRY_LIMIT,
  HYDRATING,
  isHydratingRefuse,
} from "./hydrate";

/**
 * After #255 the gateway accepted the session. The first `/v1/books`
 * then hit a cold Lambda and 503'd `the journal is still hydrating`.
 * `/healthz` and `/balance.json` on that same function were already 200 —
 * the journal was not stuck, and empty membership is 200 `[]`. Deploy
 * smoke already retried this body; the console painted the first 503.
 */

function hydrating(retryAfter = "2"): Response {
  return new Response(JSON.stringify({ error: HYDRATING }), {
    status: 503,
    headers: {
      "content-type": "application/json",
      "retry-after": retryAfter,
    },
  });
}

function json(status: number, body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("hydrate 503", () => {
  it("a hydrating 503 is retried until the journal is ready, not painted as a dead-end on the first answer", async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce(hydrating())
      .mockResolvedValueOnce(hydrating())
      .mockResolvedValueOnce(json(200, { books: [] }));
    const wait = vi.fn(async () => {});

    const r = await fetchUntilReady("https://api.example/v1/books", { cache: "no-store" }, {
      fetch,
      wait,
    });
    expect(r.status).toBe(200);
    expect(await r.json()).toEqual({ books: [] });
    expect(fetch).toHaveBeenCalledTimes(3);
    expect(wait).toHaveBeenCalledTimes(2);
    expect(wait).toHaveBeenCalledWith(2000);
  });

  it("a 503 that is not hydrating is not retried as if the journal will become ready", async () => {
    const fetch = vi.fn().mockResolvedValue(json(503, { error: "internal" }));
    const wait = vi.fn(async () => {});
    const r = await fetchUntilReady("https://api.example/v1/books", {}, { fetch, wait });
    expect(r.status).toBe(503);
    expect(await r.json()).toEqual({ error: "internal" });
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(wait).not.toHaveBeenCalled();
  });

  it("a 401 is not labeled as hydrating", () => {
    expect(isHydratingRefuse(401, HYDRATING)).toBe(false);
    expect(isHydratingRefuse(401, "Unauthorized")).toBe(false);
    expect(isHydratingRefuse(503, HYDRATING)).toBe(true);
  });

  it("honors Retry-After seconds, and caps a runaway value", () => {
    expect(hydrateWaitMs("2")).toBe(2000);
    expect(hydrateWaitMs(null)).toBe(2000);
    expect(hydrateWaitMs("90")).toBe(5000);
  });

  it("exhausts the retry budget on a journal that never becomes ready", async () => {
    const fetch = vi.fn().mockResolvedValue(hydrating());
    const wait = vi.fn(async () => {});
    const r = await fetchUntilReady("https://api.example/v1/books", {}, { fetch, wait });
    expect(r.status).toBe(503);
    expect(await r.clone().json()).toEqual({ error: HYDRATING });
    expect(fetch).toHaveBeenCalledTimes(HYDRATE_RETRY_LIMIT);
    expect(wait).toHaveBeenCalledTimes(HYDRATE_RETRY_LIMIT - 1);
  });
});
