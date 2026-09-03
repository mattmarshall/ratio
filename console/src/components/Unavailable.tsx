/**
 * A transport failure, rendered as a sentence the operator can retry.
 *
 * ⚠ THIS IS NOT `Refusal`. A 400 is an answer and will say the same
 * thing twice; a 503 is the API rolling or briefly unwell, and trying
 * again is the right move. `error.tsx` cannot carry the sentence —
 * Next redacts a thrown server error to `#441` — so the read helper
 * returns this as a value instead of throwing.
 */
export function Unavailable({ why }: { why: string }) {
  const detail = /^\d{3}$/.test(why) ? null : why;
  return (
    <div className="empty err" role="status">
      <p>The API is temporarily unavailable.</p>
      {detail ? <p className="p2">{detail}</p> : null}
      <form>
        <button type="submit" className="chip">
          Try again
        </button>
      </form>
    </div>
  );
}
