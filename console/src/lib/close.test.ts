import { describe, expect, it } from "vitest";
import {
  closeRollForward,
  coveringClose,
  equityShown,
  windowEnd,
} from "./close";
import type { Account, PeriodClose } from "@/wire/types";

const acct = (
  dimension: string,
  displayName: string,
  type: Account["type"],
  debit: string,
  credit: string,
  balance: string,
  postingCount = "1",
): Account => ({
  name: `funds/household/views/book/accounts/${dimension}`,
  displayName,
  dimension,
  type,
  debit,
  credit,
  balance,
  abnormal: false,
  postingCount,
  currencyTotals: [],
  units: "",
});

const close = (surplus: string, dest = "25"): PeriodClose => ({
  name: "funds/household/views/book/periodCloses/2026-03-31",
  view: "book",
  closedDate: { year: 2026, month: 3, day: 31 },
  journalPosition: "3",
  journalDigest: "d".repeat(64),
  configDigest: "c".repeat(64),
  closingEntry: surplus === "" ? "" : "close:book:2026-03-31",
  actor: "tester",
  createTime: "2026-04-01T00:00:00Z",
  equityDestination: dest,
  surplus,
});

describe("a period close roll-forward", () => {
  it("leaves beginning, surplus and ending unset on a chart that never moved", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "0", "0"),
      acct("25", "Retained earnings", "EQUITY", "0", "0", "0", "0"),
      acct("30", "Income", "REVENUE", "0", "0", "0", "0"),
      acct("10", "Living expenses", "EXPENSE", "0", "0", "0", "0"),
    ];
    const r = closeRollForward(accounts, null);
    expect(r.beginning).toBeNull();
    expect(r.surplus).toBeNull();
    expect(r.ending).toBeNull();
    expect(r.residual).toBeNull();
    expect(r.closed).toBe(false);
    expect(equityShown(r.beginning)).toBe("—");
    expect(equityShown(0n)).toBe("0.00");
  });

  it("shows provisional surplus on an open period and leaves ending RE unset", () => {
    // March income 30 / spend 6, not yet closed. RE never moved.
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "2400", "0", "2400"),
      acct("25", "Retained earnings", "EQUITY", "0", "0", "0", "0"),
      acct("30", "Income", "REVENUE", "0", "3000", "-3000"),
      acct("10", "Living expenses", "EXPENSE", "600", "0", "600"),
    ];
    const r = closeRollForward(accounts, null);
    expect(r.beginning).toBeNull();
    expect(r.surplus).toBe(-2400n);
    expect(r.ending).toBe(0n);
    expect(r.closed).toBe(false);
    expect(r.residual).toBeNull();
    expect(equityShown(r.surplus)).toBe("24.00");
  });

  it("ties beginning + surplus to ending when the period is closed", () => {
    // Prior prefix moved cash; RE beginning is a real zero. Close posted
    // surplus −2400 to dest 25.
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "2400"),
      acct("25", "Retained earnings", "EQUITY", "0", "2400", "-2400"),
      acct("30", "Income", "REVENUE", "3000", "3000", "0"),
      acct("10", "Living expenses", "EXPENSE", "600", "600", "0"),
    ];
    const r = closeRollForward(accounts, close("-2400"));
    expect(r.beginning).toBe(0n);
    expect(r.surplus).toBe(-2400n);
    expect(r.adjustments).toBeNull();
    expect(r.ending).toBe(-2400n);
    expect(r.residual).toBe(0n);
    expect(r.closed).toBe(true);
    expect(equityShown(r.ending)).toBe("24.00");
  });

  it("does not default a missing close surplus to zero", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "500"),
      acct("2", "Investments", "ASSET", "500", "0", "500"),
      acct("25", "Retained earnings", "EQUITY", "0", "0", "0", "0"),
    ];
    const r = closeRollForward(accounts, close(""));
    expect(r.closed).toBe(true);
    expect(r.surplus).toBeNull();
    expect(equityShown(r.surplus)).toBe("—");
  });

  it("names a covering close by the window end, not by a prefix match", () => {
    const march = close("-2400");
    expect(windowEnd("2026-03")).toBe("2026-03-31");
    expect(windowEnd("2026")).toBe("2026-12-31");
    expect(coveringClose([march], "2026-03")?.closedDate).toEqual({
      year: 2026,
      month: 3,
      day: 31,
    });
    expect(coveringClose([march], "2026")).toBeNull();
  });
});
