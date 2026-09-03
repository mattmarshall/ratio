import "server-only";

import { redirect } from "next/navigation";
import { AuthError } from "@/wire/client";
import { signInHref } from "./caller";

/**
 * Redirect to sign-in when the API refused the bearer.
 *
 * ⛔ 401 IS A REAL ANSWER, NOT A TRANSPORT FAILURE. `caller()` redirects when
 * AuthKit has no session. A session whose access token the gateway will not
 * accept is the other half of the same fact — `send()` throws `AuthError` —
 * and throwing that through a server component is how `/books` reached
 * production as `Minified React error #441` (digest `2667936230`). This is
 * that catch: the same `/signin?returnTo=…` `caller()` already uses, not a
 * second open redirect.
 *
 * ⚠ CATCH IT HERE, NOT INSIDE `caller()`. `caller()` decides which token to
 * send. The 401 is the API's answer after that send. Folding the two together
 * would make every write-path `AuthError` a navigation, and the tickets
 * already return those as values.
 */
export async function orAuth<T>(p: Promise<T>): Promise<T> {
  try {
    return await p;
  } catch (e) {
    if (e instanceof AuthError) redirect(await signInHref());
    throw e;
  }
}
