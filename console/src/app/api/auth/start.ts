import { getSignInUrl } from "@workos-inc/authkit-nextjs";
import { NextResponse, type NextRequest } from "next/server";
import { workosConfigured } from "@/lib/workos";
import { safeReturnTo, sameOrigin } from "./redirect";

/**
 * Start AuthKit. Served at `/login` (the path the Next.js docs name) and
 * at `/api/auth/login` (the existing button / bookmark).
 */
export async function startAuthKit(req: NextRequest) {
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
