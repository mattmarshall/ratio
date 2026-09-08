import { SESSION_REFUSED } from "@/lib/sessionRefused";

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
 */
export function Unavailable({ why }: { why: string }) {
  const sessionRefused = why === SESSION_REFUSED;
  const detail = /^\d{3}$/.test(why) ? null : why;
  return (
    <div className="empty err" role="status">
      <p>
        {sessionRefused
          ? "This session is signed in, but the API did not accept it."
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
