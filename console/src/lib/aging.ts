// Operating AR/AP aging presentation: parse the wire, never invent a bucket.
//
// ⛔ INTEGER STRINGS, NEVER A NUMBER. The server sends minor units as a
// string so the browser cannot silently make it a double. `Number` on
// a control balance would undo that on the one screen aging is cited.
//
// ⭐ EMPTY MEANS UNSET. A missing due date is not current. `"0"` is a
// real empty dated window. `undated` empty on a set schedule is no
// residual line, not a fake zero residual.

import { money } from "./format";
import type { AgingSchedule } from "@/wire/types";

export type AgingBucket =
  | "current"
  | "days130"
  | "days3160"
  | "days6190"
  | "daysOver90"
  | "undated";

export type AgingSide = "receivable" | "payable";

/** Empty wire string → null. `"0"` is a real zero. */
export function parseBucket(raw: string): bigint | null {
  return raw === "" ? null : BigInt(raw);
}

/** A schedule is set when `current` is present — including `"0"`. */
export function scheduleIsSet(s: AgingSchedule | null | undefined): boolean {
  return !!s && s.current !== "";
}

/**
 * Dated buckets plus undated residual equal the control.
 *
 * Unset schedules do not foot — there is nothing to add. A set
 * schedule with empty `undated` treats the residual as zero (no line).
 */
export function scheduleFoots(s: AgingSchedule | null | undefined): boolean {
  if (!scheduleIsSet(s) || !s) return false;
  const dated = [
    s.current,
    s.days130,
    s.days3160,
    s.days6190,
    s.daysOver90,
  ].reduce((n, raw) => n + BigInt(raw), 0n);
  const undated = s.undated === "" ? 0n : BigInt(s.undated);
  if (s.control === "") return false;
  return dated + undated === BigInt(s.control);
}

/**
 * Which due-date window an item falls in.
 *
 * ⛔ NULL IS UNDATED, NEVER CURRENT. Defaulting a missing due date to
 * the as-of day — or to the trade date — would put every undated
 * invoice in current the day it was raised.
 */
export function bucketOf(dueDate: string | null, asOf: string): AgingBucket {
  if (!dueDate) return "undated";
  const due = daysFromIso(dueDate);
  const asOfDays = daysFromIso(asOf);
  if (due === null || asOfDays === null) return "undated";
  const past = asOfDays - due;
  if (past <= 0) return "current";
  if (past <= 30) return "days130";
  if (past <= 60) return "days3160";
  if (past <= 90) return "days6190";
  return "daysOver90";
}

/** Days since Unix epoch from `YYYY-MM-DD`, or null. Civil, not a `Date`. */
export function daysFromIso(iso: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!m) return null;
  const y = Number(m[1]);
  const month = Number(m[2]);
  const d = Number(m[3]);
  if (month < 1 || month > 12 || d < 1 || d > 31) return null;
  // Howard Hinnant days_from_civil — same family as `ratio_common`.
  const y2 = y - (month <= 2 ? 1 : 0);
  const era = Math.floor(y2 / 400);
  const yoe = y2 - era * 400;
  const doy = Math.floor((153 * (month + (month > 2 ? -3 : 9)) + 2) / 5) + d - 1;
  const doe = yoe * 365 + Math.floor(yoe / 4) - Math.floor(yoe / 100) + doy;
  return era * 146097 + doe - 719468;
}

/**
 * One bucket or the control, as a reader expects it.
 *
 * AR is debit-normal (the raw figure). AP is credit-normal (flipped),
 * the same way the sheet shows a payable.
 */
export function agingShown(
  raw: bigint | null,
  side: AgingSide,
): string {
  if (raw === null) return "—";
  return side === "receivable" ? money(raw.toString()) : money((-raw).toString());
}
