import { describe, expect, it } from "vitest";
import {
  beginningOf,
  cashFlowStatement,
  cashFrom,
  cashShown,
  isCashAccount,
} from "./cashflow";
import type { Account, LoanSchedule } from "@/wire/types";

const acct = (
  dimension: string,
  displayName: string,
  type: Account["type"],
  debit: string,
  credit: string,
  balance: string,
): Account => ({
  name: `funds/household/views/book/accounts/${dimension}`,
  displayName,
  dimension,
  type,
  debit,
  credit,
  balance,
  abnormal: false,
  postingCount: "1",
  currencyTotals: [],
});

describe("a household cash-flow statement", () => {
  it("leaves beginning and ending unset on a chart that never moved", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "0"),
      acct("2", "Investments", "ASSET", "0", "0", "0"),
      acct("30", "Income", "REVENUE", "0", "0", "0"),
      acct("10", "Living expenses", "EXPENSE", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
    ];
    const r = cashFlowStatement(accounts, []);
    expect(r.beginning).toBeNull();
    expect(r.ending).toBeNull();
    expect(r.delta).toBeNull();
    expect(r.operating).toBeNull();
    expect(r.investing).toBeNull();
    expect(r.financing).toBeNull();
    expect(r.income).toBeNull();
    expect(r.expense).toBeNull();
    expect(r.principalPaid).toBeNull();
    expect(r.drawn).toBeNull();
    expect(r.transfers).toBeNull();
    expect(r.creditCards).toBeNull();
    expect(r.assetPurchases).toBeNull();
    expect(r.residual).toBeNull();
    expect(cashShown(r.beginning)).toBe("—");
    expect(cashShown(0n)).toBe("0.00");
  });

  it("treats origination cash as a real beginning, not unset", () => {
    // February: Dr cash 100_000 / Cr mortgage 100_000. March is empty.
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "10000000"),
      acct("41", "Mortgage", "LIABILITY", "0", "0", "-10000000"),
      acct("30", "Income", "REVENUE", "0", "0", "0"),
      acct("10", "Living expenses", "EXPENSE", "0", "0", "0"),
      acct("2", "Investments", "ASSET", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
      acct("20", "Opening equity", "EQUITY", "0", "0", "0"),
    ];
    const r = cashFlowStatement(accounts, [{ dimension: "41", interest: "12" }]);
    expect(r.beginning).toBe(10_000_000n);
    expect(r.ending).toBe(10_000_000n);
    expect(r.delta).toBe(0n);
    expect(r.operating).toBe(0n);
    expect(r.investing).toBe(0n);
    expect(r.financing).toBe(0n);
    expect(r.residual).toBe(0n);
    expect(cashShown(r.beginning)).toBe("100,000.00");
  });

  it("ties beginning plus ops/invest/financing to ending cash", () => {
    // Beginning cash 100_000. March: income 30, living 6, interest 2,
    // principal 8, xfer 5. Ending cash 100_009.
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "3000", "2100", "10000900"),
      acct("2", "Investments", "ASSET", "500", "0", "500"),
      acct("41", "Mortgage", "LIABILITY", "800", "0", "-9999200"),
      acct("30", "Income", "REVENUE", "0", "3000", "-3000"),
      acct("10", "Living expenses", "EXPENSE", "600", "0", "600"),
      acct("12", "Mortgage interest", "EXPENSE", "200", "0", "200"),
      acct("20", "Opening equity", "EQUITY", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
    ];
    const loans: LoanSchedule[] = [{ dimension: "41", interest: "12" }];
    const r = cashFlowStatement(accounts, loans);

    expect(isCashAccount(accounts[0]!)).toBe(true);
    expect(beginningOf(accounts[0]!)).toBe(10_000_000n);
    expect(r.beginning).toBe(10_000_000n);
    expect(r.ending).toBe(10_000_900n);
    expect(r.delta).toBe(900n);
    expect(r.income).toBe(3_000n);
    expect(r.expense).toBe(-800n);
    expect(r.creditCards).toBe(0n);
    expect(r.operating).toBe(2_200n);
    expect(r.transfers).toBe(-500n);
    expect(r.investing).toBe(-500n);
    expect(r.assetPurchases).toBeNull();
    expect(r.principalPaid).toBe(800n);
    expect(r.drawn).toBe(0n);
    expect(r.equity).toBe(0n);
    expect(r.financing).toBe(-800n);
    expect(r.unclassified).toEqual([]);
    expect(r.residual).toBe(0n);

    // ⭐ THE TIE. Beginning + classified movement = ending.
    expect(r.beginning! + r.operating! + r.investing! + r.financing!).toBe(
      r.ending,
    );

    // ⛔ ADDING PRINCIPAL TO OPERATING IS THE DEFECT. Principal is financing.
    expect(r.operating).not.toBe(r.operating! - r.principalPaid!);
    expect(cashShown(r.delta)).toBe("9.00");
    expect(cashShown(r.operating)).toBe("22.00");
    expect(cashShown(r.transfers)).toBe("-5.00");
    expect(cashShown(r.financing)).toBe("-8.00");
    expect(cashShown(r.assetPurchases)).toBe("—");
  });

  it("a card charge is not a cash operating outflow", () => {
    // Beginning cash 100_000 from February origination. March: living 6
    // on a card — expense up, cards up, cash still.
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "10000000"),
      acct("10", "Living expenses", "EXPENSE", "600", "0", "600"),
      acct("40", "Credit cards", "LIABILITY", "0", "600", "-600"),
      acct("30", "Income", "REVENUE", "0", "0", "0"),
      acct("2", "Investments", "ASSET", "0", "0", "0"),
      acct("20", "Opening equity", "EQUITY", "0", "0", "0"),
    ];
    const r = cashFlowStatement(accounts, []);
    expect(r.beginning).toBe(10_000_000n);
    expect(r.ending).toBe(10_000_000n);
    expect(r.expense).toBe(-600n);
    expect(r.creditCards).toBe(600n);
    expect(r.operating).toBe(0n);
    expect(r.residual).toBe(0n);
    expect(r.principalPaid).toBeNull();
  });

  it("names unnamed loan activity instead of absorbing it into financing", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "800", "-800"),
      acct("41", "Mortgage", "LIABILITY", "800", "0", "800"),
      acct("10", "Living expenses", "EXPENSE", "0", "0", "0"),
    ];
    const r = cashFlowStatement(accounts, []);
    expect(r.ending).toBe(-800n);
    expect(r.beginning).toBeNull();
    expect(r.principalPaid).toBeNull();
    expect(r.financing).toBe(0n);
    expect(r.unclassified).toEqual([
      { dimension: "41", displayName: "Mortgage", cash: -800n },
    ]);
    expect(r.residual).toBeNull();
  });

  it("leaves transfers unset when the chart has no Investments account", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "1000", "0", "1000"),
      acct("30", "Income", "REVENUE", "0", "1000", "-1000"),
    ];
    const r = cashFlowStatement(accounts, []);
    expect(r.ending).toBe(1_000n);
    expect(r.transfers).toBeNull();
    expect(r.investing).toBe(0n);
    expect(r.creditCards).toBeNull();
    expect(r.assetPurchases).toBeNull();
    expect(r.income).toBe(1_000n);
    expect(r.operating).toBe(1_000n);
    expect(cashFrom(accounts[1]!)).toBe(1_000n);
  });
});
