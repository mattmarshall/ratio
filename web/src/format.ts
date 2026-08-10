// Turning exact integers into something a fund accountant reads.
//
// ⛔ NOTHING HERE PARSES MONEY INTO A NUMBER. The server sends minor units as
// a string precisely so the browser cannot silently make it a double, and
// `Number("482137244620")` would undo that in one character. Every function
// below works on the digits.

/** `"48213724462"` → `"482,137,244.62"`. Minor units in, major units out. */
export function money(minor: string): string {
  const neg = minor.startsWith("-");
  const digits = (neg ? minor.slice(1) : minor).padStart(3, "0");
  const whole = digits.slice(0, -2);
  const cents = digits.slice(-2);
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return `${neg ? "-" : ""}${grouped}.${cents}`;
}

/** `"48112"` → `"48,112"`. For counts, which have no cents. */
export function count(n: string): string {
  const neg = n.startsWith("-");
  const d = neg ? n.slice(1) : n;
  return (neg ? "-" : "") + d.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
}

/**
 * A credit-normal figure, as a reader expects to see it.
 *
 * ⛔ A REALIZED GAIN READS NEGATIVE IN THE RAW FIGURE. `Ratio.Lots.Posting` has
 * the convention: the gain leg is `relieved − proceeds`, so a profitable
 * disposal credits income and the number carries a minus sign. A screen that
 * prints it unflipped shows every profitable fund as a loss.
 *
 * ⚠ THE FLIP LIVES HERE, IN ONE FUNCTION, on purpose. Doing it at each call
 * site is how a sign convention gets applied twice in one place and nowhere in
 * another — and both mistakes produce a plausible number.
 *
 * Negating the string rather than the value, because nothing in this file is
 * allowed to make an int64 a double.
 */
export function gain(minor: string): string {
  if (minor === "" || minor === "0") return minor;
  return money(minor.startsWith("-") ? minor.slice(1) : `-${minor}`);
}

/** Compare two minor-unit strings by magnitude, without parsing either. */
export function absCompare(a: string, b: string): number {
  const x = a.replace("-", "").replace(/^0+/, "");
  const y = b.replace("-", "").replace(/^0+/, "");
  if (x.length !== y.length) return x.length - y.length;
  return x < y ? -1 : x > y ? 1 : 0;
}

export const STATE_LABEL: Record<string, string> = {
  AWAITING_PRICES: "Awaiting prices",
  BLOCKED: "Blocked",
  IN_REVIEW: "In review",
  STRUCK: "Struck",
  UNSPECIFIED: "Unknown",
};

export const STATE_CLASS: Record<string, string> = {
  AWAITING_PRICES: "waiting",
  BLOCKED: "blocked",
  IN_REVIEW: "review",
  STRUCK: "struck",
  UNSPECIFIED: "waiting",
};

export const SEVERITY_CLASS: Record<string, string> = {
  HIGH: "high",
  MEDIUM: "med",
  LOW: "low",
  UNSPECIFIED: "low",
};
