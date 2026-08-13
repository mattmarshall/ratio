"use server";

import { revalidatePath } from "next/cache";
import { caller } from "@/lib/caller";
import { applyEvent, AuthError, Refused } from "@/wire/client";
import type { ApplyEventResponse } from "@/wire/types";

/** What a form gets back. ⛔ Never a thrown error — see `submit`. */
export type Result =
  | { ok: true; response: ApplyEventResponse }
  | { ok: false; error: string }
  | null;

/** A whole number of minor units, with an optional sign. */
const AMOUNT = /^-?\d+$/;

/**
 * Record an event, or preview what recording it would do.
 *
 * ⛔ ERRORS COME BACK AS VALUES, NOT AS THROWS. In production Next replaces an
 * uncaught Server Action error with an opaque digest, and the sentence the
 * server wrote is exactly what this console must show: `ratio-console` puts
 * prose in `error` — "the journal already has this event id", "this book is at
 * its ceiling of 250 entries" — and the whole claim of the product is that a
 * figure can be accounted for, which has to include the figure not being there.
 * A digest is the opposite of that.
 *
 * ⛔ AND THE AMOUNT IS VALIDATED AS A STRING. It is minor units as an int64,
 * sent as a string precisely so nothing makes it a double; `parseFloat` here to
 * "check it is a number" would undo that in one character, on the one screen
 * where the value becomes a journal entry.
 */
export async function submit(_prev: Result, form: FormData): Promise<Result> {
  const fund = String(form.get("fund") ?? "");
  const ruleId = String(form.get("ruleId") ?? "");
  const eventId = String(form.get("eventId") ?? "");
  const amount = String(form.get("amount") ?? "").trim();
  const days = String(form.get("days") ?? "").trim();
  // ⚠ The default is a PREVIEW. A form that posts by default is a form that
  // posts by accident.
  const validateOnly = form.get("commit") === null;

  if (!ruleId) return { ok: false, error: "Choose a rule." };
  if (!AMOUNT.test(amount)) {
    return { ok: false, error: "The amount is minor units, as whole digits." };
  }
  if (days && !/^\d+$/.test(days)) {
    return { ok: false, error: "Days is a whole number." };
  }

  try {
    const c = await caller();
    const response = await applyEvent(c, fund, {
      ruleId,
      eventId,
      amount,
      days,
      validateOnly,
    });
    if (!validateOnly) {
      // ⭐ THE WHOLE FUND, NOT A LIST SOMEBODY REMEMBERED. On a real commit the
      // NAV, the trial balance, the strikes and the change log are all stale.
      // This is the server-rendered translation of the old console's
      // `invalidateQueries({ queryKey: [fund] })`, and it has to stay a prefix.
      revalidatePath(`/funds/${fund}`, "layout");
    }
    return { ok: true, response };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}
