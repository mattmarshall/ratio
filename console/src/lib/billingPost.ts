// The cash-application rule CreateBook already seeds.
//
// ⭐ THE SAME RULE `/record` USES. This page does not grow a payment
// processor or a second AR store. A collection is a conserved pair
// (cash up, receivable down) on the project chart; `/billing` cites
// collections vs billed after it posts. Unset until billed and AR
// can support the cut — treating an unbilled job as collected 0.00
// would invent cash that never arrived.
//
// ⛔ NOT A `Method` / `Order` / `lot_method` VARIANT. The id is
// `collect_receivable`. Inventing a fifth kind here would be a
// parallel book. Stripe / ACH stay Connect.

import type { Rule } from "@/wire/types";

/** The one cash-against-AR rule CreateBook(Project) writes. */
export const COLLECT_RECEIVABLE = "collect_receivable";

// ⚠ NOT LISTED VIA `listRules`. `/billing` already makes three upstream
// reads; a fourth is refused by `route_manifest_test`. The id is the
// kind-selected seed.

/**
 * The allowlist `/billing` will post. A tampered form that sends
 * `progress_bill`, `project_cost`, or `equity_purchase` is refused
 * here rather than becoming a silent bill, a cost, or a lot.
 */
export function isBillingJournalRule(ruleId: string): boolean {
  return ruleId === COLLECT_RECEIVABLE;
}

/** Rules this page may offer — in force, and on the allowlist. */
export function billingRulesInForce(rules: readonly Rule[]): Rule[] {
  return rules.filter((r) => isBillingJournalRule(r.ruleId));
}
