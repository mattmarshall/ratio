import { describe, expect, it } from "vitest";
import nextConfig from "../next.config";

/**
 * Old `/funds/{fund}/…` job URLs were sent as permalinks. They redirect
 * onto the book. `/funds` and `/funds/{fund}` stay — ListFunds is
 * funds-only, and a fund is an optional filing.
 */
describe("fund job URLs", () => {
  it("redirect onto the book and leave the filing page alone", async () => {
    const redirects = await nextConfig.redirects!();
    const sources = redirects.map((r) => r.source);
    expect(sources).toEqual([
      "/funds/:fund/views/:path*",
      "/funds/:fund/:job",
      "/funds/:fund/:job/:path*",
    ]);
    for (const r of redirects) {
      expect(r.destination.startsWith("/books/")).toBe(true);
      expect(r.permanent).toBe(true);
    }
  });
});
