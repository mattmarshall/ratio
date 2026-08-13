import { NextResponse } from "next/server";
import { clearSession } from "@/lib/session";
import { consoleOrigin } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * Sign out.
 *
 * POST, not GET: signing someone out from an `<img src>` is a small thing to be
 * able to do to them, and there is no reason to allow it.
 *
 * ⚠ This drops the local session only; it does not end the Cognito Hosted UI
 * session, so "sign in" afterwards may not re-prompt. Ending the IdP session
 * means a redirect to the pool's `/logout`, which needs the console origin
 * registered as a `LogoutURL` — it is, in `deploy/app.yaml`, so this is a small
 * follow-on rather than a missing piece.
 */
export async function POST() {
  await clearSession();
  return NextResponse.redirect(`${consoleOrigin()}/signin`, { status: 303 });
}
