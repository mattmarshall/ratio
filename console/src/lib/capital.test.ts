import { describe, expect, it } from "vitest";
import type { Account } from "@/wire/types";
import {
  activityOf,
  allocatedPlug,
  applyCut,
  bookCapital,
  cutForKind,
  capitalShown,
  commitmentIdentityHolds,
  endingCapital,
  identityHolds,
  isCapitalAccount,
  isCommitmentAccount,
  isPosted,
  isUndrawnAccount,
  partnerCapitalAccounts,
  partnerGrain,
  partnerIdentityHolds,
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
    expect(isPosted(unset[1]!)).toBe(false);
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
    expect(remainingUndrawn(accounts[2]!)).toBe(6000n);
    expect(endingCapital(accounts[1]!)).toBe(6000n);
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

const periodAcct = (
  dimension: string,
  displayName: string,
  type: Account["type"],
  debit: string,
  credit: string,
  balance: string,
  postingCount = "1",
): Account => ({
  name: `funds/partners/views/book/accounts/${dimension}`,
  displayName,
  dimension,
  type,
  debit,
  credit,
  balance,
  abnormal: false,
  postingCount,
  currencyTotals: [],
});

describe("a per-partner capital account statement", () => {
  it("names the partner grain and leaves allocated plugs unset, not a silent zero", () => {
    expect(partnerGrain("Partner capital — LP")).toBe("LP");
    expect(partnerGrain("Partner capital — GP")).toBe("GP");
    expect(allocatedPlug(-3000n, null, "LP")).toBeNull();
    expect(allocatedPlug(0n, [], "LP")).toBeNull();
    expect(allocatedPlug(null, [{ partner: "LP", weight: 80n }], "LP")).toBeNull();
    expect(applyCut(-3000n, null)).toBeNull();
    expect(applyCut(-3000n, [])).toBeNull();
    expect(capitalShown(null)).toBe("—");
    expect(capitalShown(0n)).toBe("0.00");
  });

  it("cites a supported partner cut from the Loan-shaped period fold", () => {
    // February left LP 100.00. March: contribute 40, distribute 10.
    // GP contributed 20 this window and had nothing before.
    // Book income 30.00 and unrealized 20.00 moved — they are not a partner cut.
    const accounts = [
      periodAcct("2", "Cash and equivalents", "ASSET", "6000", "1000", "15000"),
      periodAcct("50", "Partner capital — LP", "EQUITY", "1000", "4000", "-13000"),
      periodAcct("51", "Partner capital — GP", "EQUITY", "0", "2000", "-2000"),
      periodAcct("30", "Dividend income", "REVENUE", "0", "3000", "-3000"),
      periodAcct("21", "Unrealized gain", "EQUITY", "0", "2000", "-2000"),
      periodAcct("10", "Management fee expense", "EXPENSE", "500", "0", "500"),
    ];
    const [lp, gp] = partnerCapitalAccounts(accounts, "period");
    expect(lp!.grain).toBe("LP");
    expect(lp!.beginning).toBe(10_000n);
    expect(lp!.contributions).toBe(4_000n);
    expect(lp!.distributions).toBe(1_000n);
    expect(lp!.ending).toBe(13_000n);
    expect(gp!.beginning).toBe(0n);
    expect(gp!.contributions).toBe(2_000n);
    expect(gp!.distributions).toBe(0n);
    expect(gp!.ending).toBe(2_000n);
    expect(partnerIdentityHolds(lp!)).toBe(true);
    expect(partnerIdentityHolds(gp!)).toBe(true);

    // ⛔ NEVER EQUAL-SPLIT. 30.00 income / 2 is 15.00; 20.00 unrealized / 2
    // is 10.00. Neither partner owns that figure. 0n is the other fake.
    expect(lp!.allocatedIncome).toBeNull();
    expect(gp!.allocatedIncome).toBeNull();
    expect(lp!.allocatedExpense).toBeNull();
    expect(gp!.allocatedExpense).toBeNull();
    expect(lp!.unrealized).toBeNull();
    expect(gp!.unrealized).toBeNull();
    expect(lp!.allocatedIncome === -1_500n).toBe(false);
    expect(gp!.allocatedIncome === -1_500n).toBe(false);
    expect(lp!.unrealized === -1_000n).toBe(false);
    expect(lp!.allocatedIncome === 0n).toBe(false);
    expect(lp!.unrealized === 0n).toBe(false);
    expect(allocatedPlug(-3000n, null, "LP") === -1500n).toBe(false);
    expect(applyCut(-3000n, null)?.get("LP") === -1500n).toBe(false);
  });

  it("fills allocated plugs only when a named cut divides the figure", () => {
    // 80/20 of 30.00 income is 24.00 / 6.00, not 15.00 / 15.00.
    const cut = [
      { partner: "LP", weight: 80n },
      { partner: "GP", weight: 20n },
    ];
    const accounts = [
      periodAcct("2", "Cash and equivalents", "ASSET", "6000", "1000", "15000"),
      periodAcct("50", "Partner capital — LP", "EQUITY", "1000", "4000", "-13000"),
      periodAcct("51", "Partner capital — GP", "EQUITY", "0", "2000", "-2000"),
      periodAcct("30", "Dividend income", "REVENUE", "0", "3000", "-3000"),
      periodAcct("21", "Unrealized gain", "EQUITY", "0", "2000", "-2000"),
      periodAcct("10", "Management fee expense", "EXPENSE", "500", "0", "500"),
    ];
    const [lp, gp] = partnerCapitalAccounts(accounts, "period", cut);
    expect(lp!.allocatedIncome).toBe(2_400n);
    expect(gp!.allocatedIncome).toBe(600n);
    expect(lp!.unrealized).toBe(1_600n);
    expect(gp!.unrealized).toBe(400n);
    expect(lp!.allocatedExpense).toBe(400n);
    expect(gp!.allocatedExpense).toBe(100n);
    expect(lp!.allocatedIncome === 1_500n).toBe(false);
    expect(applyCut(3000n, cut)?.get("LP")).toBe(2_400n);
    expect(applyCut(3000n, cut)?.get("LP") === 1_500n).toBe(false);

    // A figure that will not divide leaves every partner unset.
    const odd = applyCut(101n, cut);
    expect(odd).toBeNull();
    expect(allocatedPlug(101n, cut, "LP")).toBeNull();

    // Standing special: 100% of expense to GP, default 80/20 elsewhere.
    const specials = [{ partner: "GP", kind: "expense" as const, weight: 1n }];
    expect(cutForKind("expense", cut, specials)).toEqual([
      { partner: "GP", weight: 1n },
    ]);
    const [lp2, gp2] = partnerCapitalAccounts(accounts, "period", cut, specials);
    expect(gp2!.allocatedExpense).toBe(500n);
    expect(lp2!.allocatedExpense).toBeNull();
    expect(lp2!.allocatedIncome).toBe(2_400n);
  });

  it("does not silently equal-split contributions across partners", () => {
    const accounts = [
      periodAcct("2", "Cash and equivalents", "ASSET", "10000", "0", "10000"),
      periodAcct("50", "Partner capital — LP", "EQUITY", "0", "6000", "-6000"),
      periodAcct("51", "Partner capital — GP", "EQUITY", "0", "4000", "-4000"),
    ];
    const rows = partnerCapitalAccounts(accounts, "period");
    expect(rows.map((r) => r.contributions)).toEqual([6_000n, 4_000n]);
    expect(rows.every((r) => r.contributions === 5_000n)).toBe(false);
    // ⛔ SABOTAGE: dropping GP keeps LP and would stay green if we only
    // summed what we were handed and divided by the remaining count.
    const withoutGp = partnerCapitalAccounts(
      accounts.filter((a) => a.displayName !== "Partner capital — GP"),
      "period",
    );
    expect(withoutGp).toHaveLength(1);
    expect(withoutGp[0]!.contributions).toBe(6_000n);
    expect(withoutGp[0]!.contributions === 10_000n).toBe(false);
    expect(withoutGp[0]!.contributions === 5_000n).toBe(false);
  });

  it("leaves an unsupported partner cut unset, not a fabricated zero or a book-level stand-in", () => {
    const empty: Account[] = [
      periodAcct("2", "Cash and equivalents", "ASSET", "0", "0", "0", "0"),
      periodAcct("50", "Partner capital — LP", "EQUITY", "0", "0", "0", "0"),
      periodAcct("51", "Partner capital — GP", "EQUITY", "0", "0", "0", "0"),
      periodAcct("30", "Dividend income", "REVENUE", "0", "0", "0", "0"),
      periodAcct("21", "Unrealized gain", "EQUITY", "0", "0", "0", "0"),
    ];
    const unset = partnerCapitalAccounts(empty, "period");
    expect(unset).toHaveLength(2);
    for (const row of unset) {
      expect(row.beginning).toBeNull();
      expect(row.contributions).toBeNull();
      expect(row.distributions).toBeNull();
      expect(row.allocatedIncome).toBeNull();
      expect(row.allocatedExpense).toBeNull();
      expect(row.unrealized).toBeNull();
      expect(row.ending).toBeNull();
      expect(partnerIdentityHolds(row)).toBeNull();
      expect(capitalShown(row.beginning)).toBe("—");
      expect(row.beginning === 0n).toBe(false);
      expect(row.ending === 0n).toBe(false);
      expect(row.allocatedIncome === 0n).toBe(false);
    }

    // Book-level activity without a partner account is not a fake partner.
    const unallocated = [
      periodAcct("20", "Capital contributions", "EQUITY", "0", "10000", "-10000"),
      periodAcct("2", "Cash and equivalents", "ASSET", "10000", "0", "10000"),
    ];
    expect(partnerCapitalAccounts(unallocated, "period")).toEqual([]);
    expect(partnerCapitalAccounts(unallocated, "inception")).toEqual([]);
  });

  it("refuses a fake zero beginning when the fold is activity-shaped", () => {
    // capital-YYYY-MM is Activity: debit/credit = period, balance = same.
    // beginningOf is always 0. Treating that 0 as beginning capital is
    // the defect — February's 100.00 would vanish.
    const activityShaped = [
      periodAcct("50", "Partner capital — LP", "EQUITY", "1000", "4000", "-3000"),
      periodAcct("51", "Partner capital — GP", "EQUITY", "0", "2000", "-2000"),
    ];
    const rows = partnerCapitalAccounts(activityShaped, "period");
    expect(rows[0]!.beginning).toBeNull();
    expect(rows[0]!.beginning === 0n).toBe(false);
    expect(rows[0]!.contributions).toBe(4_000n);
    expect(rows[0]!.ending).toBe(3_000n);
    expect(partnerIdentityHolds(rows[0]!)).toBeNull();
  });

  it("since inception leaves beginning unset and an unposted partner unset", () => {
    const accounts = [
      acct("Partner capital — LP", "2500", "10000"),
      acct("Partner capital — GP", "0", "0", "0"),
      acct("Capital contributions", "0", "0", "0"),
    ];
    const [lp, gp] = partnerCapitalAccounts(accounts, "inception");
    expect(lp!.beginning).toBeNull();
    expect(lp!.contributions).toBe(10_000n);
    expect(lp!.distributions).toBe(2_500n);
    expect(lp!.ending).toBe(7_500n);
    expect(lp!.allocatedIncome).toBeNull();
    expect(lp!.unrealized).toBeNull();
    expect(gp!.beginning).toBeNull();
    expect(gp!.contributions).toBeNull();
    expect(gp!.ending).toBeNull();
    expect(gp!.ending === 0n).toBe(false);
    expect(partnerIdentityHolds(lp!)).toBeNull();
    expect(partnerIdentityHolds(gp!)).toBeNull();
  });
});
