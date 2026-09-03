import "server-only";

import { Refused } from "@/wire/client";

/**
 * A refusal, surfaced as a VALUE a page can render.
 *
 * ⛔ BECAUSE A THROWN REFUSAL IS REDACTED TO A NUMBER. Next strips a server
 * error's message in production and replaces it with a digest — so the one
 * sentence the API crafted to explain itself ("view `emir` is not on this
 * fund…", "two views cannot be reconciled while either has entries it cannot
 * place…") reached the person as `Minified React error #441` and an opaque
 * digest. That shipped, on the dual-basis demo fund's every view screen.
 * `error.tsx` already states the rule this file enforces: every EXPECTED
 * refusal is returned as a value rather than thrown; the error boundary is for
 * the unexpected ones.
 *
 * ⚠ `NotFound` and `AuthError` deliberately still throw — `or404` and
 * `orAuth` are their handlers, and a refusal is neither: the resource
 * exists, the caller may see it, and the answer is a sentence about why there
 * is no figure.
 */
export type OrRefused<T> = { refused: null; value: T } | { refused: string };

export async function orRefused<T>(p: Promise<T>): Promise<OrRefused<T>> {
  try {
    return { refused: null, value: await p };
  } catch (e) {
    if (e instanceof Refused) return { refused: e.message };
    throw e;
  }
}
