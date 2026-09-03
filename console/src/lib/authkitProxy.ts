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
  return { requestHeaders, responseHeaders };
}
