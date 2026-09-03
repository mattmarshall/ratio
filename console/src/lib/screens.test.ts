import { describe, expect, it } from "vitest";
import {
  defaultScreen,
  PERSONAL_SCREENS,
  screensFor,
  ticketsFor,
} from "./screens";

describe("screensFor", () => {
  it("a personal book is not offered Exceptions or NAV", () => {
    const labels = screensFor("PERSONAL").map((s) => s.label);
    expect(labels).toContain("Budget vs actual");
    expect(labels).toContain("Trial balance");
    expect(labels).not.toContain("Exceptions");
    expect(labels).not.toContain("NAV");
    expect(labels).not.toContain("Positions");
    expect(defaultScreen("PERSONAL")).toBe("budget");
  });

  it("a project or investment book is not forced onto household budget routes", () => {
    for (const kind of ["PROJECT", "INVESTMENT"] as const) {
      const labels = screensFor(kind).map((s) => s.label);
      expect(labels).toContain("Exceptions");
      expect(labels).toContain("NAV");
      expect(labels).not.toContain("Budget vs actual");
      expect(ticketsFor(kind).map((t) => t.segment)).toContain("trade");
    }
    expect(ticketsFor("PERSONAL").map((t) => t.segment)).toEqual([
      "record",
      "ingest",
    ]);
  });

  it("household budget is view-scoped", () => {
    const budget = PERSONAL_SCREENS.find((s) => s.segment === "budget");
    expect(budget?.scoped).toBe(true);
    expect(budget?.group).toBe("book");
  });
});
