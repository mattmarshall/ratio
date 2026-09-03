import { describe, expect, it } from "vitest";
import { figure, projectRollup, wipFoots } from "./project";
import type { Account } from "@/wire/types";

function acct(
  dim: string,
  displayName: string,
  type: Account["type"],
  debit: string,
  credit: string,
): Account {
  const d = BigInt(debit);
  const c = BigInt(credit);
  return {
    name: `funds/bridge/views/book/accounts/${dim}`,
    displayName,
    dimension: dim,
    type,
    debit,
    credit,
    balance: (d - c).toString(),
    abnormal: false,
    postingCount: "1",
    currencyTotals: [],
  };
}

const CHART: Account[] = [
  acct("1", "Cash", "ASSET", "500000", "200000"),
  acct("2", "Work in progress", "ASSET", "400000", "100000"),
  acct("10", "Project costs", "EXPENSE", "700000", "400000"),
  acct("20", "Funding", "EQUITY", "0", "800000"),
  acct("30", "Project revenue", "REVENUE", "0", "150000"),
  acct("40", "Payables", "LIABILITY", "0", "200000"),
];

describe("projectRollup", () => {
  it("cites incurred as costs plus WIP so recognizing does not double-count", () => {
    // costs.balance 3000.00 + wip.balance 3000.00 = 6000.00 incurred.
    // Using costs.debit (7000.00) would count capitalized amounts twice
    // after recognize_wip credited costs and then debited them again.
    const r = projectRollup(CHART, "10000000");
    expect(r.costs).toBe(300000n);
    expect(r.wip).toBe(300000n);
    expect(r.incurred).toBe(600000n);
    expect(r.payables).toBe(-200000n);
    expect(r.committed).toBe(800000n);
    expect(r.baseline).toBe(10000000n);
    expect(r.variance).toBe(9200000n);
    expect(r.funding).toBe(-800000n);
    expect(r.revenue).toBe(-150000n);
  });

  it("treats an empty budget as unset, not a baseline of zero", () => {
    const r = projectRollup(CHART, "");
    expect(r.baseline).toBeNull();
    expect(r.variance).toBeNull();
    expect(r.incurred).toBe(600000n);
  });

  it("a zero budget is a set baseline of nothing", () => {
    const r = projectRollup(CHART, "0");
    expect(r.baseline).toBe(0n);
    expect(r.variance).toBe(-800000n);
  });

  it("WIP debit equals currently capitalized plus recognized", () => {
    const r = projectRollup(CHART, "");
    expect(r.wipDebit).toBe(400000n);
    expect(r.wipCredit).toBe(100000n);
    expect(wipFoots(r)).toBe(true);
  });

  it("does not invent accounts the chart does not have", () => {
    const r = projectRollup([], "100");
    expect(r.incurred).toBe(0n);
    expect(r.committed).toBe(0n);
    expect(r.baseline).toBe(100n);
    expect(r.variance).toBe(100n);
  });
});

describe("project figures", () => {
  it("renders unset as a dash and zero as zero", () => {
    expect(figure("")).toBe("—");
    expect(figure("0")).toBe("0.00");
    expect(figure("10000")).toBe("100.00");
    expect(figure("-20000")).toBe("-200.00");
  });
});
