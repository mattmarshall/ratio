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

vi.mock("@/wire/client", async () => {
  const actual = await vi.importActual<typeof import("@/wire/client")>(
    "@/wire/client",
  );
  return {
    ...actual,
    listBooks: async () => booksFixture,
  };
});

async function renderAsync(el: Promise<ReactNode>) {
  render((await el) as ReactElement);
}

describe("authenticated /books", () => {
  beforeEach(() => {
    headersMock.mockReset();
    headersMock.mockResolvedValue(new Headers());
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
});
