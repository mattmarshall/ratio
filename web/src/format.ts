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
