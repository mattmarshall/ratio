/**
 * A return target that cannot leave this origin, and cannot be the
 * sign-in prompt.
 *
 * ⛔ AN OPEN REDIRECT ON A SIGN-IN ROUTE IS A TOKEN-EXFILTRATION BUG, so this
 * refuses everything except a single-slash-rooted path. `//evil.example` and
 * `/\evil.example` are both protocol-relative URLs that a naive
 * `startsWith("/")` accepts, and both are why the second character is checked.
 *
 * ⛔ RETURNING TO `/signin` AFTER A SUCCESSFUL AUTHKIT CALLBACK IS THE BOUNCE.
 * `getSignInUrl({ returnTo })` / `handleAuth()` send the browser here once
 * WorkOS has finished. If this is the prompt page, the operator is staring at
 * "Sign in" with a session already set — they click it, AuthKit skips the
 * hosted UI, and they land here again.
 */
export function safeReturnTo(raw: string | null | undefined): string {
  if (!raw) return "/";
  if (!raw.startsWith("/")) return "/";
  if (raw.startsWith("//") || raw.startsWith("/\\")) return "/";
  const path = raw.split("?")[0] ?? raw;
  if (isAuthFlowPath(path)) return "/";
  return raw;
}

/** The prompt, the initiate-login aliases, and the callback. */
export function isAuthFlowPath(path: string): boolean {
  return (
    path === "/signin" ||
    path === "/sign-in" ||
    path === "/login" ||
    path === "/callback" ||
    path.startsWith("/api/auth/")
  );
}
