"use server";

import { revalidatePath } from "next/cache";
import { caller } from "@/lib/caller";
import { calendarDate, hundredths, TRADE_DATE } from "@/lib/trade";
import { applyEvent, AuthError, Refused } from "@/wire/client";
import type { ApplyEventResponse } from "@/wire/types";

export type Result =
  | { ok: true; response: ApplyEventResponse; signature: string }
  | { ok: false; error: string }
  | null;

/**
 * Post a household transfer, or preview what posting it would do.
 *
 * ⛔ NO INSTRUMENT, NO QUANTITY. Sending either would make this a trade: the
 * projection opens a lot only when a posting carries both, and a transfer
 * that claimed relief would be a sale wearing a household label.
 *
 * ⛔ THE DATE IS REQUIRED. An undated entry is in no period P&L — the same
 * refusal as a lot with no acquisition date. Leaving it blank to "let the
 * server guess" is how a March spend lands in no month and looks like the
 * books still tie.
 */
export async function submit(_prev: Result, form: FormData): Promise<Result> {
  const fund = String(form.get("fund") ?? "");
  const ruleId = String(form.get("ruleId") ?? "");
  const amount = String(form.get("amount") ?? "").trim();
  const day = String(form.get("date") ?? "").trim();
  let eventId = String(form.get("eventId") ?? "").trim();
  const validateOnly = form.get("commit") === null;

  if (!ruleId) return { ok: false, error: "Choose where the money moves." };
  if (!ruleId.startsWith("xfer_")) {
    return { ok: false, error: "That rule is not a transfer." };
  }
  const parsed = hundredths(amount, "an amount");
  if (!parsed.ok) return { ok: false, error: parsed.error };
  if (!TRADE_DATE.test(day)) {
    return { ok: false, error: "The date is YYYY-MM-DD." };
  }
  const tradeDate = calendarDate(day);
  if (!tradeDate) return { ok: false, error: "The date is YYYY-MM-DD." };
  if (!eventId) {
    // Named here rather than by the server: ApplyEvent refuses an empty id.
    eventId = `xfer-${day}`;
  }

  try {
    const c = await caller();
    const response = await applyEvent(c, fund, {
      ruleId,
      eventId,
      amount,
      days: "",
      instrument: "",
      quantity: "",
      tradeDate,
      validateOnly,
    });
    if (!validateOnly) {
      revalidatePath(`/books/${fund}`, "layout");
    }
    return {
      ok: true,
      response,
      signature: [ruleId, amount, day, eventId].map((s) => s.trim()).join(" "),
    };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}
