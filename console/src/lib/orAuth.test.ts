import { beforeEach, describe, expect, it, vi } from "vitest";
import { AuthError, NotFound, Refused } from "@/wire/client";

/**
 * `orAuth` is the catch `wire/client.ts` documents: a 401 without a
 * session is a missing session. A 401 *with* a session is not — that is
 * the login bounce, and it must not become `/signin`.
 *
 * ⛔ THIS IS THE #441 PATH. `/books` called `listBooks` after `caller()` had
 * already accepted an AuthKit session; the gateway refused the bearer;
 * `AuthError` left the server component; Next redacted it to digest
 * `2667936230`. A missing session still redirects. A held session must
 * not: it becomes `Refused`, which `orTransient` renders as a status.
 */

const headersMock = vi.fn(async () => new Headers());
const withAuthMock = vi.fn(async () => ({ user: null, accessToken: null }));
const workosMock = vi.fn(() => false);

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined, set: () => {} }),
  headers: () => headersMock(),
}));

// ⚠ `orAuth` imports `signInHref` from `caller`, and `caller` imports
// AuthKit. Vitest cannot resolve AuthKit's `next/cache` import; the
// authenticated `/books` suite already mocks the same module for that
// reason. `signInHref` itself only reads `x-pathname`.
vi.mock("@workos-inc/authkit-nextjs", () => ({
  withAuth: () => withAuthMock(),
}));

vi.mock("./workos", () => ({
  workosConfigured: () => workosMock(),
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
    withAuthMock.mockReset();
    withAuthMock.mockResolvedValue({ user: null, accessToken: null });
    workosMock.mockReset();
    workosMock.mockReturnValue(false);
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

  it("does not carry the prompt as returnTo, which is the bounce", async () => {
    headersMock.mockResolvedValue(
      new Headers({ "x-pathname": "/signin?returnTo=%2Fbooks" }),
    );
    const { orAuth } = await import("./orAuth");
    expect(signInRedirect(await orAuth(Promise.reject(new AuthError())).then(
      () => {
        throw new Error("orAuth resolved an AuthError");
      },
      (e: unknown) => e,
    ))).toBe("/signin");
  });

  it("lets NotFound and Refused through, because they have their own handlers", async () => {
    const { orAuth } = await import("./orAuth");
    await expect(
      orAuth(Promise.reject(new NotFound("no such book"))),
    ).rejects.toBeInstanceOf(NotFound);
    await expect(
      orAuth(Promise.reject(new Refused(400, "no figure"))),
    ).rejects.toBeInstanceOf(Refused);
    // ⛔ A 503 IS orTransient'S JOB, NOT A SIGN-IN. Folding it in here
    // would send a signed-in operator to /signin while the API rolls.
    await expect(
      orAuth(Promise.reject(new Refused(503, "unavailable"))),
    ).rejects.toBeInstanceOf(Refused);
  });

  it("returns the value when the read succeeded", async () => {
    const { orAuth } = await import("./orAuth");
    await expect(orAuth(Promise.resolve(7))).resolves.toBe(7);
  });

  it("does not send a signed-in operator to /signin when the gateway refuses the bearer", async () => {
    workosMock.mockReturnValue(true);
    withAuthMock.mockResolvedValue({
      user: { id: "u-1" },
      accessToken: "access-token",
    });
    headersMock.mockResolvedValue(new Headers({ "x-pathname": "/books" }));
    const { orAuth } = await import("./orAuth");
    const { SESSION_REFUSED } = await import("./sessionRefused");
    const err = await orAuth(Promise.reject(new AuthError())).then(
      () => {
        throw new Error("orAuth resolved an AuthError");
      },
      (e: unknown) => e,
    );
    expect(err).toBeInstanceOf(Refused);
    expect(err).toMatchObject({ status: 401, message: SESSION_REFUSED });
    expect(signInRedirect(err)).toBeNull();
  });
});
