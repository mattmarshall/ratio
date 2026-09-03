/**
 * ⚠ HONEST ABOUT WHY IT IS SLOW. Every navigation here is a fresh server
 * render issuing upstream calls, and the API is a Lambda whose cold start
 * copies a book to /tmp before it answers. Under the single-page console that
 * cost was paid once per session; now it can be paid again after an idle spell.
 * Provisioned concurrency would hide it and would also bill at rest, which is
 * the one thing this deployment is shaped to avoid.
 */
export default function Loading() {
  return <div className="empty">Reading the book…</div>;
}
