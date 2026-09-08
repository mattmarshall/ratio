import { getSignInUrl } from "@workos-inc/authkit-nextjs";
import { redirect, unstable_rethrow } from "next/navigation";
import type { NextRequest } from "next/server";
import { workosConfigured } from "@/lib/workos";
import { safeReturnTo, sameOrigin } from "./redirect";

/**
 * Start AuthKit. Served at `/login` (the path the Next.js docs name) and
 * at `/api/auth/login` (the existing button / bookmark).
 *
 * ⛔ `redirect()` FROM `next/navigation`, NEVER `NextResponse.redirect`.
 * `getSignInUrl` seals the PKCE verifier with `cookies().set()`. A freshly
 * constructed `NextResponse.redirect` is a different Response object than
 * the cookie store, so the verifier never reaches the browser. WorkOS
 * then finishes, `/callback` finds no matching cookie, and the operator
 * is back on `/signin`. AuthKit's own `/sign-in` example uses `redirect()`.
 */
export async function startAuthKit(req: NextRequest) {
  const returnTo = safeReturnTo(req.nextUrl.searchParams.get("returnTo"));
  if (!workosConfigured()) {
    console.error("sign-in is not configured: WORKOS_* is unset");
    return sameOrigin("/signin?error=config");
  }
  try {
    const url = await getSignInUrl({ returnTo });
    redirect(url);
  } catch (e) {
    unstable_rethrow(e);
    console.error("sign-in is not configured:", e);
    return sameOrigin("/signin?error=config");
  }
}
