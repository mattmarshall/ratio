import { describe, expect, it } from "vitest";

import { sheetFoots, sheetTotals, shown } from "./statement";
import type { Account } from "@/wire/types";

// ⛔ INTEGER STRINGS, AND THE IDENTITY THAT MAKES A SHEET HONEST.
// A = L + E + surplus in the credit-normal reading is A + L + E + (I+X) = 0
// in raw debit-minus-credit. A formatter that summed with `Number` or a
// surplus that dropped income would still look like a balance sheet.

function acct(type: Account["type"], balance: string): Account {
  return {
    name: `funds/household/views/book/accounts/${type}`,
    displayName: type,
    dimension: "0",
    type,
    debit: "0",
    credit: "0",
    balance,
    abnormal: false,
    postingCount: "1",
    currencyTotals: [],
  };
}

describe("a household statement", () => {
  it("foots when assets equal liabilities, equity and surplus", () => {
    // Cash 100.00, a card 30.00, opening equity 50.00, income 40.00, spend 20.00.
    const t = sheetTotals([
      acct("ASSET", "10000"),
      acct("LIABILITY", "-3000"),
      acct("EQUITY", "-5000"),
      acct("REVENUE", "-4000"),
      acct("EXPENSE", "2000"),
    ]);
    expect(t.surplus).toBe(-2000n);
    expect(sheetFoots(t)).toBe(true);
    expect(shown("asset", t.assets)).toBe("100.00");
    expect(shown("liability", t.liabilities)).toBe("30.00");
    expect(shown("equity", t.equity)).toBe("50.00");
    expect(shown("income", t.income)).toBe("40.00");
    expect(shown("expense", t.expenses)).toBe("20.00");
    expect(shown("equity", t.surplus)).toBe("20.00");
  });

  it("does not foot when a residual is missing", () => {
    // ⛔ DROP THE SURPLUS AND THE SHEET STILL LOOKS PLAUSIBLE. Assets 100,
    // liabilities 30, equity 50 — a reader who never sees income/expense
    // would think it ties. The identity includes the residual.
    const t = sheetTotals([
      acct("ASSET", "10000"),
      acct("LIABILITY", "-3000"),
      acct("EQUITY", "-5000"),
    ]);
    expect(t.surplus).toBe(0n);
    expect(sheetFoots(t)).toBe(false);
  });
});
