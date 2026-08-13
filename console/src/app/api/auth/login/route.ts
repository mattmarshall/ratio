import { randomBytes } from "node:crypto";
import { NextResponse, type NextRequest } from "next/server";
import { authorizeUrl, challengeFor, newVerifier } from "@/lib/oidc";
import { writePending } from "@/lib/session";
import { consoleOrigin, safeReturnTo } from "../redirect";

export const dynamic = "force-dynamic";

/** Start the sign-in: stash the verifier, then hand the tab to the IdP. */
export async function GET(req: NextRequest) {
  const verifier = newVerifier();
  const state = randomBytes(16).toString("base64url");
  const returnTo = safeReturnTo(req.nextUrl.searchParams.get("returnTo"));

  await writePending({ verifier, state, returnTo });

  const url = await authorizeUrl(
    `${consoleOrigin()}/api/auth/callback`,
    challengeFor(verifier),
    state,
  );
  return NextResponse.redirect(url);
}
