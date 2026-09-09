import { SESSION_REFUSED } from "@/lib/sessionRefused";
import { HYDRATING } from "@/wire/hydrate";

/**
 * A transport failure, rendered as a sentence the operator can retry.
 *
 * ⚠ THIS IS NOT `Refusal`. A 400 is an answer and will say the same
 * thing twice; a 503 is the API rolling or briefly unwell, and trying
 * again is the right move. `error.tsx` cannot carry the sentence —
 * Next redacts a thrown server error to `#441` — so the read helper
 * returns this as a value instead of throwing.
 *
 * ⚠ A REFUSED SESSION IS THE OTHER VALUE THIS RENDERS. The heading
 * changes because "temporarily unavailable" is a lie when AuthKit
 * already has a user and the gateway will not accept the bearer.
 *
 * ⚠ A HYDRATING 503 IS ALSO NOT "UNAVAILABLE". `send()` retries it;
 * this copy is the exhausted leftover. "Temporarily unavailable" is
 * a lie when the journal is still opening and Retry-After still applies.
 */
export function Unavailable({ why }: { why: string }) {
  const sessionRefused = why === SESSION_REFUSED;
  const hydrating = why.includes("hydrating") || why === HYDRATING;
  const detail = /^\d{3}$/.test(why) ? null : why;
  return (
    <div className="empty err" role="status">
      <p>
        {sessionRefused
          ? "This session is signed in, but the API did not accept it."
          : hydrating
            ? "The journal is still opening."
            : "The API is temporarily unavailable."}
      </p>
      {detail && !sessionRefused ? <p className="p2">{detail}</p> : null}
      <form>
        <button type="submit" className="chip">
          Try again
        </button>
      </form>
    </div>
  );
}
