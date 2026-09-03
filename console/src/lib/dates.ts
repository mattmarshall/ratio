/** `{year, month, day}` as `26 Feb 2026`, or an em dash when absent.
 *
 * ⚠ Not `Intl.DateTimeFormat`, and not a `Date`. A `CalendarDate` is a day in a
 * fund's calendar, not an instant — putting it through a `Date` gives it a
 * timezone it does not have, and a trade date that moves by one day depending on
 * where the reader is sitting is a different trade date. */
export function isoDate(
  d: { year: number; month: number; day: number } | null,
): string {
  if (!d) return "—";
  const months = "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split(" ");
  return `${d.day} ${months[d.month - 1] ?? "?"} ${d.year}`;
}

const MONTHS = "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split(" ");

/** UTC calendar month `YYYY-MM`. A `Date` is an instant; this is a month. */
export function utcMonth(d: Date = new Date()): string {
  const y = d.getUTCFullYear();
  const m = String(d.getUTCMonth() + 1).padStart(2, "0");
  return `${y}-${m}`;
}

/** UTC calendar year `YYYY`. */
export function utcYear(d: Date = new Date()): string {
  return String(d.getUTCFullYear());
}

/** `2026-09` → `Sep 2026`; `2026` → `2026`. */
export function periodLabel(spec: string): string {
  if (/^\d{4}$/.test(spec)) return spec;
  const m = /^(\d{4})-(\d{2})$/.exec(spec);
  if (!m) return spec;
  const month = MONTHS[Number(m[2]) - 1];
  return month ? `${month} ${m[1]}` : spec;
}

/** Previous calendar month of `YYYY-MM`, UTC arithmetic on the digits. */
export function previousMonth(spec: string): string {
  const m = /^(\d{4})-(\d{2})$/.exec(spec);
  if (!m) return spec;
  let y = Number(m[1]);
  let mo = Number(m[2]) - 1;
  if (mo < 1) {
    mo = 12;
    y -= 1;
  }
  return `${String(y).padStart(4, "0")}-${String(mo).padStart(2, "0")}`;
}
