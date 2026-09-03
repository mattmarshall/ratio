import { describe, expect, it } from "vitest";
import {
  beginningOf,
  cashFlowStatement,
  cashFrom,
  cashShown,
  isCashAccount,
  operatingCashFlowStatement,
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

describe("an operating-company cash-flow statement", () => {
  it("leaves beginning and ending unset on a chart that never moved", () => {
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "0", "0", "0"),
      acct("2", "Accounts receivable", "ASSET", "0", "0", "0"),
      acct("10", "Operating expenses", "EXPENSE", "0", "0", "0"),
      acct("20", "Owner equity", "EQUITY", "0", "0", "0"),
      acct("30", "Operating revenue", "REVENUE", "0", "0", "0"),
      acct("40", "Accounts payable", "LIABILITY", "0", "0", "0"),
    ];
    const r = operatingCashFlowStatement(accounts);
    expect(r.beginning).toBeNull();
    expect(r.ending).toBeNull();
    expect(r.delta).toBeNull();
    expect(r.operating).toBeNull();
    expect(r.investing).toBeNull();
    expect(r.financing).toBeNull();
    expect(r.income).toBeNull();
    expect(r.expense).toBeNull();
    expect(r.receivables).toBeNull();
    expect(r.payables).toBeNull();
    expect(r.equity).toBeNull();
    expect(r.creditCards).toBeNull();
    expect(r.transfers).toBeNull();
    expect(r.assetPurchases).toBeNull();
    expect(r.principalPaid).toBeNull();
    expect(r.drawn).toBeNull();
    expect(r.residual).toBeNull();
    expect(cashShown(r.beginning)).toBe("—");
    expect(cashShown(0n)).toBe("0.00");
  });

  it("treats a prior-period contribution as a real beginning, not unset", () => {
    // February: Dr cash 1_000 / Cr owner equity 1_000. March is empty.
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "0", "0", "100000"),
      acct("2", "Accounts receivable", "ASSET", "0", "0", "0"),
      acct("10", "Operating expenses", "EXPENSE", "0", "0", "0"),
      acct("20", "Owner equity", "EQUITY", "0", "0", "-100000"),
      acct("30", "Operating revenue", "REVENUE", "0", "0", "0"),
      acct("40", "Accounts payable", "LIABILITY", "0", "0", "0"),
    ];
    const r = operatingCashFlowStatement(accounts);
    expect(r.beginning).toBe(100_000n);
    expect(r.ending).toBe(100_000n);
    expect(r.delta).toBe(0n);
    expect(r.operating).toBe(0n);
    expect(r.investing).toBeNull();
    expect(r.financing).toBe(0n);
    expect(r.residual).toBe(0n);
    expect(cashShown(r.beginning)).toBe("1,000.00");
  });

  it("ties beginning plus ops and financing to ending cash", () => {
    // Beginning cash 1_000 from a February contribution. March: invoice 400
    // (no cash), collect 150, cash sale 100, vendor bill 80 (no cash),
    // pay vendor 30, cash expense 20, owner draw 50. Ending cash 1_150.
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "25000", "10000", "115000"),
      acct("2", "Accounts receivable", "ASSET", "40000", "15000", "25000"),
      acct("10", "Operating expenses", "EXPENSE", "10000", "0", "10000"),
      acct("20", "Owner equity", "EQUITY", "5000", "0", "-95000"),
      acct("30", "Operating revenue", "REVENUE", "0", "50000", "-50000"),
      acct("40", "Accounts payable", "LIABILITY", "3000", "8000", "-5000"),
    ];
    const r = operatingCashFlowStatement(accounts);

    expect(isCashAccount(accounts[0]!)).toBe(true);
    expect(beginningOf(accounts[0]!)).toBe(100_000n);
    expect(r.beginning).toBe(100_000n);
    expect(r.ending).toBe(115_000n);
    expect(r.delta).toBe(15_000n);
    expect(r.income).toBe(50_000n);
    expect(r.expense).toBe(-10_000n);
    expect(r.receivables).toBe(-25_000n);
    expect(r.payables).toBe(5_000n);
    expect(r.operating).toBe(20_000n);
    expect(r.investing).toBeNull();
    expect(r.assetPurchases).toBeNull();
    expect(r.transfers).toBeNull();
    expect(r.creditCards).toBeNull();
    expect(r.principalPaid).toBeNull();
    expect(r.drawn).toBeNull();
    expect(r.equity).toBe(-5_000n);
    expect(r.financing).toBe(-5_000n);
    expect(r.unclassified).toEqual([]);
    expect(r.residual).toBe(0n);

    // ⭐ THE TIE. Beginning + classified movement = ending.
    // Investing is unset — the chart has no such account — and is not
    // a silent 0.00 in the cited classes.
    expect(r.beginning! + r.operating! + r.financing!).toBe(r.ending);

    // ⛔ AN INVOICE WITHOUT COLLECTION IS NOT OPERATING CASH. AR is the plug.
    expect(r.operating).not.toBe(r.income! + r.expense!);
    expect(cashShown(r.delta)).toBe("150.00");
    expect(cashShown(r.operating)).toBe("200.00");
    expect(cashShown(r.financing)).toBe("-50.00");
    expect(cashShown(r.investing)).toBe("—");
  });

  it("an invoice is not a cash operating inflow", () => {
    // Beginning cash 1_000 from February. March: invoice 400 — revenue
    // up, AR up, cash still.
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "0", "0", "100000"),
      acct("2", "Accounts receivable", "ASSET", "40000", "0", "40000"),
      acct("30", "Operating revenue", "REVENUE", "0", "40000", "-40000"),
      acct("10", "Operating expenses", "EXPENSE", "0", "0", "0"),
      acct("20", "Owner equity", "EQUITY", "0", "0", "-100000"),
      acct("40", "Accounts payable", "LIABILITY", "0", "0", "0"),
    ];
    const r = operatingCashFlowStatement(accounts);
    expect(r.beginning).toBe(100_000n);
    expect(r.ending).toBe(100_000n);
    expect(r.income).toBe(40_000n);
    expect(r.receivables).toBe(-40_000n);
    expect(r.operating).toBe(0n);
    expect(r.residual).toBe(0n);
    expect(r.investing).toBeNull();
  });

  it("a vendor bill is not a cash operating outflow", () => {
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "0", "0", "100000"),
      acct("10", "Operating expenses", "EXPENSE", "8000", "0", "8000"),
      acct("40", "Accounts payable", "LIABILITY", "0", "8000", "-8000"),
      acct("30", "Operating revenue", "REVENUE", "0", "0", "0"),
      acct("2", "Accounts receivable", "ASSET", "0", "0", "0"),
      acct("20", "Owner equity", "EQUITY", "0", "0", "-100000"),
    ];
    const r = operatingCashFlowStatement(accounts);
    expect(r.beginning).toBe(100_000n);
    expect(r.ending).toBe(100_000n);
    expect(r.expense).toBe(-8_000n);
    expect(r.payables).toBe(8_000n);
    expect(r.operating).toBe(0n);
    expect(r.residual).toBe(0n);
  });

  it("names an unclassified move instead of inventing an investing class", () => {
    // A prepaid that chart_for(Operating) does not write. Period activity
    // is a residual line, not silent absorption into investing.
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "0", "800", "-800"),
      acct("5", "Prepaid rent", "ASSET", "800", "0", "800"),
      acct("10", "Operating expenses", "EXPENSE", "0", "0", "0"),
    ];
    const r = operatingCashFlowStatement(accounts);
    expect(r.ending).toBe(-800n);
    expect(r.beginning).toBeNull();
    expect(r.investing).toBeNull();
    expect(r.financing).toBeNull();
    expect(r.unclassified).toEqual([
      { dimension: "5", displayName: "Prepaid rent", cash: -800n },
    ]);
    expect(r.residual).toBeNull();
  });

  it("leaves financing unset when the chart has no Owner equity account", () => {
    const accounts: Account[] = [
      acct("1", "Cash", "ASSET", "1000", "0", "1000"),
      acct("30", "Operating revenue", "REVENUE", "0", "1000", "-1000"),
    ];
    const r = operatingCashFlowStatement(accounts);
    expect(r.ending).toBe(1_000n);
    expect(r.equity).toBeNull();
    expect(r.financing).toBeNull();
    expect(r.receivables).toBeNull();
    expect(r.payables).toBeNull();
    expect(r.investing).toBeNull();
    expect(r.income).toBe(1_000n);
    expect(r.operating).toBe(1_000n);
    expect(cashFrom(accounts[1]!)).toBe(1_000n);
  });
});
