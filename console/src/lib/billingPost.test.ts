import { describe, expect, it } from "vitest";
import type { Rule } from "@/wire/types";
import {
  billingRulesInForce,
  COLLECT_RECEIVABLE,
  isBillingJournalRule,
} from "./billingPost";

function rule(ruleId: string): Rule {
  return {
    name: `funds/bridge/rules/${ruleId}`,
    ruleId,
    kind: "TRADE",
    description: ruleId,
    form: ruleId,
    accounts: [],
    measured: false,
  };
}

describe("billing journal rules", () => {
  it("allows only the cash-application rule CreateBook seeds", () => {
    expect(isBillingJournalRule(COLLECT_RECEIVABLE)).toBe(true);
    expect(isBillingJournalRule("progress_bill")).toBe(false);
    expect(isBillingJournalRule("project_cost")).toBe(false);
    expect(isBillingJournalRule("equity_purchase")).toBe(false);
    expect(isBillingJournalRule("approve_co_site")).toBe(false);
    expect(isBillingJournalRule("collect_receivable_extra")).toBe(false);
  });

  it("offers only the rule in force, so an unseeded book stays off the form", () => {
    const listed = [
      rule("collect_receivable"),
      rule("progress_bill"),
      rule("project_cost"),
      rule("dividend"),
    ];
    expect(billingRulesInForce(listed).map((r) => r.ruleId)).toEqual([
      "collect_receivable",
    ]);
    expect(billingRulesInForce([rule("progress_bill")])).toEqual([]);
  });
});
