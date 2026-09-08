import { beforeEach, describe, expect, it, vi } from "vitest";

const withAuthMock = vi.fn(async () => ({ user: null, accessToken: null }));
const workosMock = vi.fn(() => false);

vi.mock("@workos-inc/authkit-nextjs", () => ({
  withAuth: () => withAuthMock(),
}));

vi.mock("./workos", () => ({
  workosConfigured: () => workosMock(),
}));

function redirectDest(e: unknown): string | null {
  if (!(e instanceof Error)) return null;
  const digest = "digest" in e && typeof e.digest === "string" ? e.digest : "";
  const m = `${e.message}\n${digest}`.match(/\/[^\s;]+/);
  return m?.[0] ?? null;
}

describe("continueIfSignedIn", () => {
  beforeEach(() => {
    withAuthMock.mockReset();
    withAuthMock.mockResolvedValue({ user: null, accessToken: null });
    workosMock.mockReset();
    workosMock.mockReturnValue(false);
  });

  it("does nothing when AuthKit is not configured", async () => {
    const { continueIfSignedIn } = await import("./continueIfSignedIn");
    await expect(continueIfSignedIn("/books")).resolves.toBeUndefined();
  });

  it("does nothing when there is no session, so the prompt can render", async () => {
    workosMock.mockReturnValue(true);
    const { continueIfSignedIn } = await import("./continueIfSignedIn");
    await expect(continueIfSignedIn("/books")).resolves.toBeUndefined();
  });

  it("sends a signed-in operator onward instead of showing the prompt", async () => {
    workosMock.mockReturnValue(true);
    withAuthMock.mockResolvedValue({
      user: { id: "u-1", email: "e.marsh@example.com" },
      accessToken: "access-token",
    });
    const { continueIfSignedIn } = await import("./continueIfSignedIn");
    const err = await continueIfSignedIn("/books").then(
      () => {
        throw new Error("continueIfSignedIn resolved while signed in");
      },
      (e: unknown) => e,
    );
    expect(redirectDest(err)).toBe("/books");
  });

  it("refuses to send them back to /signin even if that was returnTo", async () => {
    workosMock.mockReturnValue(true);
    withAuthMock.mockResolvedValue({
      user: { id: "u-1" },
      accessToken: "access-token",
    });
    const { continueIfSignedIn } = await import("./continueIfSignedIn");
    const err = await continueIfSignedIn("/signin?returnTo=%2Fbooks").then(
      () => {
        throw new Error("continueIfSignedIn resolved while signed in");
      },
      (e: unknown) => e,
    );
    expect(redirectDest(err)).toBe("/");
  });
});
