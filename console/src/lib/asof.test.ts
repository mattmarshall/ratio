import { describe, expect, it } from "vitest";

import {
  citeOf,
  defaultPin,
  parsePin,
  pinKey,
  restated,
  washCites,
} from "./asof";
import type { NavStrike, PeriodClose } from "@/wire/types";

const close = (day: string, prefix: string): PeriodClose => ({
  name: `funds/demo/views/book/periodCloses/${day}`,
  view: "book",
  closedDate: {
    year: Number(day.slice(0, 4)),
    month: Number(day.slice(5, 7)),
    day: Number(day.slice(8, 10)),
  },
  journalPosition: prefix,
  journalDigest: "abcd1234deadbeef",
  configDigest: "9f2c1ab7de40551c",
  closingEntry: "c1",
  actor: "e.marsh",
  createTime: "2026-03-31T18:00:00Z",
  equityDestination: "25",
  surplus: "100",
});

const strike = (id: string, extras: Partial<NavStrike> = {}): NavStrike => ({
  name: `funds/demo/views/book/navStrikes/${id}`,
  view: "book",
  valuationTime: "2026-02-26T21:00:00Z",
  actor: "e.marsh",
  journalPosition: "6",
  journalDigest: "4b81d0c7aa19e3f2",
  netAssetValue: "10000",
  trialBalanceDifference: "0",
  configDigest: "9f2c1ab7de40551c",
  qualification: [],
  washQualified: false,
  washRestatementOriginal: "",
  washRestatementMovedTo: "",
  ...extras,
});

describe("a point-in-time pin", () => {
  it("reads now, a close, and a strike, and refuses an unknown shape", () => {
    expect(parsePin(undefined)).toBeNull();
    expect(parsePin("")).toBeNull();
    expect(parsePin("now")).toEqual({ kind: "now" });
    expect(parsePin("close:2026-03-31")).toEqual({
      kind: "close",
      id: "2026-03-31",
    });
    expect(parsePin("strike:2026-02-26")).toEqual({
      kind: "strike",
      id: "2026-02-26",
    });
    expect(parsePin("head")).toBeNull();
    expect(pinKey({ kind: "now" })).toBe("now");
    expect(pinKey({ kind: "close", id: "2026-03-31" })).toBe("close:2026-03-31");
  });

  it("defaults to the covering close, and to now when none covers", () => {
    expect(defaultPin([close("2026-03-31", "3")], "2026-03")).toEqual({
      kind: "close",
      id: "2026-03-31",
    });
    expect(defaultPin([], "2026-03")).toEqual({ kind: "now" });
    expect(defaultPin([close("2026-01-31", "2")], "2026-03")).toEqual({
      kind: "now",
    });
  });
});

describe("a pinned prefix cite", () => {
  it("leaves digest and config unset on the maintained fold", () => {
    const c = citeOf({ kind: "now" }, "6", [], [], "2026-03");
    expect(c.kind).toBe("now");
    expect(c.journalPosition).toBe("6");
    expect(c.journalDigest).toBeNull();
    expect(c.configDigest).toBeNull();
    expect(c.label).toContain("not a pinned strike");
  });

  it("does not invent a prefix when the journal is empty", () => {
    const c = citeOf({ kind: "now" }, "0", [], [], "2026-03");
    expect(c.kind).toBe("unset");
    expect(c.journalPosition).toBeNull();
    expect(c.journalDigest).toBeNull();
    expect(c.configDigest).toBeNull();
    expect(c.label).toBe("unset — no pinned prefix");
  });

  it("cites a close's prefix and digest, and stays unset when the close is absent", () => {
    const listed = [close("2026-03-31", "3")];
    const c = citeOf(
      { kind: "close", id: "2026-03-31" },
      "6",
      listed,
      [],
      "2026-03",
    );
    expect(c.kind).toBe("close");
    expect(c.journalPosition).toBe("3");
    expect(c.journalDigest).toBe("abcd1234deadbeef");
    expect(c.configDigest).toBe("9f2c1ab7de40551c");
    const missing = citeOf(
      { kind: "close", id: "2026-04-30" },
      "6",
      listed,
      [],
      "2026-04",
    );
    expect(missing.kind).toBe("unset");
    expect(missing.journalDigest).toBeNull();
    expect(missing.label).toBe("unset — no close pins this prefix");
  });

  it("cites a strike's prefix and digest without rewriting the NAV", () => {
    const s = strike("2026-02-26");
    const c = citeOf({ kind: "strike", id: "2026-02-26" }, "6", [], [s], "2026-02");
    expect(c.kind).toBe("strike");
    expect(c.journalPosition).toBe("6");
    expect(c.journalDigest).toBe("4b81d0c7aa19e3f2");
    expect(c.configDigest).toBe("9f2c1ab7de40551c");
    expect(s.netAssetValue).toBe("10000");
  });
});

describe("a WashRestatement cite", () => {
  it("stays unset when no strike was restated", () => {
    const rows = washCites([strike("2026-02-26")]);
    expect(rows).toEqual([
      {
        strikeId: "2026-02-26",
        qualified: false,
        original: null,
        movedTo: null,
      },
    ]);
    expect(restated(rows[0]!)).toBe(false);
  });

  it("cites the original and the moved figure, not a rewritten NAV", () => {
    const s = strike("2026-06-20", {
      washQualified: true,
      washRestatementOriginal: "1000",
      washRestatementMovedTo: "600",
      netAssetValue: "13443918751",
    });
    const rows = washCites([s]);
    expect(restated(rows[0]!)).toBe(true);
    expect(rows[0]!.original).toBe("1000");
    expect(rows[0]!.movedTo).toBe("600");
    expect(s.netAssetValue).toBe("13443918751");
  });
});
