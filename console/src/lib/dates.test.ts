import { describe, expect, it } from "vitest";
import { periodLabel, previousMonth } from "./dates";

describe("period chips", () => {
  it("previousMonth rolls December to the prior year", () => {
    expect(previousMonth("2026-01")).toBe("2025-12");
    expect(previousMonth("2026-03")).toBe("2026-02");
  });

  it("periodLabel names the month rather than the filter token", () => {
    expect(periodLabel("2026-03")).toBe("Mar 2026");
    expect(periodLabel("2026")).toBe("2026");
  });
});
