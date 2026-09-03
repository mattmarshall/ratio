import { beforeEach, describe, expect, it, vi } from "vitest";
import { AuthError, NotFound, Refused } from "@/wire/client";

/**
 * `orAuth` is the catch `wire/client.ts` documents: a 401 is a missing
 * session, not a figure to render and not a crash for `error.tsx`.
 *
 * ⛔ THIS IS THE #441 PATH. `/books` called `listBooks` after `caller()` had
 * already accepted an AuthKit session; the gateway refused the bearer;
 * `AuthError` left the server component; Next redacted it to digest
 * `2667936230`. The helper must redirect, not rethrow.
 */

const headersMock = vi.fn(async () => new Headers());

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined, set: () => {} }),
  headers: () => headersMock(),
}));

/** Next's `redirect()` throws; the destination lives on `digest`. */
function signInRedirect(e: unknown): string | null {
  if (!(e instanceof Error)) return null;
  const digest = "digest" in e && typeof e.digest === "string" ? e.digest : "";
  const m = `${e.message}\n${digest}`.match(/\/signin(?:\?returnTo=[^;\s]+)?/);
  return m?.[0] ?? null;
}

describe("orAuth", () => {
  beforeEach(() => {
    headersMock.mockReset();
    headersMock.mockResolvedValue(new Headers());
  });

  it("redirects AuthError to /signin?returnTo= the path the proxy set", async () => {
    headersMock.mockResolvedValue(new Headers({ "x-pathname": "/books" }));
    const { orAuth } = await import("./orAuth");
    const err = await orAuth(Promise.reject(new AuthError())).then(
      () => {
        throw new Error("orAuth resolved an AuthError");
      },
      (e: unknown) => e,
    );
    expect(err).not.toBeInstanceOf(AuthError);
    expect(signInRedirect(err)).toBe("/signin?returnTo=%2Fbooks");
  });

  it("falls back to /signin when the proxy set no path", async () => {
    const { orAuth } = await import("./orAuth");
    expect(signInRedirect(await orAuth(Promise.reject(new AuthError())).then(
      () => {
        throw new Error("orAuth resolved an AuthError");
      },
      (e: unknown) => e,
    ))).toBe("/signin");
  });

  it("reuses caller()'s rooted path — it does not invent a host", async () => {
    headersMock.mockResolvedValue(new Headers({ "x-pathname": "/books/new" }));
    const { orAuth } = await import("./orAuth");
    const dest = signInRedirect(
      await orAuth(Promise.reject(new AuthError())).then(
        () => {
          throw new Error("orAuth resolved an AuthError");
        },
        (e: unknown) => e,
      ),
    );
    expect(dest).toBe("/signin?returnTo=%2Fbooks%2Fnew");
    expect(dest?.startsWith("/")).toBe(true);
    expect(dest).not.toMatch(/^https?:/);
  });

  it("lets NotFound and Refused through, because they have their own handlers", async () => {
    const { orAuth } = await import("./orAuth");
    await expect(
      orAuth(Promise.reject(new NotFound("no such book"))),
    ).rejects.toBeInstanceOf(NotFound);
    await expect(
      orAuth(Promise.reject(new Refused(400, "no figure"))),
    ).rejects.toBeInstanceOf(Refused);
  });

  it("returns the value when the read succeeded", async () => {
    const { orAuth } = await import("./orAuth");
    await expect(orAuth(Promise.resolve(7))).resolves.toBe(7);
  });
});
