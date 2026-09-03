"use server";

import { revalidatePath } from "next/cache";
import { caller } from "@/lib/caller";
import {
  calendarDate,
  consideration,
  decimalOf,
  signature,
  TRADE_DATE,
  wholeUnits,
} from "@/lib/trade";
import { applyEvent, AuthError, Refused } from "@/wire/client";
import type { ApplyEventResponse } from "@/wire/types";

/** The ticket as it was sent, echoed back beside what the server made of it. */
export interface Ticket {
  readonly side: "buy" | "sell";
  readonly instrument: string;
  readonly units: string;
  readonly price: string;
  readonly tradeDate: string;
  readonly ruleId: string;
  readonly reference: string;
  /** The consideration, in major units with a point, exactly as sent. */
  readonly amount: string;
  /** The inputs this response answers for. See `lib/trade.ts`. */
  readonly signature: string;
}

/** What the ticket gets back. ⛔ Never a thrown error — see `place`. */
export type TradeResult =
  | { ok: true; ticket: Ticket; response: ApplyEventResponse }
  | { ok: false; error: string }
  | null;

/**
 * Place a trade, or preview what placing it would do.
 *
 * ⛔ ERRORS COME BACK AS VALUES. In production Next replaces an uncaught Server
 * Action error with an opaque digest, and the sentence the server wrote is what
 * this console exists to show — "the journal already has this event id", "this
 * book is at its ceiling of 250 entries". A digest is the opposite of a figure
 * that can be accounted for.
 *
 * ⛔ THE SIDE IS A RULE, NOT A SIGN. A sale is not a purchase with a minus in
 * front of it: the consideration is positive both ways, and the CONFIGURATION
 * decides which accounts move and in which direction — `disposal_proceeds` is a
 * different rule from `equity_purchase`, approved separately. Negating the
 * amount on a purchase rule posts a purchase backwards; the entry balances, the
 * trial balance ties, and nothing downstream objects.
 */
export async function place(
  _prev: TradeResult,
  form: FormData,
): Promise<TradeResult> {
  const fund = String(form.get("fund") ?? "");
  const side = String(form.get("side") ?? "") === "sell" ? "sell" : "buy";
  const instrument = String(form.get("instrument") ?? "").trim();
  const units = String(form.get("units") ?? "").trim();
  const price = String(form.get("price") ?? "").trim();
  const tradeDate = String(form.get("tradeDate") ?? "").trim();
  const ruleId = String(form.get("ruleId") ?? "").trim();
  const reference = String(form.get("reference") ?? "").trim();
  // ⚠ The default is a PREVIEW. A form that posts by default is a form that
  // posts by accident.
  const validateOnly = form.get("commit") === null;

  if (!instrument) return { ok: false, error: "Name the instrument traded." };
  if (!TRADE_DATE.test(tradeDate)) {
    return { ok: false, error: "The trade date is YYYY-MM-DD." };
  }
  if (!ruleId) return { ok: false, error: "Choose the rule this books under." };
  if (!reference) return { ok: false, error: "A trade needs a reference." };

  // ⛔ DERIVED HERE TOO, BY THE FUNCTION THE TICKET ALREADY SHOWED. The
  // browser's figure is a courtesy; this one is what gets sent. The two cannot
  // disagree, because there is one implementation and it is integer arithmetic.
  const c = consideration(units, price);
  if (!c.ok) return { ok: false, error: c.error };
  if (c.minor === 0n) {
    return {
      ok: false,
      error:
        "This ticket is worth nothing. An entry for zero records that a trade " +
        "happened and moves no figure, which is a claim rather than a trade.",
    };
  }
  const amount = decimalOf(c.minor);

  // ⛔ WHOLE UNITS, CHECKED HERE AS WELL AS THERE. `Console::apply_event` bails
  // on a fractional quantity rather than carrying it as none, and a form that
  // sent one would collect that refusal a round trip after computing a
  // consideration from it.
  const q = wholeUnits(units);
  if (!q.ok) return { ok: false, error: q.error };

  try {
    const who = await caller();
    const response = await applyEvent(who, fund, {
      ruleId,
      // The reference is the event id, and that is what makes recording the
      // same trade twice a refusal rather than a second position.
      eventId: reference,
      // ⛔ A DECIMAL STRING IN MAJOR UNITS, which is what the contract asks for
      // and what `ratio_common::parse_minor` implements: `"42"` reaches the
      // journal as forty-two pounds, not forty-two pence. Sending minor units
      // books every trade a hundred times too large, and it balances.
      amount,
      days: "",
      // ⭐ THE THREE THAT MAKE THIS A TRADE RATHER THAN A MOVEMENT OF VALUE.
      // Without them the postings name accounts and nothing else, the walk
      // skips them, and no lot opens or is relieved — while the entry balances
      // and the trial balance ties.
      instrument,
      quantity: q.minor.toString(),
      tradeDate: calendarDate(tradeDate),
      validateOnly,
    });
    if (!validateOnly) {
      // ⭐ THE WHOLE FUND, NOT A LIST SOMEBODY REMEMBERED. On a commit the NAV,
      // the trial balance, the positions and the change log are all stale, and
      // this has to stay a prefix.
      revalidatePath(`/books/${fund}`, "layout");
    }
    return {
      ok: true,
      response,
      ticket: {
        side,
        instrument,
        units,
        price,
        tradeDate,
        ruleId,
        reference,
        amount,
        signature: signature({
          side,
          instrument,
          units,
          price,
          tradeDate,
          ruleId,
          reference,
        }),
      },
    };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}
