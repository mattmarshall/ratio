import { NextResponse, type NextRequest } from "next/server";
import { claimsOf, exchange } from "@/lib/oidc";
import { clearPending, readPending, writeSession } from "@/lib/session";
import { consoleOrigin, safeReturnTo } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * Where the IdP sends the browser back, carrying `?code=`.
 *
 * ⚠ THE COOKIE THAT MAKES THIS WORK ARRIVES BECAUSE IT IS `SameSite=Lax`. This
 * is a top-level navigation from a different site; `Strict` would withhold the
 * pending cookie on exactly this request and the failure would read like a PKCE
 * bug rather than a cookie one.
 */
export async function GET(req: NextRequest) {
  const params = req.nextUrl.searchParams;
  const code = params.get("code");
  const pending = await readPending();
  await clearPending();

  // The IdP refused, or somebody arrived here without starting a sign-in.
  if (!code || !pending) {
    return NextResponse.redirect(`${consoleOrigin()}/signin?error=1`);
  }
  // ⛔ CSRF. A callback that does not carry the state we issued is not the flow
  // this browser started, and exchanging its code would sign this session in as
  // whoever produced it.
  if (params.get("state") !== pending.state) {
    return NextResponse.redirect(`${consoleOrigin()}/signin?error=1`);
  }

  try {
    const t = await exchange(
      code,
      pending.verifier,
      `${consoleOrigin()}/api/auth/callback`,
    );
    const { sub, email } = claimsOf(t.idToken);
    await writeSession({
      idToken: t.idToken,
      sub,
      email,
      expiresAt: t.expiresAt,
    });
  } catch {
    return NextResponse.redirect(`${consoleOrigin()}/signin?error=1`);
  }

  // Checked again on the way out, not only on the way in: the cookie is sealed,
  // but a redirect target is worth refusing twice.
  return NextResponse.redirect(
    `${consoleOrigin()}${safeReturnTo(pending.returnTo)}`,
  );
}
