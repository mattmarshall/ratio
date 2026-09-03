import { describe, expect, it } from "vitest";
import {
  defaultScreen,
  PROJECT_SCREENS,
  screensFor,
  ticketsFor,
} from "./screens";

describe("screensFor", () => {
  it("a project book is not offered Exceptions or NAV", () => {
    const labels = screensFor("PROJECT").map((s) => s.label);
    expect(labels).toContain("Budget vs actual");
    expect(labels).toContain("WIP");
    expect(labels).toContain("Trial balance");
    expect(labels).not.toContain("Exceptions");
    expect(labels).not.toContain("NAV");
    expect(labels).not.toContain("Positions");
    expect(defaultScreen("PROJECT")).toBe("budget");
  });

  it("a personal or investment book is not forced onto project figure routes", () => {
    for (const kind of ["PERSONAL", "INVESTMENT"] as const) {
      const labels = screensFor(kind).map((s) => s.label);
      expect(labels).toContain("Exceptions");
      expect(labels).toContain("NAV");
      expect(labels).not.toContain("Budget vs actual");
      expect(labels).not.toContain("WIP");
      expect(ticketsFor(kind).map((t) => t.segment)).toContain("trade");
      expect(ticketsFor(kind).map((t) => t.segment)).not.toContain("budget");
    }
    expect(ticketsFor("PROJECT").map((t) => t.segment)).toEqual([
      "record",
      "ingest",
    ]);
  });

  it("project figure screens are view-scoped", () => {
    for (const s of PROJECT_SCREENS.filter((x) =>
      ["budget", "wip"].includes(x.segment),
    )) {
      expect(s.scoped).toBe(true);
      expect(s.group).toBe("book");
    }
  });
});
