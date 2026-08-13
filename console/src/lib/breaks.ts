import type { Break } from "@/wire/types";

/**
 * Whether this break is a difference between two figures.
 *
 * ⛔ NOT EVERY BREAK IS ONE. A lot break — a disposal with no acquisition left
 * to relieve — has no "our figure" and no "their figure" to compare, so all
 * three amounts are zero. Printing `0.00 / 0.00 / 0.00` against a HIGH-severity
 * row reads as "nothing is wrong", which is the exact opposite of what it means.
 * Screens ask this before rendering an amount.
 */
export function isMoneyBreak(b: Break): boolean {
  return b.ratioAmount !== "0" || b.reportedAmount !== "0" || b.difference !== "0";
}
