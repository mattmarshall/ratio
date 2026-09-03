import { describe, expect, it } from "vitest";
import { periodLabel, previousMonth, utcMonth, utcYear } from "./dates";

describe("period chips", () => {
  it("labels a month and a year without inventing a timezone", () => {
    expect(periodLabel("2026-03")).toBe("Mar 2026");
    expect(periodLabel("2026")).toBe("2026");
    expect(periodLabel("soon")).toBe("soon");
  });

  it("steps a month on the digits, not through Date", () => {
    expect(previousMonth("2026-03")).toBe("2026-02");
    expect(previousMonth("2026-01")).toBe("2025-12");
  });

  it("utcMonth and utcYear are YYYY-MM / YYYY", () => {
    expect(utcMonth(new Date("2026-09-03T12:00:00Z"))).toBe("2026-09");
    expect(utcYear(new Date("2026-09-03T12:00:00Z"))).toBe("2026");
  });
});
