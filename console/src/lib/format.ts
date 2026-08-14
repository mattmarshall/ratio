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

/**
 * A proto3 Duration as a figure a person reads — `"412 µs"`, `"1.4 ms"`.
 *
 * ⚠ Parsed as a Number, unlike everything else here, and that is safe for the
 * reason the rest is not: this is a DURATION, not money. It is already
 * approximate, nobody is paid on it, and a nanosecond count stays exact in a
 * double until well past a century.
 */
export function micros(duration: string): string {
  // proto3 canonical JSON: seconds with an `s` suffix, e.g. "0.000005291s".
  const n = Number(duration.replace(/s$/, "")) * 1_000_000_000;
  if (!Number.isFinite(n) || n < 0) return "—";
  if (n < 1_000) return `${n} ns`;
  if (n < 1_000_000) return `${Math.round(n / 1_000)} µs`;
  if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(1)} ms`;
  return `${(n / 1_000_000_000).toFixed(1)} s`;
}

/**
 * A count of reads, or the absence of one.
 *
 * ⛔ `—` FOR EMPTY, `0` FOR ZERO, AND THEY ARE NOT THE SAME CLAIM. On a plan a
 * step that nothing costed renders empty; a step proved to cost nothing renders
 * `0`, and that zero is `Ratio.Closure.factored_nav_never_reads_the_lots`. A
 * formatter that printed both as `0` would delete the theorem, and one that
 * printed both as `—` would delete it just as thoroughly.
 */
export function reads(n: string): string {
  return n === "" ? "—" : count(n);
}

/**
 * A bare nanosecond count as a figure a person reads.
 *
 * ⛔ NOT ALWAYS MILLISECONDS — `ratio_nav::closure::human_nanos` says why: a
 * small period end lands in microseconds, and printing that as `0 ms` reads as
 * "instant" or "broken" rather than as a small number.
 *
 * ⚠ Unlike `micros`, this takes a plain integer count rather than a proto3
 * Duration, because a plan's figures are `Int64` like everything else on that
 * message. Parsed as a Number for the reason `micros` is: it is a duration,
 * nobody is paid on it, and a nanosecond count stays exact in a double for
 * about a century.
 */
export function nanos(n: string): string {
  if (n === "") return "—";
  const v = Number(n);
  if (!Number.isFinite(v) || v < 0) return "—";
  if (v < 1_000) return `${v} ns`;
  if (v < 1_000_000) return `${Math.round(v / 1_000)} µs`;
  if (v < 1_000_000_000) return `${(v / 1_000_000).toFixed(1)} ms`;
  return `${(v / 1_000_000_000).toFixed(1)} s`;
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

/**
 * How a view's recognition basis reads on a screen.
 *
 * ⛔ `RECORDED` IS NOT "T+0" AND MUST NEVER BE PRINTED AS ONE. It is the
 * journal's own order — no date consulted — which is what every book has always
 * done. A same-day settlement convention is an ELECTION: it reads the calendar
 * and refuses an entry with no trade date. Collapsing the two is
 * `lotMethod: None` versus `Some(Fifo)` one layer out, and that one reached a
 * live screen.
 */
export const BASIS_LABEL: Record<string, string> = {
  RECORDED: "journal order",
  TRADE: "trade date",
  SETTLEMENT: "settled",
  UNSPECIFIED: "—",
};

/** `settled T+2`, or the bare basis where no lag applies. */
export function basisOf(basis: string, settlementOpenDays: string): string {
  return basis === "SETTLEMENT"
    ? `settled T+${settlementOpenDays}`
    : (BASIS_LABEL[basis] ?? "—");
}
