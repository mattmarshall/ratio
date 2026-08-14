// What a trade ticket is worth, derived the way the server derives it.
//
// ⛔ INTEGERS ONLY, AND THAT IS NOT A STYLE PREFERENCE. `lib/format.ts` opens by
// saying nothing there parses money into a number, and this file is the harder
// half of the same rule: it has to MULTIPLY. A ticket is units × price, and
// `1000 * 1.005` in a double is 1004.9999999999999 — a cent short on a figure
// that becomes a journal entry. Everything below is `bigint` over the digits.
//
// ⭐ AND IT IS A MIRROR, NOT AN INVENTION. Two functions in the Rust decide this
// figure, and both are reproduced here on purpose:
//
//   * `ratio_common::parse_minor` — a decimal string into minor units, by
//     splitting on the point. Its own comment records that there were briefly
//     two money parsers "written months apart and agreeing by luck rather than
//     by construction". This is a third, so it agrees by CONSTRUCTION: the same
//     split, the same two-decimal ceiling, the same refusals.
//   * `ratio_ingest::posting_for` — quantity × price when a template's amount is
//     `consideration`. Both operands are scaled by a hundred, so the product is
//     scaled by ten thousand and has to come back down — and a product that will
//     not come down evenly is REFUSED rather than rounded, because which way to
//     round is a term of an administration agreement.
//
// ⚠ WHY THE BROWSER RUNS THIS AT ALL. The ticket has to say what it is worth
// while it is being typed — a form that reveals the consideration only after a
// round trip is a form people post to find out. The authoritative derivation is
// still the server's: `actions.ts` calls the same function before it sends, and
// the server re-parses the decimal string it is sent. Three derivations, one
// algorithm, no floats anywhere in the chain.

/** A derived figure, or the sentence explaining why there is not one. */
export type Derived =
  | { readonly ok: true; readonly minor: bigint }
  | { readonly ok: false; readonly error: string };

/**
 * A decimal string as a person types it, into hundredths.
 *
 * The console's mirror of `ratio_common::parse_minor`, refusal for refusal. A
 * third decimal place is rejected rather than dropped: the books are kept in
 * minor units, and quietly discarding a digit is worse than saying so.
 */
export function hundredths(text: string, what: string): Derived {
  const t = text.trim().replace(/,/g, "").replace(/\$/g, "");
  const neg = t.startsWith("-");
  const digits = neg ? t.slice(1) : t.startsWith("+") ? t.slice(1) : t;
  if (digits === "") return { ok: false, error: `${what} is required.` };

  const point = digits.indexOf(".");
  const whole = point === -1 ? digits : digits.slice(0, point);
  const frac = point === -1 ? "" : digits.slice(point + 1);

  if (whole === "" && frac === "") {
    return { ok: false, error: `${text.trim()} is not ${what}.` };
  }
  if (frac.length > 2) {
    return {
      ok: false,
      error: `${what} has more than two decimal places; the books are kept in minor units.`,
    };
  }
  if (!/^\d*$/.test(whole) || !/^\d*$/.test(frac)) {
    return { ok: false, error: `${text.trim()} is not ${what}.` };
  }

  const v =
    BigInt(whole === "" ? 0 : whole) * 100n +
    BigInt(frac === "" ? 0 : frac.padEnd(2, "0"));
  return { ok: true, minor: neg ? -v : v };
}

/**
 * Units × price, in minor units of the fund's currency.
 *
 * ⛔ REFUSES RATHER THAN ROUNDS. Both operands are scaled by a hundred, so the
 * product is scaled by ten thousand; one that is not a whole number of minor
 * units is a rounding decision, and the configuration has not declared one.
 * `ratio_ingest::posting_for` refuses it on the data plane in these words, and a
 * ticket that quietly rounded what a file cannot would be the same trade booking
 * two different figures depending on which door it came through.
 */
export function consideration(units: string, price: string): Derived {
  const q = hundredths(units, "a quantity");
  if (!q.ok) return q;
  const p = hundredths(price, "a price");
  if (!p.ok) return p;
  if (q.minor < 0n) return { ok: false, error: "A quantity is not negative — the side says which way it goes." };
  if (p.minor < 0n) return { ok: false, error: "A price is not negative." };

  const scaled = q.minor * p.minor;
  if (scaled % 100n !== 0n) {
    return {
      ok: false,
      error:
        `quantity x price is ${scaled} ten-thousandths, which is not a whole number of ` +
        `minor units — posting it would take a rounding decision the configuration has not declared`,
    };
  }
  return { ok: true, minor: scaled / 100n };
}

