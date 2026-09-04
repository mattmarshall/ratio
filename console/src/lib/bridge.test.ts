import { describe, expect, it } from "vitest";
import {
  beginningOf,
  debitShown,
  expenseShown,
  incomeShown,
  netWorthBridge,
  netWorthOf,
  nwShown,
} from "./bridge";
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
  units: "",
});

describe("a household net-worth bridge", () => {
  it("leaves beginning and ending unset on a chart that never moved", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "0"),
      acct("2", "Investments", "ASSET", "0", "0", "0"),
      acct("30", "Income", "REVENUE", "0", "0", "0"),
      acct("10", "Living expenses", "EXPENSE", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
    ];
    const r = netWorthBridge(accounts, []);
    expect(r.beginning).toBeNull();
    expect(r.ending).toBeNull();
    expect(r.delta).toBeNull();
    expect(r.income).toBeNull();
    expect(r.expense).toBeNull();
    expect(r.principalPaid).toBeNull();
    expect(r.transfers).toBeNull();
    expect(r.assetPurchases).toBeNull();
    expect(nwShown(r.beginning)).toBe("—");
    expect(nwShown(0n)).toBe("0.00");
  });

  it("treats an origination that nets to zero as a real beginning, not unset", () => {
    // February: Dr cash 100_000 / Cr mortgage 100_000. March is empty.
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "0", "10000000"),
      acct("41", "Mortgage", "LIABILITY", "0", "0", "-10000000"),
      acct("30", "Income", "REVENUE", "0", "0", "0"),
      acct("10", "Living expenses", "EXPENSE", "0", "0", "0"),
      acct("2", "Investments", "ASSET", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
    ];
    const r = netWorthBridge(accounts, [{ dimension: "41", interest: "12" }]);
    expect(r.beginning).toBe(0n);
    expect(r.ending).toBe(0n);
    expect(r.delta).toBe(0n);
    expect(r.income).toBe(0n);
    expect(r.expense).toBe(0n);
    expect(r.principalPaid).toBe(0n);
    expect(r.transfers).toBe(0n);
    expect(nwShown(r.beginning)).toBe("0.00");
  });

  it("cites ΔNW as income minus expenses, not principal or a transfer", () => {
    // Beginning: cash 100_000 / mortgage 100_000 (NW = 0).
    // March: income 30, living 6, interest 2, principal 8, xfer 5.
    // Ending cash 100_009, investments 5, mortgage 99_992. ΔNW = 22.
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
    const r = netWorthBridge(accounts, loans);

    expect(beginningOf(accounts[0]!)).toBe(10_000_000n);
    expect(beginningOf(accounts[2]!)).toBe(-10_000_000n);
    expect(netWorthOf(accounts, beginningOf)).toBe(0n);
    expect(r.beginning).toBe(0n);
    expect(r.ending).toBe(2_200n);
    expect(r.delta).toBe(2_200n);
    expect(r.income).toBe(-3_000n);
    expect(r.expense).toBe(800n);
    expect(r.equity).toBe(0n);
    expect(r.residual).toBe(0n);
    expect(r.principalPaid).toBe(800n);
    expect(r.transfers).toBe(500n);
    expect(r.assetPurchases).toBeNull();

    // ⛔ ADDING PRINCIPAL OR THE TRANSFER TO ΔNW IS THE DEFECT. The
    // identity is beginning + income − expenses = ending, in shown terms.
    expect(r.delta).not.toBe(r.delta! + r.principalPaid!);
    expect(r.delta).not.toBe(r.delta! + r.transfers!);
    expect(incomeShown(r.income)).toBe("30.00");
    expect(expenseShown(r.expense)).toBe("8.00");
    expect(nwShown(r.delta)).toBe("22.00");
    expect(debitShown(r.principalPaid)).toBe("8.00");
    expect(debitShown(r.transfers)).toBe("5.00");
    expect(debitShown(r.assetPurchases)).toBe("—");
  });

  it("does not invent principal when no loan is named", () => {
    const accounts: Account[] = [
      acct("1", "Cash and bank", "ASSET", "0", "4000", "-4000"),
      acct("10", "Living expenses", "EXPENSE", "4000", "0", "4000"),
      acct("41", "Mortgage", "LIABILITY", "0", "0", "0"),
      acct("2", "Investments", "ASSET", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
    ];
    const r = netWorthBridge(accounts, []);
    expect(r.ending).toBe(-4_000n);
    expect(r.beginning).toBeNull();
    expect(r.delta).toBeNull();
    expect(r.expense).toBe(4_000n);
    expect(r.principalPaid).toBeNull();
    expect(r.residual).toBeNull();
  });

  it("leaves transfers unset when the chart has no transfer accounts", () => {
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "1000", "0", "1000"),
      acct("30", "Income", "REVENUE", "0", "1000", "-1000"),
    ];
    const r = netWorthBridge(accounts, []);
    expect(r.ending).toBe(1_000n);
    expect(r.transfers).toBeNull();
    expect(r.assetPurchases).toBeNull();
  });
});
