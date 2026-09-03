import { describe, expect, it } from "vitest";
import type { Account } from "@/wire/types";
import {
  activityOf,
  bookCapital,
  commitmentIdentityHolds,
  endingCapital,
  identityHolds,
  isCapitalAccount,
  isCommitmentAccount,
  isPosted,
  isUndrawnAccount,
  partnersOf,
  remainingCommitment,
  remainingUndrawn,
  undrawnFigure,
} from "./capital";

function acct(
  displayName: string,
  debit: string,
  credit: string,
  postingCount = "1",
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
    postingCount,
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
    expect(isCapitalAccount("Commitments — LP")).toBe(false);
    expect(isCapitalAccount("Undrawn commitments — GP")).toBe(false);
  });

  it("treats commitment and undrawn as a pair, not funded capital", () => {
    expect(isCommitmentAccount("Commitments — LP")).toBe(true);
    expect(isCommitmentAccount("Commitments — GP")).toBe(true);
    expect(isUndrawnAccount("Undrawn commitments — LP")).toBe(true);
    expect(isCommitmentAccount("Partner capital — LP")).toBe(false);
    expect(isUndrawnAccount("Capital contributions")).toBe(false);
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
      acct("Capital contributions", "0", "0", "0"),
      acct("Unrealized gain", "0", "5000"),
      acct("Commitments — LP", "0", "10000"),
      acct("Undrawn commitments — LP", "10000", "0"),
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
    // A commitment is not money that arrived.
    expect(bookCapital(accounts)).not.toBe(20000n);
  });

  it("undrawn is unset when no commitment has been posted, not a callable zero", () => {
    const unset = [
      acct("Partner capital — LP", "0", "10000"),
      acct("Commitments — LP", "0", "0", "0"),
      acct("Undrawn commitments — LP", "0", "0", "0"),
      acct("Commitments — GP", "0", "0", "0"),
      acct("Undrawn commitments — GP", "0", "0", "0"),
    ];
    expect(isPosted(unset[1])).toBe(false);
    expect(undrawnFigure(unset)).toBeNull();
    expect(remainingCommitment(unset)).toBeNull();
    expect(commitmentIdentityHolds(unset)).toBe(true);
    // A chart with no commitment accounts is the same refusal.
    expect(undrawnFigure([acct("Partner capital — LP", "0", "10000")])).toBeNull();
  });

  it("a call leaves remaining commitment equal to remaining undrawn", () => {
    // Commit 100.00, call 40.00 → remaining 60.00 on both sides.
    const accounts = [
      acct("Partner capital — LP", "0", "4000"),
      acct("Commitments — LP", "4000", "10000"),
      acct("Undrawn commitments — LP", "10000", "4000"),
      acct("Commitments — GP", "0", "0", "0"),
      acct("Undrawn commitments — GP", "0", "0", "0"),
    ];
    expect(remainingUndrawn(accounts[2])).toBe(6000n);
    expect(endingCapital(accounts[1])).toBe(6000n);
    expect(undrawnFigure(accounts)).toBe(6000n);
    expect(remainingCommitment(accounts)).toBe(6000n);
    expect(commitmentIdentityHolds(accounts)).toBe(true);
    expect(bookCapital(accounts)).toBe(4000n);
    // Fully drawn is a real zero, not unset.
    const drawn = [
      acct("Commitments — LP", "10000", "10000"),
      acct("Undrawn commitments — LP", "10000", "10000"),
    ];
    expect(undrawnFigure(drawn)).toBe(0n);
    expect(remainingCommitment(drawn)).toBe(0n);
    // ⛔ SABOTAGE: dropping a partner's undrawn keeps the other side
    // internally consistent and would stay green if we only summed
    // what we were handed.
    const withoutLp = accounts.filter((a) => !a.displayName.endsWith("— LP") || a.displayName.startsWith("Partner"));
    expect(undrawnFigure(withoutLp)).toBeNull();
    expect(undrawnFigure(withoutLp) === 6000n).toBe(false);
  });
});
