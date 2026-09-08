import "server-only";

import { notFound } from "next/navigation";
import { NotFound } from "@/wire/client";
import { orAuth } from "./orAuth";

/**
 * Render 404 for a resource that is not there.
 *
 * ⚠ AND FOR ONE THIS SUBJECT MAY NOT SEE, WHICH IS THE SAME THING ON PURPOSE.
 * `Console::book_path` refuses a fund a caller has no grant for with the SAME
 * error as one that does not exist, so that the console cannot be used to
 * enumerate other people's funds. Collapsing both to 404 here is what keeps
 * that true on this side; distinguishing them would undo it.
 *
 * ⚠ A 401 AFTER `orAuth` REWROTE A HELD SESSION IS NOT A 404. `orAuth`
 * throws `Refused(401)` so the operator stays put. `orTransient` /
 * `withRefusal` render that as a status. Rethrowing it out of a page
 * that wraps neither is `#441`. This helper still rethrows — 404 is
 * its job — and the pages that call it must already have that wrap.
 */
export async function or404<T>(p: Promise<T>): Promise<T> {
  try {
    return await orAuth(p);
  } catch (e) {
    if (e instanceof NotFound) notFound();
    throw e;
  }
}
