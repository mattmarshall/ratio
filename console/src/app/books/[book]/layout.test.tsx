import type { ReactElement, ReactNode } from "react";
import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import bookFixture from "../../../../fixtures/book.json";
import fundFixture from "../../../../fixtures/fund.json";
import viewsFixture from "../../../../fixtures/views.json";

/**
 * Identity-layout reads — the #441 hole the collection layout already closed.
 *
 * ⛔ `/books/[book]` AND `/funds/[fund]` AWAITED GetBook / GetFund /
 * ListViews with no `orAuth` / `orTransient`. After digest `2094318646`
 * (prefetch skipping AuthKit) is fixed, a 401 or 503 here still leaves
 * the server component and Next redacts it. The wrap is the same as
 * `books/layout.tsx`: 5xx → `<Unavailable>`, 401 → `/signin`.
 */

const headersMock = vi.fn(async () => new Headers());

vi.mock("next/headers", () => ({
  cookies: async () => ({ get: () => undefined, set: () => {} }),
  headers: () => headersMock(),
}));

vi.mock("@workos-inc/authkit-nextjs", () => ({
  withAuth: async () => ({ user: null, accessToken: null }),
}));

vi.mock("@/lib/workos", () => ({
  workosConfigured: () => false,
}));

const getBook = vi.fn(async () => bookFixture);
const getFund = vi.fn(async () => fundFixture);
const listViews = vi.fn(async () => viewsFixture);

vi.mock("@/wire/client", async () => {
  const actual = await vi.importActual<typeof import("@/wire/client")>(
    "@/wire/client",
  );
  return {
    ...actual,
    getBook,
    getFund,
    listViews,
  };
});

function signInRedirect(e: unknown): string | null {
  if (!(e instanceof Error)) return null;
  const digest = "digest" in e && typeof e.digest === "string" ? e.digest : "";
  const m = `${e.message}\n${digest}`.match(/\/signin(?:\?returnTo=[^;\s]+)?/);
  return m?.[0] ?? null;
}

async function renderAsync(el: Promise<ReactNode>) {
  render((await el) as ReactElement);
}

describe("identity layouts wrap GetBook / GetFund / ListViews", () => {
  beforeEach(() => {
    headersMock.mockReset();
    headersMock.mockResolvedValue(new Headers());
    getBook.mockReset();
    getBook.mockResolvedValue(bookFixture);
    getFund.mockReset();
    getFund.mockResolvedValue(fundFixture);
    listViews.mockReset();
    listViews.mockResolvedValue(viewsFixture);
  });

  it("renders a recoverable status when GetBook answers 503, instead of throwing Refused", async () => {
    vi.resetModules();
    const { Refused } = await import("@/wire/client");
    getBook.mockRejectedValue(new Refused(503, "unavailable"));
    headersMock.mockResolvedValue(
      new Headers({ "x-pathname": "/books/harbourline-global-value" }),
    );

    const { default: BookLayout } = await import("./layout");
    await renderAsync(
      BookLayout({
        children: null,
        params: Promise.resolve({ book: "harbourline-global-value" }),
      }),
    );
    expect(screen.getByRole("status").textContent).toContain(
      "temporarily unavailable",
    );
    expect(screen.getByRole("button", { name: "Try again" })).toBeDefined();
  });

  it("redirects to sign-in when GetBook refuses the bearer, instead of throwing AuthError", async () => {
    vi.resetModules();
    const { AuthError } = await import("@/wire/client");
    getBook.mockRejectedValue(new AuthError());
    headersMock.mockResolvedValue(
      new Headers({ "x-pathname": "/books/harbourline-global-value" }),
    );

    const { default: BookLayout } = await import("./layout");
    const err = await BookLayout({
      children: null,
      params: Promise.resolve({ book: "harbourline-global-value" }),
    }).then(
      () => {
        throw new Error("book layout rendered an AuthError as chrome");
      },
      (e: unknown) => e,
    );
    expect(err).not.toBeInstanceOf(AuthError);
    expect(signInRedirect(err)).toBe(
      "/signin?returnTo=%2Fbooks%2Fharbourline-global-value",
    );
  });

  it("renders a recoverable status when GetFund answers 503, instead of throwing Refused", async () => {
    vi.resetModules();
    const { Refused } = await import("@/wire/client");
    getFund.mockRejectedValue(new Refused(503, "unavailable"));
    headersMock.mockResolvedValue(
      new Headers({ "x-pathname": "/funds/harbourline-global-value" }),
    );

    const { default: FundLayout } = await import("../../funds/[fund]/layout");
    await renderAsync(
      FundLayout({
        children: null,
        params: Promise.resolve({ fund: "harbourline-global-value" }),
      }),
    );
    expect(screen.getByRole("status").textContent).toContain(
      "temporarily unavailable",
    );
    expect(screen.getByRole("button", { name: "Try again" })).toBeDefined();
  });
});