/**
 * Minor units as the decimal string `ApplyEventRequest.amount` asks for.
 *
 * ⛔ MAJOR UNITS WITH A POINT — `"250000.00"`, not `"25000000"`. The contract
 * says so and `parse_minor` implements it: `"42"` reaches the journal as 42.00,
 * four thousand two hundred minor units. A form that labels this field "minor
 * units" is a form that books every figure a hundred times too large, and the
 * entry balances perfectly either way.
 */
export function decimalOf(minor: bigint): string {
  const neg = minor < 0n;
  const d = (neg ? -minor : minor).toString().padStart(3, "0");
  return `${neg ? "-" : ""}${d.slice(0, -2)}.${d.slice(-2)}`;
}

/** `2026-02-26` — a calendar day, which is what a trade date is. */
export const TRADE_DATE = /^\d{4}-\d{2}-\d{2}$/;

/**
 * Units as a whole number, which is what a tax lot is kept in.
 *
 * ⛔ REFUSED HERE BECAUSE IT IS REFUSED THERE. `ApplyEventRequest.quantity`
 * takes whole units; `Console::apply_event` bails on anything else rather than
 * carrying it as no quantity, which is what the data plane does for a file
 * nobody read. A form that let a fractional quantity through would collect the
 * refusal a round trip later, having already computed a consideration from it.
 */
export function wholeUnits(units: string): Derived {
  const q = hundredths(units, "a quantity");
  if (!q.ok) return q;
  if (q.minor < 0n) {
    return { ok: false, error: "A quantity is not negative — the side says which way it goes." };
  }
  if (q.minor % 100n !== 0n) {
    return {
      ok: false,
      error:
        "A lot is kept in whole units, so a fractional quantity has to arrive " +
        "on the data plane rather than through this form.",
    };
  }
  return { ok: true, minor: q.minor / 100n };
}

/** `2026-02-26` → the `CalendarDate` the contract asks for, or null. */
export function calendarDate(
  iso: string,
): { year: number; month: number; day: number } | null {
  if (!TRADE_DATE.test(iso)) return null;
  return {
    year: Number(iso.slice(0, 4)),
    month: Number(iso.slice(5, 7)),
    day: Number(iso.slice(8, 10)),
  };
}

/** The seven inputs that decide what a ticket posts. */
export interface TicketInputs {
  readonly side: string;
  readonly instrument: string;
  readonly units: string;
  readonly price: string;
  readonly tradeDate: string;
  readonly ruleId: string;
  readonly reference: string;
}

/**
 * The inputs a preview answers for.
 *
 * ⭐ SO THAT "PREVIEW, THEN POST" IS ENFORCED RATHER THAN SUGGESTED. Every write
 * screen in this console previews first, and every one of them will let an
 * operator preview a ticket, change the price, and post the changed one — the
 * preview stays on screen looking like a description of what the button is about
 * to do. The ticket compares this string against the one the server echoed back
 * and disables Post when they differ, so the figures on screen are always the
 * figures the button will write.
 *
 * ⚠ IN `lib/`, NOT IN `actions.ts`. A `"use server"` module may only export
 * async functions, so a signature living there would be a network round trip on
 * every keystroke to compute a string join.
 */
export function signature(t: TicketInputs): string {
  return [t.side, t.instrument, t.units, t.price, t.tradeDate, t.ruleId, t.reference]
    .map((s) => s.trim())
    .join(" ");
}

/**
 * A reference the server will accept, suggested from the ticket itself.
 *
 * ⚠ THE ONE PLACE THE TRADE DATE REACHES THE JOURNAL, and it reaches it as
 * TEXT. `JournalEntry.trade_date` is what the lot engine reads to establish a
 * holding period, and `ApplyEventRequest` has no field that fills it — so the
 * date here is legible to a person and invisible to `Ratio.Lots.Relief`. The
 * ticket says as much where it says what will not be carried.
 *
 * The charset is `Console::apply_event`'s: letters, digits, `-`, `_`, `.`, and
 * at most sixty-four of them, because the id reaches a filename-shaped
 * identifier and a screen other people read.
 */
export function suggestedReference(
  side: "buy" | "sell",
  instrument: string,
  tradeDate: string,
): string {
  const clean = (s: string) => s.replace(/[^A-Za-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "");
  const parts = [side === "buy" ? "BUY" : "SEL", clean(instrument), clean(tradeDate)];
  return parts.filter(Boolean).join("-").slice(0, 64);
}
