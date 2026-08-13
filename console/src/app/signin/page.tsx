import { Brand } from "@/components/Brand";

export const dynamic = "force-dynamic";

/** The console with no session: a prompt, not a wall of refusals.
 *
 * The button is a plain link to `/api/auth/login`, which stashes a PKCE
 * verifier and hands the tab to the Cognito Hosted UI. Nothing about the flow
 * runs in this page — see `lib/oidc.ts` for why that is the point. */
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
        <p>This console shows only the funds you administer. Sign in to continue.</p>
        {error ? (
          <p className="empty err">That sign-in did not complete. Try again.</p>
        ) : null}
        <a className="signin-btn" href={href}>
          Sign in
        </a>
      </div>
    </div>
  );
}
