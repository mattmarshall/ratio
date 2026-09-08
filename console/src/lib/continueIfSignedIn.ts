import "server-only";

import { withAuth } from "@workos-inc/authkit-nextjs";
import { redirect } from "next/navigation";
import { safeReturnTo } from "./returnTo";
import { workosConfigured } from "./workos";

/**
 * Leave the sign-in prompt when AuthKit already has a session.
 *
 * ⭐ THE CALLBACK CAN LAND HERE. `handleAuth()` honours `returnTo`. If that
 * path is `/signin` (or the operator bookmarks the prompt), a successful
 * sign-in still renders "Sign in". Sending them onward is the whole
 * difference between a completed login and a bounce.
 */
export async function continueIfSignedIn(
  returnTo: string | undefined,
): Promise<void> {
  if (!workosConfigured()) return;
  const { user } = await withAuth();
  if (user) redirect(safeReturnTo(returnTo));
}
