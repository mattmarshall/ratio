"use server";

import { revalidatePath } from "next/cache";
import { caller } from "@/lib/caller";
import { AuthError, markPositions, Refused } from "@/wire/client";
import type { MarkPositionsResponse } from "@/wire/types";

export type MarkResult =
  | { ok: true; response: MarkPositionsResponse; signature: string }
  | { ok: false; error: string }
  | null;

/** `2026-02-26` — the only date shape this form accepts. */
const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

/**
 * Mark positions to market, or preview what marking would do.
 *
 * ⚠ A POSITION ALREADY AT MARKET MOVES BY NOTHING, so a second mark at one
 * price is a no-op rather than a second gain. That is a property of the server,
 * not of this form — but it is why previewing twice is safe.
 */
export async function mark(
  _prev: MarkResult,
  form: FormData,
): Promise<MarkResult> {
  const fund = String(form.get("fund") ?? "");
  const raw = String(form.get("valuationDate") ?? "").trim();
  const validateOnly = form.get("commit") === null;

  const m = DATE.exec(raw);
  if (!m) return { ok: false, error: "The valuation date is YYYY-MM-DD." };

  try {
    const c = await caller();
    const response = await markPositions(c, fund, {
      valuationDate: {
        year: Number(m[1]),
        month: Number(m[2]),
        day: Number(m[3]),
      },
      validateOnly,
    });
    if (!validateOnly) revalidatePath(`/funds/${fund}`, "layout");
    // ⭐ THE INPUT THIS ANSWERS FOR, so the commit stays shut until a preview
    // has come back for exactly it. Otherwise "preview, then post" is a
    // sentence on the screen rather than a property of it: preview one date,
    // type another, and the marks on screen describe neither.
    return { ok: true, response, signature: raw };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}
