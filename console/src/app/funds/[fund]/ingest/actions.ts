"use server";

import { revalidatePath } from "next/cache";
import { caller } from "@/lib/caller";
import { admitFacts, AuthError, ingestDelivery, Refused } from "@/wire/client";
import type { AdmitFactsResponse, IngestDeliveryResponse } from "@/wire/types";

export type ReadResult =
  | { ok: true; response: IngestDeliveryResponse; signature: string }
  | { ok: false; error: string }
  | null;

export type AdmitResult =
  | { ok: true; response: AdmitFactsResponse }
  | { ok: false; error: string }
  | null;

/**
 * Read a file into facts, or preview what reading it would produce.
 *
 * ⛔ Errors as values, never as throws — Next turns an uncaught Server Action
 * error into an opaque digest in production, and the server's sentence is the
 * thing worth showing. Same reasoning as `record/actions.ts`.
 */
export async function read(_prev: ReadResult, form: FormData): Promise<ReadResult> {
  const fund = String(form.get("fund") ?? "");
  const templateId = String(form.get("templateId") ?? "");
  const content = String(form.get("content") ?? "");
  const origin = String(form.get("origin") ?? "");
  const validateOnly = form.get("commit") === null;

  if (!templateId) return { ok: false, error: "Choose a template." };
  if (!content.trim()) return { ok: false, error: "There is nothing to read." };

  try {
    const c = await caller();
    const response = await ingestDelivery(c, fund, {
      templateId,
      content,
      origin,
      validateOnly,
    });
    if (!validateOnly) revalidatePath(`/funds/${fund}`, "layout");
    // ⭐ THE INPUTS THIS ANSWERS FOR, so `Read` stays shut until a preview has
    // come back for exactly them — otherwise the counts on screen can describe
    // one file while the button reads another.
    return { ok: true, response, signature: [templateId, origin.trim(), content].join(" ") };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}

/** Post every fact that fully resolves. */
export async function admit(
  _prev: AdmitResult,
  form: FormData,
): Promise<AdmitResult> {
  const fund = String(form.get("fund") ?? "");
  const validateOnly = form.get("commit") === null;
  try {
    const c = await caller();
    const response = await admitFacts(c, fund, { validateOnly });
    if (!validateOnly) revalidatePath(`/funds/${fund}`, "layout");
    return { ok: true, response };
  } catch (e) {
    if (e instanceof AuthError) return { ok: false, error: "Sign in required." };
    if (e instanceof Refused) return { ok: false, error: e.message };
    return { ok: false, error: "The server did not answer." };
  }
}
