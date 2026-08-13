import { type NextRequest } from "next/server";
import { claimsOf, exchange } from "@/lib/oidc";
import { clearPending, readPending, writeSession } from "@/lib/session";
import { consoleOrigin, safeReturnTo, sameOrigin } from "../redirect";

export const dynamic = "force-dynamic";

/**
 * Where the IdP sends the browser back, carrying `?code=`.
 *
 * ⚠ THE COOKIE THAT MAKES THIS WORK ARRIVES BECAUSE IT IS `SameSite=Lax`. This
 * is a top-level navigation from a different site; `Strict` would withhold the
 * pending cookie on exactly this request and the failure would read like a PKCE
 * bug rather than a cookie one.
 *
 * ⛔ EVERY EXIT FROM HERE IS RELATIVE EXCEPT THE ONE COGNITO READS. The
 * `redirect_uri` sent to the token endpoint must equal the one the code was
 * issued against, character for character, so that one is built from
 * `consoleOrigin()`. The rest are pages this same process serves, and building
 * *those* from a declared origin used to mean a wrong or missing
 * `RATIO_CONSOLE_ORIGIN` turned a refused sign-in — which has a page that
 * explains itself — into a 500 or a redirect to nowhere.
 */
export async function GET(req: NextRequest) {
  const params = req.nextUrl.searchParams;
  const code = params.get("code");
  const pending = await readPending();
  await clearPending();

  // The IdP refused, or somebody arrived here without starting a sign-in.
  if (!code || !pending) {
    return sameOrigin("/signin?error=1");
  }
  // ⛔ CSRF. A callback that does not carry the state we issued is not the flow
  // this browser started, and exchanging its code would sign this session in as
  // whoever produced it.
  if (params.get("state") !== pending.state) {
    return sameOrigin("/signin?error=1");
  }

  try {
    const t = await exchange(
      code,
      pending.verifier,
      `${await consoleOrigin()}/api/auth/callback`,
    );
    const { sub, email } = claimsOf(t.idToken);
    await writeSession({
      idToken: t.idToken,
      sub,
      email,
      expiresAt: t.expiresAt,
    });
  } catch (e) {
    console.error("sign-in did not complete:", e);
    return sameOrigin("/signin?error=1");
  }

  // Checked again on the way out, not only on the way in: the cookie is sealed,
  // but a redirect target is worth refusing twice — and with a relative
  // `Location` this guard is the only thing standing between a tampered
  // `returnTo` and another origin.
  return sameOrigin(safeReturnTo(pending.returnTo));
}
