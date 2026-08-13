import "server-only";

import { NextResponse } from "next/server";
import { authConfig } from "@/lib/oidc";

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
 * ⛔ DECLARED BY THE DEPLOYMENT, NEVER TAKEN FROM THE `Host` HEADER. That rule
 * has not moved and is the reason this function exists at all: the value is an
 * OAuth `redirect_uri`, so whoever chooses it chooses where an authorization
 * code is delivered. A forged `Host` must never get a vote.
 *
 * ⭐ WHAT CHANGED IS *WHICH* DECLARATION IS AUTHORITATIVE, AND IT IS NOT THIS
 * PROCESS'S ENVIRONMENT. `deploy/app.yaml` builds the Cognito app client's
 * callback URL and the API's `RATIO_CONSOLE_URL` out of a single `ConsoleOrigin`
 * parameter, and the API publishes it at `/authconfig.json`. Reading it there
 * gets the byte-identical string the IdP will compare against. A second copy in
 * `RATIO_CONSOLE_ORIGIN` was a value that could disagree, and on the first
 * deployment that had both, it did — Vercel held `https://ratio-console.vercel.app`
 * while Cognito had `https://ratio-ims.vercel.app` — which is the failure
 * `watch.rs` names for the issuer and the client id: *two places that must agree
 * is two places that will one day not.*
 *
 * ⛔ AND THIS IS NOT THE `Host` HEADER BY ANOTHER ROUTE. `/authconfig.json` is
 * the same TLS document already trusted for `issuer`, `clientId` and `domain`,
 * every one of which is worse to forge than a redirect target — a forged
 * `domain` sends the person to an attacker's hosted UI. Trusting one more field
 * of it adds no new trust. Deriving from the request would have added one.
 *
 * ⚠ Cognito accepts no wildcards, so `deploy/app.yaml` registers exactly two
 * callbacks: `${ConsoleOrigin}` and `http://localhost:3000`. A preview
 * deployment on its own generated hostname cannot sign in, by design — previews
 * run against `console/fixtures/`.
 */
export async function consoleOrigin(): Promise<string> {
  const declared = process.env.RATIO_CONSOLE_ORIGIN?.replace(/\/+$/, "");

  let published: string | undefined;
  try {
    published = (await authConfig()).consoleOrigin?.replace(/\/+$/, "");
  } catch {
    // An unreachable API is reported by whoever needed the config, not here.
    // ⚠ `next dev` against a laptop with no server still has to get to the
    // fallback below, and `authConfig()` throwing is the ordinary way that
    // looks. Swallowing it here loses nothing: every caller of this function
    // also calls `authConfig()` for the IdP domain moments later, and that call
    // surfaces the same failure with the URL in it.
  }

  // ⚠ Empty is the local answer, not a missing field: a `ratio watch` on
  // loopback publishes "" because it has no console to point at, and that is
  // exactly when `RATIO_CONSOLE_ORIGIN` should win.
  if (!published) {
    if (!declared) {
      throw new Error(
        "no console origin: the API published none and RATIO_CONSOLE_ORIGIN is not set",
      );
    }
    return declared;
  }

  // ⛔ THE PUBLISHED VALUE WINS A DISAGREEMENT, AND THE LOG SAYS SO. Cognito
  // will accept exactly one of these two and refuse the other, and the one it
  // accepts is the one it was configured from. Preferring the local copy would
  // mean an operator's stale environment variable can only ever break the
  // sign-in; preferring the published one means it cannot. It is still worth a
  // line in the log, because a variable that no longer does anything is a trap
  // for the next person who edits it expecting it to.
  if (declared && declared !== published) {
    console.warn(
      `RATIO_CONSOLE_ORIGIN is ${declared} but the API publishes ${published}; ` +
        `using the published value, which is the one Cognito registered. ` +
        `Unset RATIO_CONSOLE_ORIGIN or correct it to match.`,
    );
  }
  return published;
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
