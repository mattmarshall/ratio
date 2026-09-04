import { describe, expect, it } from "vitest";

import { elapsed, listedCrumb, nanos, reads } from "./format";

// The three formatters that carry the plan screen's one load-bearing
// distinction: a figure nothing produced is NOT a figure of zero.
//
// ⛔ THIS FILE EXISTS BECAUSE A SABOTAGE RUN FOUND THE GAP. `screens.test.tsx`
// asserts the words "not measured" appear, and those come from `analyzed` being
// false — so breaking `elapsed` to render `null` as `0 ns` left the whole render
// suite green. The render tests check that the page SAYS it is unmeasured; these
// check that the figures do not quietly contradict it.

describe("a duration that may be absent", () => {
  it("renders nothing measured as a dash, and a real zero as a zero", () => {
    // ⛔ THE ONE THAT WENT UNGUARDED. `0 ns` reads as "instant"; `—` reads as
    // "nobody looked". A step of an unanalyzed plan is the second, always.
    expect(elapsed(null)).toBe("—");
    expect(elapsed("0s")).toBe("0 ns");
    expect(elapsed("0.000000000s")).toBe("0 ns");
  });

  it("reads a proto3 Duration at the scale a period end actually lands in", () => {
    // ⚠ NOT ALWAYS MILLISECONDS. A small strike lands in microseconds and
    // printing that as `0 ms` reads as broken — `human_nanos` in the Rust says
    // the same thing for the same reason.
    expect(elapsed("0.000004436s")).toBe("4 µs");
    expect(elapsed("0.012500000s")).toBe("12.5 ms");
    expect(elapsed("8.600000000s")).toBe("8.6 s");
    expect(elapsed("0.000000412s")).toBe("412 ns");
  });

  it("refuses a duration it cannot read rather than printing a guess", () => {
    expect(elapsed("nonsense")).toBe("—");
    expect(elapsed("-1s")).toBe("—");
  });
});

describe("a count of reads", () => {
  it("keeps an absent count and a proved zero apart", () => {
    // ⛔ The zero is `Ratio.Closure.factored_nav_never_reads_the_lots` — twenty
    // million tax lots costing nothing. Rendering it as `—` deletes a theorem;
    // rendering an absent count as `0` invents one.
    expect(reads("")).toBe("—");
    expect(reads("0")).toBe("0");
    expect(reads("252843")).toBe("252,843");
  });
});

describe("the collection crumb", () => {
  it("names how many the list returned, and does not print / 0 over none", () => {
    expect(listedCrumb("Books", 5)).toEqual({ noun: "Books", n: "5" });
    expect(listedCrumb("Books", 0)).toEqual({ noun: "Books", n: null });
  });
});

describe("a rate, which is not a duration", () => {
  it("reads nanoseconds per read at the scale the calibration lands in", () => {
    // `nanosPerRead` is an int64 like every other count on that message: ns per
    // read is not a length of time, so `elapsed` does not apply to it.
    expect(nanos("4436")).toBe("4 µs");
    expect(nanos("")).toBe("—");
    expect(nanos("250")).toBe("250 ns");
  });
});
