import { describe, expect, it } from "vitest";

import { periodLabel, previousMonth } from "./dates";

describe("a calendar period", () => {
  it("names a month without turning it into an instant", () => {
    expect(periodLabel("2026-09")).toBe("Sep 2026");
    expect(periodLabel("2026")).toBe("2026");
    expect(periodLabel("soon")).toBe("soon");
  });

  it("steps to the previous month across a year boundary", () => {
    expect(previousMonth("2026-09")).toBe("2026-08");
    expect(previousMonth("2026-01")).toBe("2025-12");
  });
});
