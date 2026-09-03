import type { ReactElement, ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import booksFixture from "../../../fixtures/books.json";

/**
 * Authenticated `/books` render path — the one that crashed in production.
 *
 * `withAuth()` refuses to run unless `x-workos-middleware` reached the server
 * component through the proxy. The books page and header chip both call into
 * `caller()` / `principal()`, which invoke `withAuth()` when WorkOS is on.
 */

vi.mock("@/lib/workos", () => ({
  workosConfigured: () => true,
}));

vi.mock("@workos-inc/authkit-nextjs", () => ({
  withAuth: async () => {
    const { headers } = await import("next/headers");
    const h = await headers();
    if (!h.get("x-workos-middleware")) {
      throw new Error(
        "You are calling 'withAuth' on a route that isn't covered by the AuthKit middleware.",
      );
    }
    return {
      user: { id: "u-1", email: "e.marsh@example.com" },
      accessToken: "access-token",
    };
  },
}));

const headersMock = vi.fn(async () => new Headers());

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined, set: () => {} }),
  headers: () => headersMock(),
}));

const listBooks = vi.fn(async () => booksFixture);
const listFunds = vi.fn(async () => ({ funds: [] }));

vi.mock("@/wire/client", async () => {
  const actual = await vi.importActual<typeof import("@/wire/client")>(
    "@/wire/client",
  );
  return {
    ...actual,
    listBooks: (...args: unknown[]) => listBooks(...args),
    listFunds: (...args: unknown[]) => listFunds(...args),
  };
});

/** Next's `redirect()` throws; the destination lives on `digest`. */
function signInRedirect(e: unknown): string | null {
  if (!(e instanceof Error)) return null;
  const digest = "digest" in e && typeof e.digest === "string" ? e.digest : "";
  const m = `${e.message}\n${digest}`.match(/\/signin(?:\?returnTo=[^;\s]+)?/);
  return m?.[0] ?? null;
}

async function renderAsync(el: Promise<ReactNode>) {
  render((await el) as ReactElement);
}

describe("authenticated /books", () => {
  beforeEach(() => {
    headersMock.mockReset();
    headersMock.mockResolvedValue(new Headers());
    listBooks.mockReset();
    listBooks.mockResolvedValue(booksFixture);
    listFunds.mockReset();
    listFunds.mockResolvedValue({ funds: [] });
  });

  it("throws when AuthKit middleware headers never reached the page", async () => {
    const Books = (await import("./page")).default;
    await expect(Books()).rejects.toThrow(/isn't covered by the AuthKit middleware/);
  });

  it("renders the book list when middleware headers are present", async () => {
    headersMock.mockResolvedValue(
      new Headers({
        "x-workos-middleware": "true",
        "x-workos-session": "sealed",
      }),
    );

    const Books = (await import("./page")).default;
    await renderAsync(Books());
    expect(screen.getByText("Your books")).toBeDefined();
    expect(screen.getByText("Household")).toBeDefined();

    const { Who } = await import("@/components/Who");
    await renderAsync(Who());
    expect(screen.getByText("e.marsh@example.com")).toBeDefined();
  });

  // ⛔ THE PRODUCTION FAILURE, NAMED. AuthKit had a session, so `caller()`
  // sent the bearer; the gateway refused it (audience / `WORKOS_CLIENT_ID`);
  // `listBooks` threw `AuthError`; layout and page both awaited the same
  // helper; Next redacted the throw to `Minified React error #441`. A test
  // that only checks a missing session would stay green for this case.
  it("redirects to sign-in when the API refuses the bearer, instead of throwing AuthError", async () => {
    vi.resetModules();
    // ⚠ Construct AFTER resetModules. `orAuth` compares `instanceof AuthError`
    // against the class it imported; a class from the previous module graph
    // would miss and the throw would look like the production #441 again.
    const { AuthError } = await import("@/wire/client");
    listBooks.mockRejectedValue(new AuthError());
    listFunds.mockRejectedValue(new AuthError());
    headersMock.mockResolvedValue(
      new Headers({
        "x-workos-middleware": "true",
        "x-workos-session": "sealed",
        "x-pathname": "/books",
      }),
    );

    const { default: Books } = await import("./page");
    const pageErr = await Books().then(
      () => {
        throw new Error("page rendered an AuthError as a row");
      },
      (e: unknown) => e,
    );
    expect(pageErr).not.toBeInstanceOf(AuthError);
    expect(signInRedirect(pageErr)).toBe("/signin?returnTo=%2Fbooks");

    const { default: BooksLayout } = await import("./layout");
    const layoutErr = await BooksLayout({ children: null }).then(
      () => {
        throw new Error("layout rendered an AuthError as chrome");
      },
      (e: unknown) => e,
    );
    expect(layoutErr).not.toBeInstanceOf(AuthError);
    expect(signInRedirect(layoutErr)).toBe("/signin?returnTo=%2Fbooks");
  });
});
