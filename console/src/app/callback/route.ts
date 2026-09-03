import { handleAuth } from "@workos-inc/authkit-nextjs";

export const dynamic = "force-dynamic";

/**
 * AuthKit's redirect URI. Must match `NEXT_PUBLIC_WORKOS_REDIRECT_URI`
 * and the Redirect URI registered on the WorkOS application.
 *
 * Docs: https://workos.com/docs/authkit/nextjs — `handleAuth()`.
 */
export const GET = handleAuth();
