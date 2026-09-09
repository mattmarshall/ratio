/**
 * ⚠ HONEST ABOUT THE COLD START. `/books` waits on ListBooks, and a
 * cold Lambda 503s `the journal is still hydrating` for 200ms then
 * Retry-After: 2. `send()` retries that body; this is the sentence
 * while it does. "The API is temporarily unavailable" is the exhausted
 * leftover, not the wait.
 */
export default function Loading() {
  return <div className="empty">Opening the journal…</div>;
}
