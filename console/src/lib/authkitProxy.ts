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

/**
 * Routes that set AuthKit cookies themselves and must not run `authkit()`.
 *
 * ⛔ `/callback` IS A DOCUMENT REQUEST WITH NO SESSION YET. `authkit()` mints
 * a new PKCE verifier on every unauthenticated HTML navigation. The
 * verifier the operator actually holds is the one `/sign-in` sealed.
 * Running `authkit()` here is how a completed WorkOS login looks up the
 * wrong cookie — AuthKit's own middlewareAuth path auto-adds the redirect
 * URI to unauthenticatedPaths for that loop. `/sign-in` (and its login
 * aliases) call `getSignInUrl`; they do not call `withAuth()`. Logout
 * still needs the session. The `/signin` *page* still needs the
 * middleware marker so `continueIfSignedIn` can run.
 */
export function skipAuthkitInProxy(pathname: string): boolean {
  return (
    pathname === "/callback" ||
    pathname === "/sign-in" ||
    pathname === "/login" ||
    pathname === "/api/auth/login" ||
    pathname === "/api/auth/callback"
  );
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
