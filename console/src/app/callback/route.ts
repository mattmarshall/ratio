import { handleAuth } from "@workos-inc/authkit-nextjs";
import { unstable_rethrow } from "next/navigation";
import type { NextRequest } from "next/server";
import { sameOrigin } from "@/app/api/auth/redirect";
import { continueWithSealedSession } from "@/lib/authkitCallback";

export const dynamic = "force-dynamic";

/**
 * AuthKit's redirect URI. Must match `NEXT_PUBLIC_WORKOS_REDIRECT_URI`
 * and the Redirect URI registered on the WorkOS application.
 *
 * Docs: https://workos.com/docs/authkit/nextjs — `handleAuth()`.
 *
 * ⚠ `onError` SENDS THEM TO THE PROMPT, NOT A 500 JSON BODY. A missing
 * PKCE cookie after WorkOS finished used to look like "Couldn't sign in"
 * with no way back. `/signin?error=1` is the same sentence the prompt
 * already knows how to say. It does not invent a second callback.
 *
 * ⛔ `handleAuth` RETURNS `NextResponse.redirect` AFTER `cookies().set()`
 * OF `wos-session`. That is the remaining bounce after #253: the PKCE
 * cookie now lands, WorkOS finishes, and the session cookie does not
 * ride that 307. `continueWithSealedSession` throws `redirect()` from
 * `next/navigation` so it does.
 */
const handle = handleAuth({
  onError: async () => sameOrigin("/signin?error=1"),
});

export async function GET(request: NextRequest) {
  try {
    return await continueWithSealedSession(await handle(request));
  } catch (e) {
    unstable_rethrow(e);
    throw e;
  }
}
