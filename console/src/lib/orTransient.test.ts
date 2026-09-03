import { beforeEach, describe, expect, it, vi } from "vitest";
import { AuthError, NotFound, Refused } from "@/wire/client";

/**
 * `orTransient` is the catch the 2026-09-03 /books 503 needed:
 * `orAuth` only handles 401, so a rolling API's `Refused(503)` left
 * the server component and Next redacted it to digest `2106392403`.
 *
 * ⛔ THIS IS THE #441 PATH FOR A TRANSPORT FAILURE. A rejected
 * `Refused(503)` must not escape. A 401 must still become `/signin`.
 * A 400 must still throw — that is `orRefused`'s job, not a second
 * AuthError path.
 */

const headersMock = vi.fn(async () => new Headers());

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined, set: () => {} }),
  headers: () => headersMock(),
}));

// ⚠ Same AuthKit stub as `orAuth.test.ts`: `orTransient` calls
// `orAuth`, which imports `signInHref` from `caller`.
vi.mock("@workos-inc/authkit-nextjs", () => ({
  withAuth: async () => ({ user: null, accessToken: null }),
}));

vi.mock("./workos", () => ({
  workosConfigured: () => false,
}));

/** Next's `redirect()` throws; the destination lives on `digest`. */
function signInRedirect(e: unknown): string | null {
  if (!(e instanceof Error)) return null;
  const digest = "digest" in e && typeof e.digest === "string" ? e.digest : "";
  const m = `${e.message}\n${digest}`.match(/\/signin(?:\?returnTo=[^;\s]+)?/);
  return m?.[0] ?? null;
}

describe("orTransient", () => {
  beforeEach(() => {
    headersMock.mockReset();
    headersMock.mockResolvedValue(new Headers());
  });

  it("does not let a 503 Refused escape as an uncaught throw", async () => {
    const { orTransient } = await import("./orTransient");
    await expect(
      orTransient(Promise.reject(new Refused(503, "unavailable"))),
    ).resolves.toEqual({ unavailable: "unavailable" });
  });

  it("treats any status ≥ 500 the same — a 500 is also not a figure", async () => {
    const { orTransient } = await import("./orTransient");
    await expect(
      orTransient(Promise.reject(new Refused(500, "internal"))),
    ).resolves.toEqual({ unavailable: "internal" });
  });

  it("redirects AuthError to /signin — a 401 is still a missing session", async () => {
    headersMock.mockResolvedValue(new Headers({ "x-pathname": "/books" }));
    const { orTransient } = await import("./orTransient");
    const err = await orTransient(Promise.reject(new AuthError())).then(
      () => {
        throw new Error("orTransient resolved an AuthError");
      },
      (e: unknown) => e,
    );
    expect(err).not.toBeInstanceOf(AuthError);
    expect(signInRedirect(err)).toBe("/signin?returnTo=%2Fbooks");
  });

  it("lets a 400 Refused through, because that is orRefused's sentence", async () => {
    const { orTransient } = await import("./orTransient");
    await expect(
      orTransient(Promise.reject(new Refused(400, "no figure"))),
    ).rejects.toBeInstanceOf(Refused);
  });

  it("lets NotFound through, because or404 is its handler", async () => {
    const { orTransient } = await import("./orTransient");
    await expect(
      orTransient(Promise.reject(new NotFound("no such book"))),
    ).rejects.toBeInstanceOf(NotFound);
  });

  it("returns the value when the read succeeded", async () => {
    const { orTransient } = await import("./orTransient");
    await expect(orTransient(Promise.resolve(7))).resolves.toEqual({
      unavailable: null,
      value: 7,
    });
  });
});
