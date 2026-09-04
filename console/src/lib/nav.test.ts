import { describe, expect, it } from "vitest";
import {
  beginningOf,
  bookIssued,
  bookRedeemed,
  bookUnits,
  expenseShown,
  incomeShown,
  navOf,
  navRollForward,
  navShown,
  outflowShown,
  perShareOf,
  perShareShown,
  unitsOf,
  unitsShown,
} from "./nav";
import type { Account } from "@/wire/types";

const acct = (
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
  units: "",
  unitsIssued: "",
  unitsRedeemed: "",
});

describe("a period NAV roll-forward", () => {
  it("leaves beginning and ending unset on a chart that never moved", () => {
    const accounts: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "0", "0", "0", "0"),
      acct("1", "Investments at fair value", "ASSET", "0", "0", "0", "0"),
      acct("50", "Partner capital — LP", "EQUITY", "0", "0", "0", "0"),
      acct("21", "Unrealized gain", "EQUITY", "0", "0", "0", "0"),
      acct("30", "Dividend income", "REVENUE", "0", "0", "0", "0"),
      acct("10", "Management fee expense", "EXPENSE", "0", "0", "0", "0"),
      acct("54", "Undrawn commitments — LP", "EQUITY", "0", "0", "0", "0"),
    ];
    const r = navRollForward(accounts);
    expect(r.beginning).toBeNull();
    expect(r.ending).toBeNull();
    expect(r.delta).toBeNull();
    expect(r.contributions).toBeNull();
    expect(r.distributions).toBeNull();
    expect(r.income).toBeNull();
    expect(r.expense).toBeNull();
    expect(r.unrealized).toBeNull();
    expect(navShown(r.beginning)).toBe("—");
    expect(navShown(0n)).toBe("0.00");
    expect(bookUnits(accounts)).toBeNull();
    expect(bookIssued(accounts)).toBeNull();
    expect(bookRedeemed(accounts)).toBeNull();
    expect(r.issued).toBeNull();
    expect(r.redeemed).toBeNull();
    expect(r.perShare).toBeNull();
    expect(unitsShown(null)).toBe("—");
    expect(unitsShown(0n)).toBe("0");
    expect(perShareShown(null)).toBe("—");
  });

  it("treats a commitment-only prefix that nets to zero NAV as a real beginning", () => {
    // February: Dr undrawn 50 / Cr commitments 50. March is empty.
    const accounts: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "0", "0", "0"),
      acct("52", "Commitments — LP", "EQUITY", "0", "0", "-5000"),
      acct("54", "Undrawn commitments — LP", "EQUITY", "0", "0", "5000"),
      acct("50", "Partner capital — LP", "EQUITY", "0", "0", "0"),
      acct("21", "Unrealized gain", "EQUITY", "0", "0", "0", "0"),
    ];
    const r = navRollForward(accounts);
    expect(r.beginning).toBe(0n);
    expect(r.ending).toBe(0n);
    expect(r.delta).toBe(0n);
    expect(navShown(r.beginning)).toBe("0.00");
    // ⛔ SABOTAGE: treating undrawn as an asset inflates NAV by the unfunded line.
    expect(navOf(accounts, beginningOf)).toBe(0n);
    expect(
      navOf(accounts, beginningOf) + beginningOf(accounts[2]!),
    ).toBe(5000n);
    expect(navOf(accounts, beginningOf) + beginningOf(accounts[2]!) === 0n).toBe(
      false,
    );
  });

  it("cites ΔNAV as contributions minus distributions plus income minus expense plus unrealized", () => {
    // Beginning: cash 100.00 / partner 100.00 (NAV = 100). Commitment 50
    // is equity and cancels. March: contribute 40, distribute 10, dividend
    // 20, fee 5, mark 20. Ending cash 150, investments 20, payable −5.
    // ΔNAV = 65 = 40 − 10 + 20 − 5 + 20.
    const accounts: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "6000", "1000", "15000"),
      acct("1", "Investments at fair value", "ASSET", "2000", "0", "2000"),
      acct("40", "Management fee payable", "LIABILITY", "0", "500", "-500"),
      acct("50", "Partner capital — LP", "EQUITY", "1000", "4000", "-13000"),
      acct("20", "Capital contributions", "EQUITY", "0", "0", "0", "0"),
      acct("22", "Distributions", "EQUITY", "0", "0", "0", "0"),
      acct("30", "Dividend income", "REVENUE", "0", "2000", "-2000"),
      acct("10", "Management fee expense", "EXPENSE", "500", "0", "500"),
      acct("21", "Unrealized gain", "EQUITY", "0", "2000", "-2000"),
      acct("52", "Commitments — LP", "EQUITY", "0", "0", "-5000"),
      acct("54", "Undrawn commitments — LP", "EQUITY", "0", "0", "5000"),
      acct("23", "Allocations", "EQUITY", "0", "0", "0", "0"),
      acct("24", "Capital transfers", "EQUITY", "0", "0", "0", "0"),
    ];
    const r = navRollForward(accounts);

    expect(beginningOf(accounts[0]!)).toBe(10_000n);
    expect(navOf(accounts, beginningOf)).toBe(10_000n);
    expect(r.beginning).toBe(10_000n);
    expect(r.ending).toBe(16_500n);
    expect(r.delta).toBe(6_500n);
    expect(r.contributions).toBe(4_000n);
    expect(r.distributions).toBe(1_000n);
    expect(r.income).toBe(-2_000n);
    expect(r.expense).toBe(500n);
    expect(r.unrealized).toBe(-2_000n);
    expect(r.allocations).toBeNull();
    expect(r.transfers).toBeNull();
    expect(r.residual).toBe(0n);

    // ⛔ ADDING UNDRAWN TO NAV IS THE DEFECT. The identity is beginning
    // + contributions − distributions + income − expenses + unrealized.
    expect(r.ending).not.toBe(r.ending! + 5_000n);
    expect(incomeShown(r.income)).toBe("20.00");
    expect(expenseShown(r.expense)).toBe("5.00");
    expect(navShown(r.delta)).toBe("65.00");
    expect(outflowShown(r.contributions)).toBe("40.00");
    expect(outflowShown(r.distributions)).toBe("10.00");
    expect(incomeShown(r.unrealized)).toBe("20.00");
  });

  it("leaves unrealized unset when the account did not move this window", () => {
    const accounts: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "4000", "0", "4000"),
      acct("50", "Partner capital — LP", "EQUITY", "0", "4000", "-4000"),
      acct("21", "Unrealized gain", "EQUITY", "0", "0", "0", "0"),
    ];
    const r = navRollForward(accounts);
    expect(r.ending).toBe(4_000n);
    expect(r.beginning).toBeNull();
    expect(r.delta).toBeNull();
    expect(r.contributions).toBe(4_000n);
    expect(r.unrealized).toBeNull();
    expect(incomeShown(r.unrealized)).toBe("—");
  });

  it("leaves contributions unset when the chart has no partner or contribution account", () => {
    const accounts: Account[] = [
      acct("2", "Cash", "ASSET", "1000", "0", "1000"),
      acct("30", "Dividend income", "REVENUE", "0", "1000", "-1000"),
    ];
    const r = navRollForward(accounts);
    expect(r.ending).toBe(1_000n);
    expect(r.contributions).toBeNull();
    expect(r.distributions).toBeNull();
    expect(r.income).toBe(-1_000n);
  });

  it("dropping a partner's credit fails the contribution plug, not just conservation", () => {
    const accounts: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "10000", "0", "10000"),
      acct("50", "Partner capital — LP", "EQUITY", "0", "6000", "-6000"),
      acct("51", "Partner capital — GP", "EQUITY", "0", "4000", "-4000"),
    ];
    const r = navRollForward(accounts);
    expect(r.contributions).toBe(10_000n);
    expect(r.ending).toBe(10_000n);
    expect(r.residual).toBeNull();
    const withoutGp = accounts.filter((a) => a.displayName !== "Partner capital — GP");
    const dropped = navRollForward(withoutGp);
    expect(dropped.contributions).toBe(6_000n);
    expect(dropped.contributions === 10_000n).toBe(false);
  });

  it("leaves units in issue unset until a unit event posts, and treats a full redeem as zero", () => {
    const contributeOnly: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "10000", "0", "10000"),
      acct("50", "Partner capital — LP", "EQUITY", "0", "10000", "-10000"),
    ];
    expect(unitsOf(contributeOnly[1]!)).toBeNull();
    expect(bookUnits(contributeOnly)).toBeNull();
    expect(bookUnits(contributeOnly) === 0n).toBe(false);

    const subscribed = contributeOnly.map((a) =>
      a.displayName.startsWith("Partner capital") ? { ...a, units: "10" } : a,
    );
    expect(unitsOf(subscribed[1]!)).toBe(10n);
    expect(bookUnits(subscribed)).toBe(10n);

    const redeemed = subscribed.map((a) =>
      a.displayName.startsWith("Partner capital") ? { ...a, units: "0" } : a,
    );
    expect(unitsOf(redeemed[1]!)).toBe(0n);
    expect(bookUnits(redeemed)).toBe(0n);
    expect(unitsShown(0n)).toBe("0");
  });

  it("cites period issued and redeemed when the window posted them, and leaves them unset otherwise", () => {
    const contributeOnly: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "10000", "0", "10000"),
      acct("50", "Partner capital — LP", "EQUITY", "0", "10000", "-10000"),
    ];
    const r0 = navRollForward(contributeOnly);
    expect(r0.issued).toBeNull();
    expect(r0.redeemed).toBeNull();
    expect(r0.issued === 0n).toBe(false);
    expect(r0.redeemed === 0n).toBe(false);

    const both: Account[] = [
      {
        ...acct("2", "Cash and equivalents", "ASSET", "6000", "4000", "12000"),
        units: "",
      },
      {
        ...acct("50", "Partner capital — LP", "EQUITY", "4000", "10000", "-16000"),
        units: "6",
        unitsIssued: "10",
        unitsRedeemed: "4",
      },
    ];
    const r = navRollForward(both);
    expect(r.issued).toBe(10n);
    expect(r.redeemed).toBe(4n);
    expect(bookIssued(both)).toBe(10n);
    expect(bookRedeemed(both)).toBe(4n);
    // ⛔ THE NET IS NOT THE PLUG. Issued 10 / redeemed 4 is not "issued 6".
    expect(r.issued === 6n).toBe(false);
    expect(bookUnits(both)).toBe(6n);
  });

  it("cites Euclidean per-share when units exist and leaves it unset otherwise", () => {
    // `Ratio.Closure.perShare 1000 3 = (333, 1)` and `perShare (-7) 3 = (-3, 2)`.
    expect(perShareOf(1000n, 3n)).toEqual({ perUnit: 333n, residual: 1n });
    expect(perShareOf(-7n, 3n)).toEqual({ perUnit: -3n, residual: 2n });
    expect(3n * -3n + 2n).toBe(-7n);
    expect(perShareOf(1000n, 0n)).toBeNull();
    expect(perShareOf(1000n, null)).toBeNull();
    expect(perShareOf(null, 10n)).toBeNull();

    const subscribed: Account[] = [
      acct("2", "Cash and equivalents", "ASSET", "10000", "0", "10000"),
      { ...acct("50", "Partner capital — LP", "EQUITY", "0", "10000", "-10000"), units: "10" },
    ];
    const r = navRollForward(subscribed);
    expect(r.ending).toBe(10_000n);
    expect(r.perShare).toEqual({ perUnit: 1000n, residual: 0n });
    expect(perShareShown(r.perShare)).toBe("10.00");

    const fullRedeem = subscribed.map((a) =>
      a.displayName.startsWith("Partner capital") ? { ...a, units: "0" } : a,
    );
    const done = navRollForward(fullRedeem);
    expect(bookUnits(fullRedeem)).toBe(0n);
    expect(done.perShare).toBeNull();
    expect(perShareShown(done.perShare)).toBe("—");
  });
});
