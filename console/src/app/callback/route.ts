import { handleAuth } from "@workos-inc/authkit-nextjs";
import { sameOrigin } from "@/app/api/auth/redirect";

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
 */
export const GET = handleAuth({
  onError: async () => sameOrigin("/signin?error=1"),
});
