import { Brand } from "@/components/Brand";

export const dynamic = "force-dynamic";

/** The console with no session: a prompt, not a wall of refusals.
 *
 * The button is a plain link to `/api/auth/login`, which is the AuthKit
 * initiate-login URL. Nothing about the flow runs in this page. */
export default async function SignIn({
  searchParams,
}: {
  searchParams: Promise<{ returnTo?: string; error?: string }>;
}) {
  const { returnTo, error } = await searchParams;
  const href = returnTo
    ? `/api/auth/login?returnTo=${encodeURIComponent(returnTo)}`
    : "/api/auth/login";

  return (
    <div className="app signin">
      <header className="top">
        <Brand />
      </header>
      <div className="signin-body">
        <h1>Sign in</h1>
        <p>This console shows the books you may open. Sign in to continue.</p>
        {/* ⚠ TWO DIFFERENT FAILURES, AND TELLING THEM APART SAVES AN AFTERNOON.
            `error=1` is a sign-in that started and did not finish — a stale
            code, a mismatched state, a refused exchange — and trying again is
            the right advice. `error=config` is this deployment being wrong, and
            trying again will never help; the server log names the URL it could
            not read. */}
        {error === "config" ? (
          <p className="empty err">
            This console is not configured to reach its API. Trying again will
            not help — check <code>RATIO_API_ORIGIN</code> and{" "}
            <code>RATIO_CONSOLE_ORIGIN</code>; the server log names the URL it
            tried.
          </p>
        ) : error ? (
          <p className="empty err">That sign-in did not complete. Try again.</p>
        ) : null}
        <a className="signin-btn" href={href}>
          Sign in
        </a>
      </div>
    </div>
  );
}
