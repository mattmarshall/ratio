import { partitionAuthkitHeaders } from "@workos-inc/authkit-nextjs";
import type { NextRequest } from "next/server";

/**
 * Merge AuthKit's trusted request headers with this proxy's own.
 *
 * ⛔ REQUEST HEADERS, NOT RESPONSE ONES. `withAuth()` reads `x-workos-middleware`
 * and `x-workos-session` from the incoming request. Putting them on the
 * response instead reached production as `Minified React error #441` on `/books`
 * after sign-in — the session cookie was set, but every server component that
 * called `withAuth()` threw because the middleware marker never arrived.
 *
 * ⛔ PKCE COOKIES DO NOT BELONG ON `NextResponse.next()`. `authkit()` mints a
 * verifier on every unauthenticated document request. AuthKit's
 * `handleAuthkitHeaders` strips those unless it is redirecting to AuthKit.
 * This proxy never redirects to AuthKit (`/sign-in` does, via `cookies().set()`).
 * Forwarding a competing `wos-auth-verifier-*` on every page is how the
 * callback looks up the wrong cookie after WorkOS finishes.
 */
export function mergeAuthkitProxyHeaders(
  req: NextRequest,
  authkitHeaders: Headers,
  extraRequestHeaders: Record<string, string>,
): { requestHeaders: Headers; responseHeaders: Headers } {
  const { requestHeaders, responseHeaders } = partitionAuthkitHeaders(
    req,
    authkitHeaders,
  );
  for (const [key, value] of Object.entries(extraRequestHeaders)) {
    requestHeaders.set(key, value);
  }
  return {
    requestHeaders,
    responseHeaders: dropPkceSetCookie(responseHeaders),
  };
}

function dropPkceSetCookie(headers: Headers): Headers {
  const cookies =
    typeof headers.getSetCookie === "function" ? headers.getSetCookie() : [];
  const kept = new Headers();
  for (const [name, value] of headers) {
    if (name.toLowerCase() !== "set-cookie") kept.append(name, value);
  }
  for (const cookie of cookies) {
    const cookieName = cookie.split("=", 1)[0] ?? "";
    if (
      cookieName === "wos-auth-verifier" ||
      cookieName.startsWith("wos-auth-verifier-")
    ) {
      continue;
    }
    kept.append("set-cookie", cookie);
  }
  return kept;
}
