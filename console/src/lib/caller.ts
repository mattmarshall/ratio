import "server-only";

import { withAuth } from "@workos-inc/authkit-nextjs";
import { headers } from "next/headers";
import { redirect } from "next/navigation";
import type { Caller } from "@/wire/client";
import { workosConfigured } from "./workos";

/**
 * Who this request speaks to the API as.
 *
 * Two shapes, and the difference is the deployment rather than the person:
 *
 *   * **Deployed.** WorkOS AuthKit is configured, so a session is required
 *     and its access token is the bearer. No session means a redirect to
 *     sign-in, which is what the server would refuse anyway — `RATIO_AUTH=
 *     required` makes `/v1` fail closed.
 *   * **Local.** `ratio watch` on loopback sets no `WORKOS_*`, so there is
 *     no sign-in to do, and the server answers as `Subject::Local` —
 *     unrestricted, and not a tenant. `idToken` is null and no
 *     `Authorization` header is sent.
 *
 * ⚠ THIS IS NOT AN AUTHORIZATION DECISION AND MUST NOT BE READ AS ONE. Which
 * books a subject may open is decided in Rust at `Console::open_book`, against
 * `MEMBERSHIP.tsv`, where the test suite can break it. All this does is decide
 * which token to send. A book a caller may not see is refused with the same
 * error as one that does not exist, and that refusal comes from the server.
 */
export async function caller(): Promise<Caller> {
  if (!workosConfigured()) return { idToken: null };
  const session = await withAuth();
  if (!session.user || !session.accessToken) redirect(await signInHref());
  return { idToken: session.accessToken };
}

/**
 * Where to send somebody who is not signed in — carrying where they were going.
 *
 * ⭐ THE RETURN PATH IS THE WHOLE POINT OF A CITABLE URL. Someone was sent a
 * link to ONE break. Dropping them on the default queue after they sign in and
 * making them find it again is the old console's behaviour with extra steps.
 *
 * ⚠ The path comes from a header `src/proxy.ts` sets, because a server
 * component cannot see its own URL. It is a path and never an absolute URL, and
 * `/sign-in` re-checks that before redirecting to it — an unchecked
 * return target on a route that carries tokens is an open redirect.
 *
 * `orAuth` reuses this rather than building a second one — a second
 * construction is how an open redirect gets invented.
 */
export async function signInHref(): Promise<string> {
  const here = (await headers()).get("x-pathname");
  return here ? `/signin?returnTo=${encodeURIComponent(here)}` : "/signin";
}

/** Who to show in the header chip. */
export interface Principal {
  sub: string;
  email: string;
  /** WorkOS `user.profilePictureUrl`. Empty when the IdP sent no photo. */
  profilePictureUrl: string | null;
  firstName: string | null;
  lastName: string | null;
}

export async function principal(): Promise<Principal | null> {
  if (!workosConfigured()) return null;
  const { user } = await withAuth();
  if (!user) return null;
  return {
    sub: user.id,
    email: user.email,
    profilePictureUrl: user.profilePictureUrl || null,
    firstName: user.firstName || null,
    lastName: user.lastName || null,
  };
}
