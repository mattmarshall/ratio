"use client";

/**
 * ⛔ SAY WHAT WENT WRONG, NOT "SOMETHING WENT WRONG". The API puts a sentence in
 * `error` — "the journal already has this event id", "no fund" — and the whole
 * claim of this product is that a figure can be accounted for, which has to
 * include the figure not being there. A page that swallows the sentence is
 * arguing against the thing it is a console for.
 *
 * ⚠ Next redacts a server error's message in production and replaces it with a
 * digest, so what reaches here on a server crash is deliberately opaque. That is
 * why every EXPECTED refusal is returned as a value rather than thrown — see
 * `record/actions.ts`. This screen is for the unexpected ones.
 */
export default function Error({
  error,
  reset,
}: {
  error: Error & { digest?: string };
  reset: () => void;
}) {
  return (
    <div className="empty err">
      <p>{error.message || "The console could not read that."}</p>
      {error.digest ? <p className="p2 num">{error.digest}</p> : null}
      <button type="button" className="chip" onClick={reset}>
        Try again
      </button>
    </div>
  );
}
