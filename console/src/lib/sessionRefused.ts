/**
 * Copy when AuthKit has a session and the API still answers 401.
 *
 * ⛔ THIS IS NOT A MISSING SESSION. `caller()` already required `user` and
 * an access token. Redirecting to `/signin` is how a successful WorkOS
 * login becomes a loop: the gateway refuses the bearer (audience / issuer),
 * the console asks them to sign in again, AuthKit reuses the same session,
 * and `/books` 401s again.
 */
export const SESSION_REFUSED = "the API did not accept this session";
