// Kind × phase → the journal rule CreateBook already seeds.
//
// ⭐ THE SAME RULES `/record` AND `change-orders` / `purchase-orders` USE.
// This page does not grow a second budget store. An award or change order
// is a conserved equity pair on the project chart; `/budget` cites it
// after it posts. Unset until then — `postingCount === "0"` is the
// distinction, and treating an unposted award as 0 would print
// budget − actual as headroom.
//
// ⛔ NOT A `Method` / `Order` / `lot_method` VARIANT. These ids are
// `approve_co_*` / `deduct_co_*` / `award_commitment_*` /
// `release_commitment_*`. Inventing a fifth kind here would be a
// parallel book.

import type { Rule, Template } from "@/wire/types";

export type BudgetPostKind = "approve" | "deduct" | "award" | "release";

/**
 * Work-package grain. `unpartitioned` is the pair without a suffix
 * (`approve_co`, `award_commitment`). ⛔ NOT `""` — the picker uses empty
 * for "not yet chosen", and collapsing the two would mark the step
 * answered the moment an unpartitioned rule existed.
 */
export type BudgetPhase = "unpartitioned" | "site" | "structure" | "finishes";

export const BUDGET_POST_KINDS: readonly {
  readonly id: BudgetPostKind;
  readonly label: string;
  readonly prefix: string;
}[] = [
  { id: "approve", label: "Approve a change order", prefix: "approve_co" },
  { id: "deduct", label: "Deduct a change order", prefix: "deduct_co" },
  { id: "award", label: "Award a purchase order", prefix: "award_commitment" },
  { id: "release", label: "Release a purchase order", prefix: "release_commitment" },
];

export const BUDGET_PHASES: readonly {
  readonly id: BudgetPhase;
  readonly label: string;
}[] = [
  { id: "unpartitioned", label: "Unpartitioned" },
  { id: "site", label: "Site and mobilization" },
  { id: "structure", label: "Structure" },
  { id: "finishes", label: "Finishes and closeout" },
];

const RULE_ID =
  /^(approve_co|deduct_co|award_commitment|release_commitment)(_site|_structure|_finishes)?$/;

/**
 * The allowlist `/budget` will post. A tampered form that sends
 * `project_cost` or `equity_purchase` is refused here rather than
 * becoming a silent cost or a lot.
 */
export function isBudgetJournalRule(ruleId: string): boolean {
  return RULE_ID.test(ruleId);
}

export function budgetRuleId(kind: BudgetPostKind, phase: BudgetPhase): string {
  const row = BUDGET_POST_KINDS.find((k) => k.id === kind);
  if (!row) return "";
  return phase === "unpartitioned" ? row.prefix : `${row.prefix}_${phase}`;
}

export function kindOfRule(ruleId: string): BudgetPostKind | null {
  if (ruleId === "approve_co" || ruleId.startsWith("approve_co_")) return "approve";
  if (ruleId === "deduct_co" || ruleId.startsWith("deduct_co_")) return "deduct";
  if (ruleId === "award_commitment" || ruleId.startsWith("award_commitment_")) {
    return "award";
  }
  if (ruleId === "release_commitment" || ruleId.startsWith("release_commitment_")) {
    return "release";
  }
  return null;
}

export function phaseOfRule(ruleId: string): BudgetPhase | null {
  if (!isBudgetJournalRule(ruleId)) return null;
  if (ruleId.endsWith("_site")) return "site";
  if (ruleId.endsWith("_structure")) return "structure";
  if (ruleId.endsWith("_finishes")) return "finishes";
  return "unpartitioned";
}

/** Rules this page may offer — in force, and on the allowlist. */
export function budgetRulesInForce(rules: readonly Rule[]): Rule[] {
  return rules.filter((r) => isBudgetJournalRule(r.ruleId));
}

export function kindsInForce(rules: readonly Rule[]): BudgetPostKind[] {
  const seen = new Set<BudgetPostKind>();
  for (const r of budgetRulesInForce(rules)) {
    const k = kindOfRule(r.ruleId);
    if (k) seen.add(k);
  }
  return BUDGET_POST_KINDS.map((k) => k.id).filter((id) => seen.has(id));
}

export function phasesInForce(
  rules: readonly Rule[],
  kind: BudgetPostKind | "",
): BudgetPhase[] {
  if (!kind) return [];
  const seen = new Set<BudgetPhase>();
  for (const r of budgetRulesInForce(rules)) {
    if (kindOfRule(r.ruleId) !== kind) continue;
    const p = phaseOfRule(r.ruleId);
    if (p) seen.add(p);
  }
  return BUDGET_PHASES.map((p) => p.id).filter((id) => seen.has(id));
}

/**
 * The two ingest mappings CreateBook writes on a Project book.
 *
 * ⚠ NOT A SECOND LIST. `listTemplates` would be a fourth upstream call
 * on `/budget` (over the three-call door). These ids are the kind-selected
 * catalog `INGEST_TEMPLATE_KIND` already names; the live mapping is the
 * book's configuration, and a book that never received them refuses at
 * admit rather than inventing a parallel store.
 */
export const BUDGET_INGEST_TEMPLATES: readonly Template[] = [
  {
    name: "templates/change-orders",
    templateId: "change-orders",
    factKind: "change",
    form: "one change per row — ChangeRef, Date, Amount, Ccy, Memo, Kind. Kind is approve_co_* / deduct_co_* (phase keys). Posts through the rules in force.",
    posts: true,
  },
  {
    name: "templates/purchase-orders",
    templateId: "purchase-orders",
    factKind: "purchase",
    form: "one purchase per row — PurchaseRef, Date, Amount, Ccy, Memo, Kind. Kind is award_commitment_* / release_commitment_* (phase keys). Posts through the rules in force.",
    posts: true,
  },
];
