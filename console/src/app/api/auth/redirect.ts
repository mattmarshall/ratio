import "server-only";

import { NextResponse } from "next/server";

/**
 * A redirect to a page on this same server, with no host in it at all.
 *
 * ⛔ THE ONE PLACE THE ABSOLUTE ORIGIN IS LOAD-BEARING IS THE `redirect_uri`,
 * AND EVERY OTHER USE OF IT WAS A LIABILITY. `consoleOrigin()` is a declared
 * value that can be wrong or missing, and both of those are failures the
 * sign-in routes have to *report* — so building the error page's URL out of it
 * meant the report was delivered to a hostname that might not resolve, or threw
 * and became a 500. A live deployment held `https://ratio-console.vercel.app`,
 * which does not exist, and the page saying so was never reachable.
 *
 * A relative `Location` (RFC 7231 §7.1.2) is resolved by the browser against
 * the URL it actually requested. No environment variable, no `Host` header, no
 * forwarded-header question behind a proxy — for a page served by this same
 * process there is nothing to get wrong.
 *
 * ⚠ Callers pass a rooted path. `safeReturnTo` is what makes an untrusted one
 * safe to pass, and a relative Location makes its guard load-bearing rather
 * than belt-and-braces: `//evil.example` resolves to another origin.
 */
export function sameOrigin(path: string, status: 303 | 307 = 307): NextResponse {
  return new NextResponse(null, { status, headers: { location: path } });
}

/**
 * The origin this console is served from.
 *
 * ⛔ AN ENVIRONMENT VARIABLE, NEVER THE `Host` HEADER. The value is used as an
 * OAuth `redirect_uri`, and Cognito matches it against a registered callback
 * exactly. Deriving it from the request would let a forged `Host` choose where
 * the code is delivered; declaring it means a mismatch is a refused sign-in,
 * which is a loud failure rather than a quiet one.
 *
 * ⚠ Cognito accepts no wildcards, so exactly three values are registered in
 * `deploy/app.yaml`: production, one stable preview alias, and
 * `http://localhost:3000`. A preview deployment on its own generated hostname
 * cannot sign in, by design — previews run against `console/fixtures/`.
 */
export function consoleOrigin(): string {
  const o = process.env.RATIO_CONSOLE_ORIGIN;
  if (!o) throw new Error("RATIO_CONSOLE_ORIGIN is not set");
  return o.replace(/\/+$/, "");
}

/**
 * A return target that cannot leave this origin.
 *
 * ⛔ AN OPEN REDIRECT ON A SIGN-IN ROUTE IS A TOKEN-EXFILTRATION BUG, so this
 * refuses everything except a single-slash-rooted path. `//evil.example` and
 * `/\evil.example` are both protocol-relative URLs that a naive
 * `startsWith("/")` accepts, and both are why the second character is checked.
 */
export function safeReturnTo(raw: string | null | undefined): string {
  if (!raw) return "/";
  if (!raw.startsWith("/")) return "/";
  if (raw.startsWith("//") || raw.startsWith("/\\")) return "/";
  return raw;
}
