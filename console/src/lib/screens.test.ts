import { describe, expect, it } from "vitest";
import {
  defaultScreen,
  INVESTMENT_SCREENS,
  PERSONAL_SCREENS,
  PROJECT_SCREENS,
  screensFor,
  ticketsFor,
} from "./screens";

describe("screensFor", () => {
  it("a personal book opens a sheet, not Exceptions or NAV", () => {
    const labels = screensFor("PERSONAL").map((s) => s.label);
    expect(labels).toContain("Balance sheet");
    expect(labels).toContain("Period P&L");
    expect(labels).toContain("Budget vs actual");
    expect(labels).toContain("Loan schedule");
    expect(labels).toContain("Trial balance");
    expect(labels).not.toContain("Exceptions");
    expect(labels).not.toContain("NAV");
    expect(labels).not.toContain("Positions");
    expect(labels).not.toContain("WIP");
    expect(labels).not.toContain("Billing");
    expect(defaultScreen("PERSONAL")).toBe("sheet");
    expect(ticketsFor("PERSONAL").map((t) => t.segment)).toEqual([
      "transfer",
      "record",
      "ingest",
    ]);
  });

  it("a project book offers budget, WIP, and billing, not Exceptions or NAV", () => {
    const segments = screensFor("PROJECT").map((s) => s.segment);
    expect(segments.slice(0, 4)).toEqual(["budget", "wip", "billing", "accounts"]);
    const labels = screensFor("PROJECT").map((s) => s.label);
    expect(labels).toContain("Budget vs actual");
    expect(labels).toContain("WIP");
    expect(labels).toContain("Billing");
    expect(labels).toContain("Trial balance");
    expect(labels).not.toContain("Exceptions");
    expect(labels).not.toContain("NAV");
    expect(labels).not.toContain("Positions");
    expect(defaultScreen("PROJECT")).toBe("budget");
    expect(ticketsFor("PROJECT").map((t) => t.segment)).toEqual([
      "record",
      "ingest",
    ]);
    expect(ticketsFor("PROJECT").some((t) => t.segment === "trade")).toBe(false);
    expect(ticketsFor("PROJECT").some((t) => t.segment === "mark")).toBe(false);
  });

  it("an investment book cites capital first, then the ABOR warehouse", () => {
    const labels = screensFor("INVESTMENT").map((s) => s.label);
    expect(labels[0]).toBe("Capital activity");
    expect(labels).toContain("Exceptions");
    expect(labels).toContain("NAV");
    expect(labels).toContain("Positions");
    expect(labels).not.toContain("Balance sheet");
    expect(labels).not.toContain("WIP");
    expect(labels).not.toContain("Billing");
    expect(defaultScreen("INVESTMENT")).toBe("capital");
    expect(ticketsFor("INVESTMENT").map((t) => t.segment)).toEqual([
      "trade",
      "record",
      "ingest",
      "mark",
    ]);
  });

  it("an unspecified book keeps the operations surface", () => {
    const labels = screensFor("UNSPECIFIED").map((s) => s.label);
    expect(labels).toContain("Exceptions");
    expect(labels).toContain("NAV");
    expect(defaultScreen("UNSPECIFIED")).toBe("breaks");
  });

  it("figure screens are view-scoped", () => {
    for (const s of [
      ...PERSONAL_SCREENS.filter((x) =>
        ["sheet", "pnl", "budget", "loans"].includes(x.segment),
      ),
      ...PROJECT_SCREENS.filter((x) =>
        ["budget", "wip", "billing"].includes(x.segment),
      ),
      ...INVESTMENT_SCREENS.filter((x) => x.segment === "capital"),
    ]) {
      expect(s.scoped).toBe(true);
      expect(s.group).toBe("book");
    }
  });
});
