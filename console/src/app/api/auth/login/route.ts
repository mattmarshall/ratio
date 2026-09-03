import { getSignInUrl } from "@workos-inc/authkit-nextjs";
import { NextResponse, type NextRequest } from "next/server";
import { workosConfigured } from "@/lib/workos";
import { safeReturnTo, sameOrigin } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * Initiate login URL — AuthKit's documented start of the hosted sign-in.
 *
 * https://workos.com/docs/authkit/nextjs
 *
 * ⛔ A MISCONFIGURED DEPLOYMENT ANSWERS WITH A SENTENCE, NOT A 500.
 */
export async function GET(req: NextRequest) {
  const returnTo = safeReturnTo(req.nextUrl.searchParams.get("returnTo"));
  if (!workosConfigured()) {
    console.error("sign-in is not configured: WORKOS_* is unset");
    return sameOrigin("/signin?error=config");
  }
  try {
    const url = await getSignInUrl({ returnTo });
    return NextResponse.redirect(url);
  } catch (e) {
    console.error("sign-in is not configured:", e);
    return sameOrigin("/signin?error=config");
  }
}
