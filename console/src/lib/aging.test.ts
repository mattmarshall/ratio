import { describe, expect, it } from "vitest";
import {
  agingShown,
  bucketOf,
  parseBucket,
  scheduleFoots,
  scheduleIsSet,
} from "./aging";
import type { AgingSchedule } from "@/wire/types";

const empty: AgingSchedule = {
  current: "",
  daysThirty: "",
  daysSixty: "",
  daysNinety: "",
  daysOlder: "",
  undated: "",
  control: "",
};

const cited: AgingSchedule = {
  current: "30000",
  daysThirty: "20000",
  daysSixty: "0",
  daysNinety: "0",
  daysOlder: "0",
  undated: "",
  control: "50000",
};

describe("parseBucket", () => {
  it("empty is unset, not a measured zero", () => {
    expect(parseBucket("")).toBeNull();
    expect(parseBucket("0")).toBe(0n);
    expect(parseBucket("40000")).toBe(40000n);
  });
});

describe("bucketOf", () => {
  it("a null due date is undated, never current", () => {
    expect(bucketOf(null, "2026-04-15")).toBe("undated");
    expect(bucketOf("", "2026-04-15")).toBe("undated");
    expect(bucketOf(null, "2026-04-15")).not.toBe("current");
  });

  it("places a dated item in the window the as-of day names", () => {
    expect(bucketOf("2026-04-20", "2026-04-15")).toBe("current");
    expect(bucketOf("2026-04-15", "2026-04-15")).toBe("current");
    expect(bucketOf("2026-03-16", "2026-04-15")).toBe("daysThirty");
    expect(bucketOf("2026-03-15", "2026-04-15")).toBe("daysSixty");
    expect(bucketOf("2026-02-13", "2026-04-15")).toBe("daysNinety");
    expect(bucketOf("2026-01-14", "2026-04-15")).toBe("daysOlder");
  });
});

describe("scheduleFoots", () => {
  it("an unset schedule does not pretend to foot", () => {
    expect(scheduleIsSet(empty)).toBe(false);
    expect(scheduleFoots(empty)).toBe(false);
    expect(scheduleFoots(null)).toBe(false);
  });

  it("dated buckets plus residual equal the control", () => {
    expect(scheduleIsSet(cited)).toBe(true);
    expect(scheduleFoots(cited)).toBe(true);
    expect(
      scheduleFoots({
        ...cited,
        undated: "10000",
        control: "60000",
      }),
    ).toBe(true);
    expect(scheduleFoots({ ...cited, control: "40000" })).toBe(false);
  });

  it("a fully collected book is a real-zero schedule that foots", () => {
    const zero: AgingSchedule = {
      current: "0",
      daysThirty: "0",
      daysSixty: "0",
      daysNinety: "0",
      daysOlder: "0",
      undated: "",
      control: "0",
    };
    expect(scheduleIsSet(zero)).toBe(true);
    expect(scheduleFoots(zero)).toBe(true);
  });
});

describe("agingShown", () => {
  it("unset is an em dash, a real zero is 0.00", () => {
    expect(agingShown(null, "receivable")).toBe("—");
    expect(agingShown(0n, "receivable")).toBe("0.00");
    expect(agingShown(30000n, "receivable")).toBe("300.00");
    expect(agingShown(-8000n, "payable")).toBe("80.00");
  });
});
