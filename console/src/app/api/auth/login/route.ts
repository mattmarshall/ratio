import { randomBytes } from "node:crypto";
import { NextResponse, type NextRequest } from "next/server";
import { authorizeUrl, challengeFor, newVerifier } from "@/lib/oidc";
import { writePending } from "@/lib/session";
import { consoleOrigin, safeReturnTo } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * Start the sign-in: stash the verifier, then hand the tab to the IdP.
 *
 * ⛔ A MISCONFIGURED DEPLOYMENT ANSWERS WITH A SENTENCE, NOT A 500. Everything
 * this route needs comes from the environment — `RATIO_API_ORIGIN`,
 * `RATIO_CONSOLE_ORIGIN`, `RATIO_SESSION_KEYS` — and any of them being wrong
 * used to throw, which Next renders as an opaque 500 with the reason only in a
 * server log. That is the same failure `record/actions.ts` avoids by returning
 * errors as values: the whole claim of this product is that a thing which does
 * not work can be accounted for.
 *
 * The reason still goes to the log, where the URL it tried is named. What
 * changes is that the person clicking gets told to check the configuration
 * rather than shown a blank error page.
 */
export async function GET(req: NextRequest) {
  const returnTo = safeReturnTo(req.nextUrl.searchParams.get("returnTo"));

  let origin: string;
  try {
    origin = consoleOrigin();
  } catch (e) {
    console.error("sign-in is not configured:", e);
    // No console origin means no safe absolute URL to build, so this one is
    // relative — the only branch here that cannot name its own host.
    return NextResponse.redirect(new URL("/signin?error=config", req.url));
  }

  try {
    const verifier = newVerifier();
    const state = randomBytes(16).toString("base64url");
    await writePending({ verifier, state, returnTo });
    const url = await authorizeUrl(
      `${origin}/api/auth/callback`,
      challengeFor(verifier),
      state,
    );
    return NextResponse.redirect(url);
  } catch (e) {
    console.error("sign-in is not configured:", e);
    return NextResponse.redirect(`${origin}/signin?error=config`);
  }
}
