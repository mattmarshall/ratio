import "server-only";

import { withAuth } from "@workos-inc/authkit-nextjs";
import { redirect } from "next/navigation";
import { AuthError, Refused } from "@/wire/client";
import { signInHref } from "./caller";
import { SESSION_REFUSED } from "./sessionRefused";
import { workosConfigured } from "./workos";

/**
 * Redirect to sign-in when there is no AuthKit session.
 *
 * ⛔ 401 IS A REAL ANSWER, NOT A TRANSPORT FAILURE. `caller()` redirects when
 * AuthKit has no session. Throwing `AuthError` through a server component is
 * how `/books` reached production as `Minified React error #441` (digest
 * `2667936230`). A missing session still becomes `/signin?returnTo=…`.
 *
 * ⛔ A 401 AFTER `caller()` HAD A TOKEN IS NOT A MISSING SESSION. Redirecting
 * to `/signin` then is the login bounce: WorkOS finished, the session cookie
 * is set, the gateway refuses the bearer, and the operator is asked to sign
 * in again. They will, and it will 401 again. `orTransient` surfaces that
 * as a status. It does not send them around the IdP.
 *
 * ⚠ CATCH IT HERE, NOT INSIDE `caller()`. `caller()` decides which token to
 * send. The 401 is the API's answer after that send. Folding the two together
 * would make every write-path `AuthError` a navigation, and the tickets
 * already return those as values.
 */
export async function orAuth<T>(p: Promise<T>): Promise<T> {
  try {
    return await p;
  } catch (e) {
    if (e instanceof AuthError) {
      if (await hasAuthKitSession()) {
        throw new Refused(401, SESSION_REFUSED);
      }
      redirect(await signInHref());
    }
    throw e;
  }
}

async function hasAuthKitSession(): Promise<boolean> {
  if (!workosConfigured()) return false;
  const session = await withAuth();
  return Boolean(session.user && session.accessToken);
}
