import { describe, expect, it, vi } from "vitest";

/**
 * AuthKit proxy matcher — the production #441 path on `/books/[book].rsc`.
 *
 * ⛔ DIGEST `2094318646`. Next skips middleware when the matcher `missing`
 * list includes `next-router-prefetch` / `purpose: prefetch`. RSC fetches
 * to `/books/[book]` arrive as prefetch; `authkit()` never runs;
 * `withAuth()` throws; Next redacts to #441. Official AuthKit matcher
 * (authkit-nextjs README) excludes only `_next/static`, `_next/image`,
 * and `favicon.ico`.
 *
 * AuthKit is stubbed so this file can import `config` without resolving
 * the SDK's `next/cache` graph — the same reason `orAuth.test.ts` mocks it.
 */

vi.mock("@workos-inc/authkit-nextjs", () => ({
  applyResponseHeaders: (res: unknown) => res,
  authkit: async () => ({ headers: new Headers() }),
  partitionAuthkitHeaders: (
    req: { headers: Headers },
    _authkitHeaders: Headers,
  ) => ({
    requestHeaders: new Headers(req.headers),
    responseHeaders: new Headers(),
  }),
}));

vi.mock("@/lib/workos", () => ({
  workosConfigured: () => false,
}));

type MatcherHas = { type: string; key: string; value?: string };
type MatcherEntry =
  | string
  | {
      source: string;
      missing?: MatcherHas[];
    };

function entries(matcher: unknown): MatcherEntry[] {
  if (matcher == null) return [];
  return Array.isArray(matcher) ? matcher : [matcher as MatcherEntry];
}

function skipsWhenHeaderPresent(
  entry: MatcherEntry,
  key: string,
  value?: string,
): boolean {
  if (typeof entry === "string") return false;
  return (entry.missing ?? []).some((c) => {
    if (c.key !== key) return false;
    return value === undefined ? true : c.value === value;
  });
}

describe("AuthKit proxy matcher", () => {
  it("covers prefetch RSC fetches so withAuth can run (digest 2094318646)", async () => {
    const { config } = await import("./proxy");
    const matchers = entries(config.matcher);
    expect(matchers.length).toBeGreaterThan(0);

    for (const entry of matchers) {
      const source = typeof entry === "string" ? entry : entry.source;
      expect(source).toContain("(?!_next/static|_next/image|favicon.ico)");
      expect(
        skipsWhenHeaderPresent(entry, "next-router-prefetch"),
        "matcher must not skip next-router-prefetch",
      ).toBe(false);
      expect(
        skipsWhenHeaderPresent(entry, "purpose", "prefetch"),
        "matcher must not skip purpose: prefetch",
      ).toBe(false);
    }
  });

  it("still partitions AuthKit headers onto the request (#112)", async () => {
    const src = await import("node:fs").then((fs) =>
      fs.readFileSync(new URL("./proxy.ts", import.meta.url), "utf8"),
    );
    expect(src).toContain("mergeAuthkitProxyHeaders");
    expect(src).toContain("applyResponseHeaders");
  });
});
