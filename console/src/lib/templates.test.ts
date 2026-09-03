import { describe, expect, it } from "vitest";
import { BOOK_TEMPLATES } from "./templates";

describe("book templates", () => {
  it("are the three CreateBook kinds, not a second ledger", () => {
    expect(BOOK_TEMPLATES.map((t) => t.kind)).toEqual([
      "PERSONAL",
      "INVESTMENT",
      "PROJECT",
    ]);
  });

  it("names accounts chart_for actually writes", () => {
    const byKind = Object.fromEntries(BOOK_TEMPLATES.map((t) => [t.kind, t.blurb]));
    expect(byKind.PERSONAL).toMatch(/Cash and bank/);
    expect(byKind.INVESTMENT).toMatch(/fair value/);
    expect(byKind.INVESTMENT).toMatch(/distributions/);
    expect(byKind.INVESTMENT).toMatch(/partner capital/);
    expect(byKind.INVESTMENT).toMatch(/Does not file a fund/);
    expect(byKind.PROJECT).toMatch(/work in progress/);
    expect(byKind.PROJECT).toMatch(/not a second ledger/);
  });
});
