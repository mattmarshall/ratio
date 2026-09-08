import { beforeEach, describe, expect, it, vi } from "vitest";

/**
 * handleAuth seals wos-session with cookies().set() then returns a
 * freshly constructed NextResponse.redirect. That is the remaining
 * login bounce after #253: WorkOS finished, the browser never stored
 * the session, caller() sent them back to /signin.
 */

const redirectMock = vi.fn((url: string) => {
  const err = new Error(`NEXT_REDIRECT;replace;${url};307;`);
  (err as Error & { digest: string }).digest = `NEXT_REDIRECT;replace;${url};307;`;
  throw err;
});

const cookieStore = {
  getAll: vi.fn((): { name: string; value: string }[] => []),
  delete: vi.fn(),
};

vi.mock("next/navigation", () => ({
  redirect: (url: string) => redirectMock(url),
}));

vi.mock("next/headers", () => ({
  cookies: async () => cookieStore,
}));

function redirectDest(e: unknown): string | null {
  if (!(e instanceof Error)) return null;
  const digest = "digest" in e && typeof e.digest === "string" ? e.digest : "";
  return digest.split(";").find((p) => p.startsWith("/")) ?? null;
}

describe("pathFromCallbackLocation", () => {
  it("keeps a book path, which is where a completed login should land", async () => {
    const { pathFromCallbackLocation } = await import("./authkitCallback");
    expect(pathFromCallbackLocation("/books")).toBe("/books");
    expect(pathFromCallbackLocation("https://ratio.marsh.build/books")).toBe(
      "/books",
    );
  });

  it("keeps /signin?error= so a failed exchange still has a sentence", async () => {
    const { pathFromCallbackLocation } = await import("./authkitCallback");
    expect(pathFromCallbackLocation("/signin?error=1")).toBe("/signin?error=1");
  });

  it("refuses to return to the prompt itself, which is the bounce", async () => {
    const { pathFromCallbackLocation } = await import("./authkitCallback");
    expect(pathFromCallbackLocation("/signin")).toBe("/");
    expect(pathFromCallbackLocation("/signin?returnTo=%2Fbooks")).toBe("/");
  });
});

describe("continueWithSealedSession", () => {
  beforeEach(() => {
    redirectMock.mockClear();
    cookieStore.getAll.mockReset();
    cookieStore.getAll.mockReturnValue([]);
    cookieStore.delete.mockReset();
  });

  it("sends a successful handleAuth redirect through next/navigation so the sealed session is on that response", async () => {
    const { continueWithSealedSession } = await import("./authkitCallback");
    const response = new Response(null, {
      status: 307,
      headers: { location: "https://ratio.marsh.build/books" },
    });
    const err = await continueWithSealedSession(response).then(
      () => {
        throw new Error("continueWithSealedSession returned a Response");
      },
      (e: unknown) => e,
    );
    expect(redirectDest(err)).toBe("/books");
    expect(redirectMock).toHaveBeenCalledWith("/books");
  });

  it("deletes leftover PKCE verifier cookies on the same response the session rides", async () => {
    cookieStore.getAll.mockReturnValue([
      { name: "wos-auth-verifier-abcd", value: "sealed" },
      { name: "wos-session", value: "session" },
    ]);
    const { continueWithSealedSession } = await import("./authkitCallback");
    await continueWithSealedSession(
      new Response(null, {
        status: 307,
        headers: { location: "/books" },
      }),
    ).then(
      () => {
        throw new Error("continueWithSealedSession returned a Response");
      },
      () => undefined,
    );
    expect(cookieStore.delete).toHaveBeenCalledWith("wos-auth-verifier-abcd");
    expect(cookieStore.delete).not.toHaveBeenCalledWith("wos-session");
  });

  it("leaves a non-redirect body alone, because that is not a completed login", async () => {
    const { continueWithSealedSession } = await import("./authkitCallback");
    const response = new Response("no", { status: 500 });
    await expect(continueWithSealedSession(response)).resolves.toBe(response);
    expect(redirectMock).not.toHaveBeenCalled();
  });
});
