import { describe, expect, it } from "vitest";
import { householdRollup } from "./household";
import type { Account, HouseholdEnvelope } from "@/wire/types";

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
    name: `funds/household/views/book/accounts/${dim}`,
    displayName,
    dimension: dim,
    type,
    debit,
    credit,
    balance: (d - c).toString(),
    abnormal: false,
    postingCount: "1",
    currencyTotals: [],
    units: "",
  };
}

const CHART: Account[] = [
  acct("1", "Cash and bank", "ASSET", "100000", "4000"),
  acct("10", "Living expenses", "EXPENSE", "4000", "0"),
  acct("11", "Taxes", "EXPENSE", "0", "0"),
  acct("30", "Income", "REVENUE", "0", "50000"),
];

describe("householdRollup", () => {
  it("cites spent as expense balances, not a second ledger", () => {
    const r = householdRollup(CHART, "500000", [
      { dimension: "10", budget: "400000" },
      { dimension: "11", budget: "100000" },
    ]);
    expect(r.spent).toBe(4000n);
    expect(r.baseline).toBe(500000n);
    expect(r.variance).toBe(496000n);
    const living = r.envelopes.find((e) => e.dimension === "10")!;
    expect(living.actual).toBe(4000n);
    expect(living.planned).toBe(400000n);
    expect(living.variance).toBe(396000n);
    const taxes = r.envelopes.find((e) => e.dimension === "11")!;
    expect(taxes.planned).toBe(100000n);
    expect(taxes.actual).toBe(0n);
  });

  it("treats an empty budget as unset, not a baseline of zero", () => {
    const r = householdRollup(CHART, "", []);
    expect(r.baseline).toBeNull();
    expect(r.variance).toBeNull();
    expect(r.spent).toBe(4000n);
    expect(r.envelopes.every((e) => e.planned === null)).toBe(true);
    expect(r.envelopes.every((e) => e.variance === null)).toBe(true);
  });

  it("a zero budget is a set baseline of nothing", () => {
    const r = householdRollup(CHART, "0", [{ dimension: "10", budget: "0" }]);
    expect(r.baseline).toBe(0n);
    expect(r.variance).toBe(-4000n);
    const living = r.envelopes.find((e) => e.dimension === "10")!;
    expect(living.planned).toBe(0n);
    expect(living.variance).toBe(-4000n);
    const taxes = r.envelopes.find((e) => e.dimension === "11")!;
    expect(taxes.planned).toBeNull();
  });

  it("does not invent envelopes the configuration did not declare", () => {
    const r = householdRollup(CHART, "500000", []);
    expect(r.envelopes.find((e) => e.dimension === "10")!.planned).toBeNull();
    expect(r.envelopes.find((e) => e.dimension === "11")!.planned).toBeNull();
    expect(r.baseline).toBe(500000n);
  });

  it("does not fold envelopes into a total nobody declared", () => {
    const envelopes: HouseholdEnvelope[] = [
      { dimension: "10", budget: "400000" },
    ];
    const r = householdRollup(CHART, "", envelopes);
    expect(r.baseline).toBeNull();
    expect(r.variance).toBeNull();
    expect(r.envelopes.find((e) => e.dimension === "10")!.planned).toBe(400000n);
  });
});
