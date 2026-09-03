import { describe, expect, it } from "vitest";
import type { Account } from "@/wire/types";
import {
  activityOf,
  bookCapital,
  endingCapital,
  identityHolds,
  isCapitalAccount,
  partnersOf,
} from "./capital";

function acct(
  displayName: string,
  debit: string,
  credit: string,
): Account {
  return {
    name: `funds/partners/views/book/accounts/${displayName}`,
    displayName,
    dimension: "0",
    type: "EQUITY",
    debit,
    credit,
    balance: String(BigInt(debit) - BigInt(credit)),
    abnormal: false,
    postingCount: "1",
    currencyTotals: [],
  };
}

describe("capital activity", () => {
  it("treats partner and contribution equity as capital, not unrealized gain", () => {
    expect(isCapitalAccount("Partner capital — LP")).toBe(true);
    expect(isCapitalAccount("Capital contributions")).toBe(true);
    expect(isCapitalAccount("Distributions")).toBe(true);
    expect(isCapitalAccount("Unrealized gain")).toBe(false);
    expect(isCapitalAccount("Investments at fair value")).toBe(false);
  });

  it("ending capital is credit-normal, not the debit-credit balance", () => {
    // 100.00 contributed, 25.00 distributed → 75.00 ending.
    const lp = acct("Partner capital — LP", "2500", "10000");
    expect(endingCapital(lp)).toBe(7500n);
    expect(lp.balance).toBe("-7500");
  });

  it("book capital is partners plus unallocated activity, and nothing else", () => {
    const accounts = [
      acct("Partner capital — LP", "0", "6000"),
      acct("Partner capital — GP", "0", "4000"),
      acct("Capital contributions", "0", "0"),
      acct("Unrealized gain", "0", "5000"),
    ];
    expect(partnersOf(accounts)).toHaveLength(2);
    expect(activityOf(accounts)).toHaveLength(1);
    expect(bookCapital(accounts)).toBe(10000n);
    expect(identityHolds(accounts)).toBe(true);
    // ⛔ SABOTAGE: dropping a partner keeps conservation of the remainder
    // and would stay green if the test only summed what it was handed.
    const withoutGp = accounts.filter((a) => a.displayName !== "Partner capital — GP");
    expect(bookCapital(withoutGp)).toBe(6000n);
    expect(bookCapital(withoutGp) === 10000n).toBe(false);
  });
});
