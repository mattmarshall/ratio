import { describe, expect, it } from "vitest";
import {
  accountsReceivable,
  approvedChangeOrders,
  awardedCommitments,
  changeOrdersInWindow,
  collectedAgainstBilled,
  figure,
  isApprovedChangeOrder,
  isAwardedCommitment,
  isAwardedCommitmentAccount,
  isChangeOrderAccount,
  isFundingAccount,
  outstandingAgainstBilled,
  phaseApproved,
  phaseAwarded,
  projectRollup,
  remainingToBill,
  remainingToSpendOf,
  revisedContract,
  wipFoots,
} from "./project";
import type { Account } from "@/wire/types";

function acct(
  dim: string,
  displayName: string,
  type: Account["type"],
  debit: string,
  credit: string,
  postingCount = "1",
): Account {
  const d = BigInt(debit);
  const c = BigInt(credit);
  return {
    name: `funds/bridge/views/book/accounts/${dim}`,
    displayName,
    dimension: dim,
    type,
    debit,
    credit,
    balance: (d - c).toString(),
    abnormal: false,
    postingCount,
    currencyTotals: [],
  };
}

const CHART: Account[] = [
  acct("1", "Cash", "ASSET", "500000", "200000"),
  acct("2", "Work in progress", "ASSET", "400000", "100000"),
  acct("10", "Project costs", "EXPENSE", "700000", "400000"),
  acct("20", "Funding", "EQUITY", "0", "800000"),
  acct("21", "Change-order authorization", "EQUITY", "0", "0", "0"),
  acct("25", "Approved change orders", "EQUITY", "0", "0", "0"),
  acct("30", "Project revenue", "REVENUE", "0", "150000"),
  acct("40", "Payables", "LIABILITY", "0", "200000"),
  acct("60", "Commitment authorization", "EQUITY", "0", "0", "0"),
  acct("64", "Awarded commitments", "EQUITY", "0", "0", "0"),
];

describe("projectRollup", () => {
  it("cites incurred as costs plus WIP so recognizing does not double-count", () => {
    // costs.balance 3000.00 + wip.balance 3000.00 = 6000.00 incurred.
    // Using costs.debit (7000.00) would count capitalized amounts twice
    // after recognize_wip credited costs and then debited them again.
    const r = projectRollup(CHART, "10000000");
    expect(r.costs).toBe(300000n);
    expect(r.wip).toBe(300000n);
    expect(r.incurred).toBe(600000n);
    expect(r.payables).toBe(-200000n);
    expect(r.committed).toBe(800000n);
    expect(r.baseline).toBe(10000000n);
    expect(r.approved).toBeNull();
    expect(r.awarded).toBeNull();
    expect(r.revised).toBe(10000000n);
    expect(r.variance).toBe(9200000n);
    expect(r.remainingToSpend).toBeNull();
    expect(r.funding).toBe(-800000n);
    expect(r.revenue).toBe(-150000n);
  });

  it("treats an empty budget as unset, not a baseline of zero", () => {
    const r = projectRollup(CHART, "");
    expect(r.baseline).toBeNull();
    expect(r.approved).toBeNull();
    expect(r.revised).toBeNull();
    expect(r.variance).toBeNull();
    expect(r.awarded).toBeNull();
    expect(r.remainingToSpend).toBeNull();
    expect(r.incurred).toBe(600000n);
  });

  it("a zero budget is a set baseline of nothing", () => {
    const r = projectRollup(CHART, "0");
    expect(r.baseline).toBe(0n);
    expect(r.revised).toBe(0n);
    expect(r.variance).toBe(-800000n);
    expect(r.remainingToSpend).toBeNull();
  });

  it("WIP debit equals currently capitalized plus recognized", () => {
    const r = projectRollup(CHART, "");
    expect(r.wipDebit).toBe(400000n);
    expect(r.wipCredit).toBe(100000n);
    expect(wipFoots(r)).toBe(true);
  });

  it("does not invent accounts the chart does not have", () => {
    const r = projectRollup([], "100");
    expect(r.incurred).toBe(0n);
    expect(r.committed).toBe(0n);
    expect(r.baseline).toBe(100n);
    expect(r.approved).toBeNull();
    expect(r.revised).toBe(100n);
    expect(r.variance).toBe(100n);
    expect(r.awarded).toBeNull();
    expect(r.remainingToSpend).toBeNull();
  });

  it("adds approved change orders to the revised contract without rewriting the baseline", () => {
    const withCo = [
      ...CHART.filter((a) => a.dimension !== "21" && a.dimension !== "25"),
      acct("21", "Change-order authorization", "EQUITY", "500000", "0"),
      acct("25", "Approved change orders", "EQUITY", "0", "500000"),
      acct("22", "Change-order authorization — Site and mobilization", "EQUITY", "400000", "0"),
      acct("26", "Approved change orders — Site and mobilization", "EQUITY", "0", "400000"),
    ];
    const r = projectRollup(withCo, "10000000");
    expect(r.baseline).toBe(10000000n);
    expect(r.approved).toBe(900000n);
    expect(r.revised).toBe(10900000n);
    expect(r.variance).toBe(10100000n);
    expect(r.committed).toBe(800000n);
    expect(r.awarded).toBeNull();
    expect(r.remainingToSpend).toBeNull();
    // ⛔ SABOTAGE: folding the pair into funding would cancel (good) but
    // folding only the credit into funding would inflate owner money.
    expect(r.funding).toBe(-800000n);
    expect(withCo.filter(isFundingAccount).map((a) => a.displayName)).toEqual(["Funding"]);
  });

  it("cannot revise an unknown baseline even when a change order has posted", () => {
    const withCo = [
      ...CHART.filter((a) => a.dimension !== "25"),
      acct("25", "Approved change orders", "EQUITY", "0", "500000"),
    ];
    const r = projectRollup(withCo, "");
    expect(r.baseline).toBeNull();
    expect(r.approved).toBe(500000n);
    expect(r.revised).toBeNull();
    expect(r.variance).toBeNull();
  });
});

