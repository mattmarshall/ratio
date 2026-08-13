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
