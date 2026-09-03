import "server-only";

import { Refused } from "@/wire/client";
import { orAuth } from "./orAuth";

/**
 * A 5xx from an authenticated read, surfaced as a VALUE.
 *
 * ⛔ THIS IS THE OTHER #441 PATH. `/books` called `listBooks` after
 * `caller()` had a session; the API was rolling and answered 503;
 * `send()` threw `Refused`; `orAuth` only catches `AuthError`; Next
 * redacted the throw to digest `2106392403`. A 503 is not a missing
 * session and not a figure — folding it into `orAuth` would send the
 * operator to `/signin` while they are already signed in, and would
 * turn every write-path `Refused` that reused this helper into a
 * navigation. This sibling converts status ≥ 500 into a value the
 * page can render, and leaves 401 to `orAuth`.
 *
 * ⚠ 4xx `Refused` AND `NotFound` STILL THROW. A 400 is a sentence
 * about a figure (`orRefused`); a 404 is a missing resource (`or404`).
 * A transport failure is neither.
 */
export type OrTransient<T> =
  | { unavailable: null; value: T }
  | { unavailable: string };

export async function orTransient<T>(p: Promise<T>): Promise<OrTransient<T>> {
  try {
    return { unavailable: null, value: await orAuth(p) };
  } catch (e) {
    if (e instanceof Refused && e.status >= 500) {
      return { unavailable: e.message };
    }
    throw e;
  }
}
