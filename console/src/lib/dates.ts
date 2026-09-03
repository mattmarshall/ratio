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

/** UTC `YYYY-MM` for period chips. A fund's day is a calendar day, not a timezone. */
export function utcMonth(d = new Date()): string {
  return `${d.getUTCFullYear()}-${String(d.getUTCMonth() + 1).padStart(2, "0")}`;
}

export function utcYear(d = new Date()): string {
  return String(d.getUTCFullYear());
}

/** The previous calendar month of a `YYYY-MM`. */
export function previousMonth(ym: string): string {
  const y = Number(ym.slice(0, 4));
  const m = Number(ym.slice(5, 7));
  if (m === 1) return `${y - 1}-12`;
  return `${y}-${String(m - 1).padStart(2, "0")}`;
}

/** `2026-03` → `Mar 2026`; a year is itself. */
export function periodLabel(spec: string): string {
  if (/^\d{4}$/.test(spec)) return spec;
  const months = "Jan Feb Mar Apr May Jun Jul Aug Sep Oct Nov Dec".split(" ");
  const m = Number(spec.slice(5, 7));
  return `${months[m - 1] ?? "?"} ${spec.slice(0, 4)}`;
}