describe("change orders", () => {
  it("names the seeded pair, not a cost or the baseline key", () => {
    expect(isApprovedChangeOrder("Approved change orders")).toBe(true);
    expect(isApprovedChangeOrder("Approved change orders — Structure")).toBe(true);
    expect(isChangeOrderAccount("Change-order authorization — Finishes and closeout")).toBe(true);
    expect(isApprovedChangeOrder("Project costs")).toBe(false);
    expect(isChangeOrderAccount("Funding")).toBe(false);
    expect(isChangeOrderAccount("Site and mobilization")).toBe(false);
  });

  it("treats postingCount 0 as unset, and a posted net of nothing as a real zero", () => {
    expect(approvedChangeOrders(CHART)).toBeNull();
    expect(
      approvedChangeOrders([acct("25", "Approved change orders", "EQUITY", "0", "0", "0")]),
    ).toBeNull();
    expect(
      approvedChangeOrders([acct("25", "Approved change orders", "EQUITY", "5000", "5000")]),
    ).toBe(0n);
  });

  it("keys a phase CO to the matching work-package expense", () => {
    const accounts = [
      acct("25", "Approved change orders", "EQUITY", "0", "0", "0"),
      acct("26", "Approved change orders — Site and mobilization", "EQUITY", "0", "400000"),
    ];
    expect(phaseApproved(accounts, "Site and mobilization")).toBe(400000n);
    expect(phaseApproved(accounts, "Structure")).toBeNull();
    expect(phaseApproved(accounts, "Project costs")).toBeNull();
  });

  it("window activity is unset when nothing posted in the fold", () => {
    expect(changeOrdersInWindow(CHART)).toBeNull();
    const windowed = [acct("26", "Approved change orders — Site and mobilization", "EQUITY", "0", "400000")];
    expect(changeOrdersInWindow(windowed)).toBe(400000n);
  });

  it("revised equals baseline when no change order has posted", () => {
    expect(revisedContract(10000000n, null)).toBe(10000000n);
    expect(revisedContract(null, 500000n)).toBeNull();
    expect(revisedContract(10000000n, 500000n)).toBe(10500000n);
  });
});

describe("remaining to bill", () => {
  it("stays unset when either the revised contract or billed cannot support the cut", () => {
    expect(remainingToBill(null, "")).toBeNull();
    expect(remainingToBill(null, "100000")).toBeNull();
    expect(remainingToBill(10000000n, "")).toBeNull();
  });

  it("is revised minus billed when both sides are set, including a real zero", () => {
    expect(remainingToBill(10000000n, "100000")).toBe(9900000n);
    expect(remainingToBill(10500000n, "100000")).toBe(10400000n);
    expect(remainingToBill(100000n, "100000")).toBe(0n);
    expect(remainingToBill(0n, "0")).toBe(0n);
  });
});

