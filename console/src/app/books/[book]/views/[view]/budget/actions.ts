"use server";

import { revalidatePath } from "next/cache";
import { isBudgetJournalRule } from "@/lib/budgetPost";
import { caller } from "@/lib/caller";
import { calendarDate, hundredths, TRADE_DATE } from "@/lib/trade";
import { applyEvent, AuthError, Refused } from "@/wire/client";
import type { ApplyEventResponse } from "@/wire/types";

/** What the `/budget` post form gets back. ⛔ Never a thrown error — see `submit`. */
export type Result =
  | { ok: true; response: ApplyEventResponse; signature: string }
  | { ok: false; error: string }
  | null;

/**
 * Post an approved change order or award, or preview what posting it would do.
 *
 * ⛔ THE SAME `ApplyEvent` `/record` USES, AND THE SAME RULE IDS CreateBook
 * seeds. This action is an allowlist over those ids, not a second budget
 * write. `project_cost` / `equity_purchase` stay refused here — a cost or
 * a lot opened from this page would be a parallel book wearing `/budget`.
 *
 * ⛔ ERRORS COME BACK AS VALUES. Production Next replaces an uncaught Server
 * Action error with an opaque digest, and the sentence `ratio-console` wrote
 * is the one this screen must show.
 *
 * ⚠ THE DEFAULT IS A PREVIEW. A form that posts by default is a form that
 * posts by accident.
 */
export async function submit(_prev: Result, form: FormData): Promise<Result> {
  const fund = String(form.get("fund") ?? "");
  const ruleId = String(form.get("ruleId") ?? "").trim();
  const eventId = String(form.get("eventId") ?? "").trim();
  const amount = String(form.get("amount") ?? "").trim();
  const dated = String(form.get("dated") ?? "").trim();
  const validateOnly = form.get("commit") === null;

  if (!ruleId) return { ok: false, error: "Choose a change order or award." };
  if (!isBudgetJournalRule(ruleId)) {
    return {
      ok: false,
      error:
        "This page posts approve_co / deduct_co / award_commitment / release_commitment, keyed by phase. A cost or a trade is a different screen.",
    };
  }
  if (!eventId) {
    return { ok: false, error: "Name the change order or purchase order." };
  }
  const parsed = hundredths(amount, "an amount");
  if (!parsed.ok) return { ok: false, error: parsed.error };
  if (dated && !TRADE_DATE.test(dated)) {
    return { ok: false, error: "The date is YYYY-MM-DD, or left blank." };
  }
  const tradeDate = dated ? calendarDate(dated) : null;
  if (dated && tradeDate === null) {
    return { ok: false, error: "The date is YYYY-MM-DD, or left blank." };
  }

  try {
    const c = await caller();
    const response = await applyEvent(c, fund, {
      ruleId,
      eventId,
      amount,
      days: "",
      // ⚠ INSTRUMENT AND QUANTITY STAY EMPTY. A change order / award is
      // an equity pair. `/trade` is the lot screen; a quantity here would
      // claim relief the walk-through already asserts these rules refuse.
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
      signature: [ruleId, amount, dated, eventId].map((s) => s.trim()).join(" "),
    };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}
