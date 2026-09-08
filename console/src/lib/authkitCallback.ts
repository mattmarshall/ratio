import "server-only";

import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { safeReturnTo } from "./returnTo";

/**
 * Finish AuthKit's callback with a redirect the session cookie can ride.
 *
 * ⛔ `handleAuth()` SEALS `wos-session` WITH `cookies().set()` THEN RETURNS
 * `NextResponse.redirect`. Those are different Response objects — the same
 * defect `startAuthKit` already named for the PKCE verifier. AuthKit
 * finishes, the operator lands on `/books` with no session cookie, and
 * `caller()` sends them back to `/signin`. #253 stopped a 401-*with*-session
 * from looping; this is the bounce when the session never lands.
 *
 * `redirect()` from `next/navigation` is the cookie store's response, so
 * the sealed session is on the 307 the browser actually stores.
 */
export async function continueWithSealedSession(
  response: Response,
): Promise<Response> {
  const location = response.headers.get("location");
  if (!location || response.status < 300 || response.status >= 400) {
    return response;
  }

  const path = pathFromCallbackLocation(location);
  if (path === null) return response;

  const jar = await cookies();
  for (const cookie of jar.getAll()) {
    if (
      cookie.name === "wos-auth-verifier" ||
      cookie.name.startsWith("wos-auth-verifier-")
    ) {
      jar.delete(cookie.name);
    }
  }

  redirect(path);
}

/**
 * A callback Location that cannot leave this origin.
 *
 * `handleAuth` builds an absolute URL from the request. A relative path
 * is what `redirect()` from `next/navigation` accepts without inventing
 * a second host. Failed exchange keeps `/signin?error=…` — `safeReturnTo`
 * would strip the prompt and drop the sentence.
 */
export function pathFromCallbackLocation(location: string): string | null {
  let path: string;
  if (location.startsWith("/") && !location.startsWith("//")) {
    path = location;
  } else {
    try {
      const url = new URL(location);
      path = `${url.pathname}${url.search}`;
    } catch {
      return null;
    }
  }

  if (path.startsWith("/signin?") && path.includes("error=")) return path;
  return safeReturnTo(path);
}
