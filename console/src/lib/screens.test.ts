import { describe, expect, it } from "vitest";
import {
  defaultScreen,
  FUND_OPS_DEEP_LINKS,
  FUND_SCREENS,
  INVESTMENT_SCREENS,
  OPERATING_SCREENS,
  PERSONAL_SCREENS,
  PROJECT_SCREENS,
  screensFor,
  ticketsFor,
  wearsFundOps,
} from "./screens";

describe("screensFor", () => {
  it("a personal book opens a sheet, not Exceptions or NAV", () => {
    const labels = screensFor("PERSONAL").map((s) => s.label);
    expect(labels).toContain("Balance sheet");
    expect(labels).toContain("Period P&L");
    expect(labels).toContain("Net-worth bridge");
    expect(labels).toContain("Cash flow");
    expect(labels).toContain("Period close");
    expect(labels).toContain("As-of");
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
    expect(segments.slice(0, 6)).toEqual([
      "budget",
      "wip",
      "billing",
      "close",
      "asof",
      "accounts",
    ]);
    const labels = screensFor("PROJECT").map((s) => s.label);
    expect(labels).toContain("Budget vs actual");
    expect(labels).toContain("WIP");
    expect(labels).toContain("Billing");
    expect(labels).toContain("Trial balance");
    expect(labels).not.toContain("Exceptions");
    expect(labels).not.toContain("NAV");
    expect(labels).not.toContain("Positions");
    expect(labels).not.toContain("Cash flow");
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
    expect(labels[1]).toBe("NAV roll-forward");
    expect(labels[2]).toBe("Period close");
    expect(labels[3]).toBe("As-of");
    expect(labels).toContain("Exceptions");
    expect(labels).toContain("NAV");
    expect(labels).toContain("Positions");
    expect(labels).not.toContain("Balance sheet");
    expect(labels).not.toContain("WIP");
    expect(labels).not.toContain("Billing");
    expect(labels).not.toContain("Cash flow");
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
    expect(labels).toContain("Period close");
    expect(labels).toContain("As-of");
    expect(labels).not.toContain("Cash flow");
    expect(labels).not.toContain("Income statement");
    expect(defaultScreen("UNSPECIFIED")).toBe("breaks");
    expect(screensFor("UNSPECIFIED")).not.toEqual(screensFor("OPERATING"));
  });

  it("an operating book opens a sheet, income statement, and cash-flow, not Fund or Project chrome", () => {
    const segments = screensFor("OPERATING").map((s) => s.segment);
    expect(segments.slice(0, 7)).toEqual([
      "sheet",
      "pnl",
      "cashflow",
      "close",
      "asof",
      "aging",
      "accounts",
    ]);
    const labels = screensFor("OPERATING").map((s) => s.label);
    expect(labels).toContain("Balance sheet");
    expect(labels).toContain("Income statement");
    expect(labels).toContain("Cash flow");
    expect(labels).toContain("Period close");
    expect(labels).toContain("As-of");
    expect(labels).toContain("AR/AP aging");
    expect(labels).toContain("Trial balance");
    expect(labels).not.toContain("Exceptions");
    expect(labels).not.toContain("NAV");
    expect(labels).not.toContain("Positions");
    expect(labels).not.toContain("WIP");
    expect(labels).not.toContain("Billing");
    expect(labels).not.toContain("Net-worth bridge");
    expect(labels).not.toContain("Loan schedule");
    expect(labels).not.toContain("Capital activity");
    expect(defaultScreen("OPERATING")).toBe("sheet");
    expect(ticketsFor("OPERATING").map((t) => t.segment)).toEqual([
      "record",
      "ingest",
    ]);
    expect(ticketsFor("OPERATING").some((t) => t.segment === "trade")).toBe(false);
    expect(ticketsFor("OPERATING").some((t) => t.segment === "mark")).toBe(false);
    expect(ticketsFor("OPERATING").some((t) => t.segment === "transfer")).toBe(
      false,
    );
  });

  it("Personal, Project, and Operating do not wear fund-ops chrome", () => {
    // ⭐ #175. screensFor already hid the tabs; a typed URL or palette
    // deep-link that still offered Exceptions / Positions / NAV was the
    // leftover. wearsFundOps is the one predicate the pages and the
    // palette both read.
    expect(wearsFundOps("INVESTMENT")).toBe(true);
    expect(wearsFundOps("UNSPECIFIED")).toBe(true);
    expect(wearsFundOps("PERSONAL")).toBe(false);
    expect(wearsFundOps("PROJECT")).toBe(false);
    expect(wearsFundOps("OPERATING")).toBe(false);
    expect([...FUND_OPS_DEEP_LINKS]).toEqual([
      "breaks",
      "positions",
      "strikes",
      "actions",
    ]);
    for (const kind of ["PERSONAL", "PROJECT", "OPERATING"] as const) {
      const segments = screensFor(kind).map((s) => s.segment);
      expect(segments).not.toContain("breaks");
      expect(segments).not.toContain("positions");
      expect(segments).not.toContain("strikes");
      expect(ticketsFor(kind).some((t) => t.segment === "trade")).toBe(false);
      expect(ticketsFor(kind).some((t) => t.segment === "mark")).toBe(false);
    }
    const investment = screensFor("INVESTMENT").map((s) => s.segment);
    expect(investment).toContain("breaks");
    expect(investment).toContain("positions");
    expect(investment).toContain("strikes");
    expect(ticketsFor("INVESTMENT").map((t) => t.segment)).toEqual([
      "trade",
      "record",
      "ingest",
      "mark",
    ]);
  });

  it("figure screens are view-scoped", () => {
    for (const s of [
      ...PERSONAL_SCREENS.filter((x) =>
        [
          "sheet",
          "pnl",
          "bridge",
          "cashflow",
          "close",
          "asof",
          "budget",
          "loans",
        ].includes(x.segment),
      ),
      ...PROJECT_SCREENS.filter((x) =>
        ["budget", "wip", "billing", "close", "asof"].includes(x.segment),
      ),
      ...INVESTMENT_SCREENS.filter((x) =>
        ["capital", "nav", "close", "asof"].includes(x.segment),
      ),
      ...OPERATING_SCREENS.filter((x) =>
        ["sheet", "pnl", "cashflow", "close", "asof", "aging"].includes(x.segment),
      ),
      ...FUND_SCREENS.filter((x) => ["close", "asof"].includes(x.segment)),
    ]) {
      expect(s.scoped).toBe(true);
      expect(s.group).toBe("book");
    }
  });
});
