import type { ReactNode } from "react";
import { Unavailable } from "@/components/Unavailable";
import { Refused } from "@/wire/client";

/**
 * A refusal, rendered as the sentence the API wrote.
 *
 * The same shape `error.tsx` uses — but this one keeps the message, because it
 * arrived as a value instead of being thrown through Next's production
 * redaction. No "Try again": a refusal is an answer, not a failure, and it
 * will say the same thing twice.
 */
export function Refusal({ why }: { why: string }) {
  return (
    <div className="empty err" role="status">
      {why}
    </div>
  );
}

/**
 * Render a page, and render a `Refused` as its sentence.
 *
 * ⛔ BECAUSE A THROWN REFUSAL IS REDACTED TO A NUMBER IN PRODUCTION. Next
 * strips a server error's message and substitutes a digest, so the API's one
 * explanatory sentence reached the person as `Minified React error #441` —
 * on every view screen of the dual-basis demo fund, the fund that exists to
 * demonstrate this feature. `error.tsx` states the rule this enforces: every
 * EXPECTED refusal is a value, and the error boundary is for the unexpected.
 *
 * ⚠ `notFound()`'s control-flow throw, `AuthError` (a missing session
 * — `orAuth` turns that into a redirect), and genuine crashes pass
 * through. A 401 *with* a session is `Refused(401)` after `#253`; a
 * 5xx is the other `#441` door. Both are a status, not a figure and
 * not `/signin`. Flattening a real defect into a polite sentence
 * would hide it; flattening a held-session 401 into an overlay is
 * how `/books/[book]` came back as `#441`.
 */
export function withRefusal<P>(
  page: (props: P) => Promise<ReactNode>,
): (props: P) => Promise<ReactNode> {
  return async function refusable(props: P): Promise<ReactNode> {
    try {
      return await page(props);
    } catch (e) {
      if (e instanceof Refused) {
        if (e.status === 401 || e.status >= 500) {
          return <Unavailable why={e.message} />;
        }
        return <Refusal why={e.message} />;
      }
      throw e;
    }
  };
}
