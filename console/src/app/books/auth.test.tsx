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
const getBook = vi.fn(async () => {
  throw new Error("getBook is not stubbed in this test");
});
const getView = vi.fn(async () => {
  throw new Error("getView is not stubbed in this test");
});

vi.mock("@/wire/client", async () => {
  const actual = await vi.importActual<typeof import("@/wire/client")>(
    "@/wire/client",
  );
  return {
    ...actual,
    listBooks,
    listFunds,
    getBook,
    getView,
  };
});

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
    getBook.mockReset();
    getView.mockReset();
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
  // `listBooks` threw `AuthError`. Sending that operator to `/signin` is the
  // login bounce: they sign in again, AuthKit reuses the session, `/books`
  // 401s again. A missing session still redirects. A held session is a
  // status — the same door as a 503 — so Next never redacts it to `#441`.
  it("renders a session-refused status when the API refuses a bearer the operator already holds", async () => {
    vi.resetModules();
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
    await renderAsync(Books());
    expect(screen.getByRole("status").textContent).toContain(
      "the API did not accept it",
    );
    expect(screen.queryByText("Your books")).toBeNull();

    const { default: BooksLayout } = await import("./layout");
    await renderAsync(BooksLayout({ children: null }));
    expect(screen.getAllByRole("status").length).toBeGreaterThan(0);
  });

  it("renders a session-refused status on the book hub when GetBook refuses a bearer the operator already holds", async () => {
    vi.resetModules();
    const { AuthError } = await import("@/wire/client");
    getBook.mockRejectedValue(new AuthError());
    getView.mockRejectedValue(new AuthError());
    headersMock.mockResolvedValue(
      new Headers({
        "x-workos-middleware": "true",
        "x-workos-session": "sealed",
        "x-pathname": "/books/harbourline-global-value",
      }),
    );

    const { default: BookPage } = await import("./[book]/page");
    await renderAsync(
      BookPage({
        params: Promise.resolve({ book: "harbourline-global-value" }),
      }),
    );
    expect(screen.getByRole("status").textContent).toContain(
      "the API did not accept it",
    );
  });
  // API was rolling and GET /books answered 503; `listBooks` threw
  // `Refused`; `orAuth` rethrew it; Next redacted the page to digest
  // `2106392403`. A 503 is not a missing session — the operator stays
  // on /books and can try again.
  it("renders a recoverable status when the API answers 503, instead of throwing Refused", async () => {
    vi.resetModules();
    const { Refused } = await import("@/wire/client");
    listBooks.mockRejectedValue(new Refused(503, "unavailable"));
    listFunds.mockRejectedValue(new Refused(503, "unavailable"));
    headersMock.mockResolvedValue(
      new Headers({
        "x-workos-middleware": "true",
        "x-workos-session": "sealed",
        "x-pathname": "/books",
      }),
    );

    const { default: Books } = await import("./page");
    await renderAsync(Books());
    expect(screen.getByRole("status").textContent).toContain(
      "temporarily unavailable",
    );
    expect(screen.getByRole("button", { name: "Try again" })).toBeDefined();

    const { default: BooksLayout } = await import("./layout");
    await renderAsync(BooksLayout({ children: null }));
    expect(screen.getAllByRole("status").length).toBeGreaterThan(0);
  });

  // After #255 the gateway accepted the bearer. A cold Lambda then
  // 503'd `the journal is still hydrating`. That is not a refused
  // session (the bounce) and not a lasting "API unavailable". `send()`
  // retries it; this is the exhausted leftover so Next never redacts it.
  it("shows a hydrating 503 as a journal still opening, not a refused session", async () => {
    vi.resetModules();
    const { Refused } = await import("@/wire/client");
    listBooks.mockRejectedValue(
      new Refused(503, "the journal is still hydrating"),
    );
    listFunds.mockRejectedValue(
      new Refused(503, "the journal is still hydrating"),
    );
    headersMock.mockResolvedValue(
      new Headers({
        "x-workos-middleware": "true",
        "x-workos-session": "sealed",
        "x-pathname": "/books",
      }),
    );

    const { default: Books } = await import("./page");
    await renderAsync(Books());
    const status = screen.getByRole("status").textContent ?? "";
    expect(status).toContain("still opening");
    expect(status).toContain("still hydrating");
    expect(status).not.toContain("the API did not accept it");
    expect(status).not.toContain("temporarily unavailable");
    expect(screen.getByRole("button", { name: "Try again" })).toBeDefined();
  });
});