describe("awarded commitments", () => {
  it("names the seeded pair, not a cost or a payable", () => {
    expect(isAwardedCommitment("Awarded commitments")).toBe(true);
    expect(isAwardedCommitment("Awarded commitments — Structure")).toBe(true);
    expect(isAwardedCommitmentAccount("Commitment authorization — Finishes and closeout")).toBe(true);
    expect(isAwardedCommitment("Project costs")).toBe(false);
    expect(isAwardedCommitmentAccount("Funding")).toBe(false);
    expect(isAwardedCommitmentAccount("Payables")).toBe(false);
    expect(isAwardedCommitmentAccount("Site and mobilization")).toBe(false);
    expect(isChangeOrderAccount("Awarded commitments")).toBe(false);
  });

  it("treats postingCount 0 as unset, and a posted net of nothing as a real zero", () => {
    expect(awardedCommitments(CHART)).toBeNull();
    expect(
      awardedCommitments([acct("64", "Awarded commitments", "EQUITY", "0", "0", "0")]),
    ).toBeNull();
    expect(
      awardedCommitments([acct("64", "Awarded commitments", "EQUITY", "5000", "5000")]),
    ).toBe(0n);
  });

  it("keys a phase award to the matching work-package expense", () => {
    const accounts = [
      acct("64", "Awarded commitments", "EQUITY", "0", "0", "0"),
      acct("65", "Awarded commitments — Site and mobilization", "EQUITY", "0", "300000"),
    ];
    expect(phaseAwarded(accounts, "Site and mobilization")).toBe(300000n);
    expect(phaseAwarded(accounts, "Structure")).toBeNull();
    expect(phaseAwarded(accounts, "Project costs")).toBeNull();
  });

  it("does not fold the pair into funding", () => {
    const withPo = [
      ...CHART.filter((a) => a.dimension !== "60" && a.dimension !== "64"),
      acct("60", "Commitment authorization", "EQUITY", "350000", "0"),
      acct("64", "Awarded commitments", "EQUITY", "0", "350000"),
      acct("61", "Commitment authorization — Site and mobilization", "EQUITY", "300000", "0"),
      acct("65", "Awarded commitments — Site and mobilization", "EQUITY", "0", "300000"),
    ];
    const r = projectRollup(withPo, "10000000");
    expect(r.awarded).toBe(650000n);
    expect(r.incurred).toBe(600000n);
    expect(r.remainingToSpend).toBe(8750000n);
    expect(r.funding).toBe(-800000n);
    expect(withPo.filter(isFundingAccount).map((a) => a.displayName)).toEqual(["Funding"]);
  });
});

describe("remaining to spend", () => {
  it("stays unset when either the revised contract or awarded cannot support the cut", () => {
    expect(remainingToSpendOf(null, 0n, null)).toBeNull();
    expect(remainingToSpendOf(null, 600000n, 350000n)).toBeNull();
    expect(remainingToSpendOf(10000000n, 600000n, null)).toBeNull();
    expect(remainingToSpendOf(10000000n, 0n, null)).toBeNull();
  });

  it("is revised minus incurred minus awarded when both sides are set, including a real zero", () => {
    expect(remainingToSpendOf(10000000n, 600000n, 350000n)).toBe(9050000n);
    expect(remainingToSpendOf(10500000n, 600000n, 350000n)).toBe(9550000n);
    expect(remainingToSpendOf(100000n, 100000n, 0n)).toBe(0n);
    expect(remainingToSpendOf(0n, 0n, 0n)).toBe(0n);
  });

  it("cannot treat an unknown award as zero headroom even when actual is known", () => {
    const r = projectRollup(CHART, "10000000");
    expect(r.incurred).toBe(600000n);
    expect(r.awarded).toBeNull();
    expect(r.remainingToSpend).toBeNull();
    expect(r.variance).toBe(9200000n);
  });
});

describe("collections vs billed", () => {
  it("stays unset when billed has not posted, even if AR looks empty", () => {
    expect(collectedAgainstBilled("", null, "")).toBeNull();
    expect(collectedAgainstBilled("", 0n, "")).toBeNull();
    expect(outstandingAgainstBilled("", 40000n, "10000")).toBeNull();
  });

  it("stays unset when billed is set but AR has never posted", () => {
    expect(collectedAgainstBilled("100000", null, "")).toBeNull();
    expect(collectedAgainstBilled("100000", null, "10000")).toBeNull();
    expect(outstandingAgainstBilled("100000", null, "")).toBeNull();
  });

  it("treats unheld retainage as zero and a billed-but-uncollected job as a real zero", () => {
    expect(collectedAgainstBilled("100000", 100000n, "")).toBe(0n);
    expect(outstandingAgainstBilled("100000", 100000n, "")).toBe(100000n);
  });

  it("is cash against AR: billed minus receivable minus retainage held", () => {
    expect(collectedAgainstBilled("100000", 40000n, "10000")).toBe(50000n);
    expect(outstandingAgainstBilled("100000", 40000n, "10000")).toBe(50000n);
    const billed = 100000n;
    const collected = collectedAgainstBilled("100000", 40000n, "10000")!;
    const outstanding = outstandingAgainstBilled("100000", 40000n, "10000")!;
    expect(collected + outstanding).toBe(billed);
  });

  it("reads Accounts receivable only when that account has posted", () => {
    expect(accountsReceivable(CHART)).toBeNull();
    expect(
      accountsReceivable([acct("3", "Accounts receivable", "ASSET", "0", "0", "0")]),
    ).toBeNull();
    expect(
      accountsReceivable([acct("3", "Accounts receivable", "ASSET", "100000", "50000")]),
    ).toBe(50000n);
  });
});

describe("project figures", () => {
  it("renders unset as a dash and zero as zero", () => {
    expect(figure("")).toBe("—");
    expect(figure("0")).toBe("0.00");
    expect(figure("10000")).toBe("100.00");
    expect(figure("-20000")).toBe("-200.00");
  });
});
