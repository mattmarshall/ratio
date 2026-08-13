import "server-only";

import { headers } from "next/headers";
import { redirect } from "next/navigation";
import type { Caller } from "@/wire/client";
import { authConfigured } from "./oidc";
import { readSession, type Principal, principalOf } from "./session";

/**
 * Who this request speaks to the API as.
 *
 * Two shapes, and the difference is the deployment rather than the person:
 *
 *   * **Deployed.** An identity provider is configured, so a session is
 *     required and its id token is the bearer. No session means a redirect to
 *     sign-in, which is what the server would refuse anyway — `RATIO_AUTH=
 *     required` makes `/v1` fail closed.
 *   * **Local.** `ratio watch` on loopback sets no `RATIO_COGNITO_*`, so
 *     `/authconfig.json` answers with empty strings, there is no sign-in to do,
 *     and the server answers as `Subject::Local` — unrestricted, and not a
 *     tenant. `idToken` is null and no `Authorization` header is sent.
 *
 * ⚠ THIS IS NOT AN AUTHORIZATION DECISION AND MUST NOT BE READ AS ONE. Which
 * funds a subject may open is decided in Rust at `Console::book_path`, against
 * `MEMBERSHIP.tsv`, where the test suite can break it. All this does is decide
 * which token to send. A fund a caller may not see is refused with the same
 * error as one that does not exist, and that refusal comes from the server.
 */
export async function caller(): Promise<Caller> {
  if (!(await authConfigured())) return { idToken: null };
  const s = await readSession();
  if (!s) redirect(await signInHref());
  return { idToken: s.idToken };
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
 * `/api/auth/login` re-checks that before redirecting to it — an unchecked
 * return target on a route that carries tokens is an open redirect.
 */
async function signInHref(): Promise<string> {
  const here = (await headers()).get("x-pathname");
  return here ? `/signin?returnTo=${encodeURIComponent(here)}` : "/signin";
}

/** The person in the header chip, or null on a local run. */
export async function principal(): Promise<Principal | null> {
  return principalOf(await readSession());
}
