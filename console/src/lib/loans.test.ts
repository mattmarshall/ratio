import { describe, expect, it } from "vitest";
import { debitShown, liabilityShown, loanRollup } from "./loans";
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

describe("a loan roll-forward is the journal against named liabilities", () => {
  it("keys each row by liability dimension, not a single debt bucket", () => {
    const loans: LoanSchedule[] = [
      { dimension: "41", interest: "12" },
      { dimension: "42", interest: "13" },
    ];
    // Ending mortgage 95_000 credit-normal → stored −9_500_000.
    // March principal 5_000 debit, so beginning stored = −10_000_000.
    const accounts: Account[] = [
      acct("41", "Mortgage", "LIABILITY", "500000", "0", "-9500000"),
      acct("12", "Mortgage interest", "EXPENSE", "20000", "0", "20000"),
      acct("42", "Auto loan", "LIABILITY", "35000", "0", "-1765000"),
      acct("13", "Auto loan interest", "EXPENSE", "4500", "0", "4500"),
      // Credit cards moved this month and are NOT a declared loan.
      acct("40", "Credit cards", "LIABILITY", "8900", "0", "-8900"),
    ];
    const r = loanRollup(accounts, loans);
    expect(r.rows.map((row) => row.dimension)).toEqual(["41", "42"]);
    expect(r.rows.map((row) => row.displayName)).toEqual([
      "Mortgage",
      "Auto loan",
    ]);
    expect(r.rows[0]!.beginning).toBe(-10_000_000n);
    expect(r.rows[0]!.principalPaid).toBe(500_000n);
    expect(r.rows[0]!.interestPaid).toBe(20_000n);
    expect(r.rows[0]!.ending).toBe(-9_500_000n);
    expect(r.rows[1]!.beginning).toBe(-1_800_000n);
    expect(r.rows[1]!.principalPaid).toBe(35_000n);
    expect(liabilityShown(r.rows[0]!.beginning)).toBe("100,000.00");
    expect(debitShown(r.rows[0]!.interestPaid)).toBe("200.00");
  });

  it("does not invent a row for a liability nobody named", () => {
    const accounts: Account[] = [
      acct("41", "Mortgage", "LIABILITY", "0", "0", "0"),
      acct("40", "Credit cards", "LIABILITY", "0", "0", "0"),
    ];
    expect(loanRollup(accounts, []).rows).toEqual([]);
  });

  it("treats a configured loan with no activity as a real zero, not as unset", () => {
    const loans: LoanSchedule[] = [{ dimension: "41", interest: "12" }];
    const accounts: Account[] = [
      acct("41", "Mortgage", "LIABILITY", "0", "0", "-25000000"),
      acct("12", "Mortgage interest", "EXPENSE", "0", "0", "0"),
    ];
    const r = loanRollup(accounts, loans);
    expect(r.rows).toHaveLength(1);
    expect(r.rows[0]!.beginning).toBe(-25_000_000n);
    expect(r.rows[0]!.principalPaid).toBe(0n);
    expect(r.rows[0]!.interestPaid).toBe(0n);
    expect(r.rows[0]!.ending).toBe(-25_000_000n);
  });

  it("omits a draw of nothing and reports a draw that happened", () => {
    const loans: LoanSchedule[] = [{ dimension: "41", interest: "12" }];
    const accounts: Account[] = [
      acct("41", "Mortgage", "LIABILITY", "500000", "1000000", "-10000000"),
      acct("12", "Mortgage interest", "EXPENSE", "0", "0", "0"),
    ];
    const r = loanRollup(accounts, loans);
    // ending − (debit − credit) = −10_000_000 − (500_000 − 1_000_000)
    // = −10_000_000 − (−500_000) = −9_500_000. Drew 10_000, paid 5_000
    // principal, so the book started at 95_000 not 105_000.
    expect(r.rows[0]!.beginning).toBe(-9_500_000n);
    expect(r.rows[0]!.drawn).toBe(1_000_000n);
    expect(r.rows[0]!.principalPaid).toBe(500_000n);
  });
});
