import { describe, expect, it } from "vitest";

import { periodLabel, previousMonth, utcMonth, utcYear } from "./dates";

describe("a calendar period", () => {
  it("names a UTC month and year from a Date without a timezone shift", () => {
    // ⛔ A LOCAL DATE WOULD MOVE THE MONTH. `2026-09-01T00:30Z` is 31 Aug
    // in US Pacific; a household figure that cited August for a September
    // payment would be the wrong window wearing the right URL.
    const d = new Date("2026-09-01T00:30:00Z");
    expect(utcMonth(d)).toBe("2026-09");
    expect(utcYear(d)).toBe("2026");
  });

  it("names a month without turning it into an instant", () => {
    expect(periodLabel("2026-09")).toBe("Sep 2026");
    expect(periodLabel("2026-03")).toBe("Mar 2026");
    expect(periodLabel("2026")).toBe("2026");
    expect(periodLabel("soon")).toBe("soon");
  });

  it("steps to the previous month across a year boundary", () => {
    expect(previousMonth("2026-09")).toBe("2026-08");
    expect(previousMonth("2026-03")).toBe("2026-02");
    expect(previousMonth("2026-01")).toBe("2025-12");
  });
});
