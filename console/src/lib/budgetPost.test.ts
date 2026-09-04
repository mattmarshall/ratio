import { describe, expect, it } from "vitest";
import type { Rule } from "@/wire/types";
import {
  BUDGET_INGEST_TEMPLATES,
  budgetRuleId,
  budgetRulesInForce,
  isBudgetJournalRule,
  kindOfRule,
  kindsInForce,
  phaseOfRule,
  phasesInForce,
} from "./budgetPost";

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

describe("budget journal rules", () => {
  it("maps kind × phase onto the seeded rule ids, not a new kind", () => {
    expect(budgetRuleId("approve", "site")).toBe("approve_co_site");
    expect(budgetRuleId("approve", "unpartitioned")).toBe("approve_co");
    expect(budgetRuleId("deduct", "structure")).toBe("deduct_co_structure");
    expect(budgetRuleId("award", "finishes")).toBe("award_commitment_finishes");
    expect(budgetRuleId("award", "unpartitioned")).toBe("award_commitment");
    expect(budgetRuleId("release", "site")).toBe("release_commitment_site");
  });

  it("allows only the CO / award pair CreateBook seeds", () => {
    expect(isBudgetJournalRule("approve_co_site")).toBe(true);
    expect(isBudgetJournalRule("award_commitment")).toBe(true);
    expect(isBudgetJournalRule("release_commitment_finishes")).toBe(true);
    expect(isBudgetJournalRule("project_cost")).toBe(false);
    expect(isBudgetJournalRule("vendor_invoice_site")).toBe(false);
    expect(isBudgetJournalRule("equity_purchase")).toBe(false);
    expect(isBudgetJournalRule("approve_co_roof")).toBe(false);
    expect(isBudgetJournalRule("award_commitment_site_extra")).toBe(false);
  });

  it("reads kind and phase back off a rule id", () => {
    expect(kindOfRule("approve_co_site")).toBe("approve");
    expect(phaseOfRule("approve_co_site")).toBe("site");
    expect(kindOfRule("award_commitment")).toBe("award");
    expect(phaseOfRule("award_commitment")).toBe("unpartitioned");
    expect(kindOfRule("project_cost")).toBeNull();
    expect(phaseOfRule("project_cost")).toBeNull();
  });

  it("offers only the rules in force, so an unseeded phase stays off the picker", () => {
    const listed = [
      rule("approve_co_site"),
      rule("award_commitment_site"),
      rule("project_cost"),
      rule("dividend"),
    ];
    expect(budgetRulesInForce(listed).map((r) => r.ruleId)).toEqual([
      "approve_co_site",
      "award_commitment_site",
    ]);
    expect(kindsInForce(listed)).toEqual(["approve", "award"]);
    expect(phasesInForce(listed, "approve")).toEqual(["site"]);
    expect(phasesInForce(listed, "award")).toEqual(["site"]);
    expect(phasesInForce(listed, "deduct")).toEqual([]);
    expect(phasesInForce(listed, "")).toEqual([]);
  });

  it("names the two ingest templates CreateBook writes, and no others", () => {
    expect(BUDGET_INGEST_TEMPLATES.map((t) => t.templateId)).toEqual([
      "change-orders",
      "purchase-orders",
    ]);
    expect(BUDGET_INGEST_TEMPLATES.every((t) => t.posts)).toBe(true);
    expect(BUDGET_INGEST_TEMPLATES.some((t) => t.templateId === "project-invoices")).toBe(
      false,
    );
  });
});
