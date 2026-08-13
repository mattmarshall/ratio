import "server-only";

import { notFound } from "next/navigation";
import { NotFound } from "@/wire/client";

/**
 * Render 404 for a resource that is not there.
 *
 * ⚠ AND FOR ONE THIS SUBJECT MAY NOT SEE, WHICH IS THE SAME THING ON PURPOSE.
 * `Console::book_path` refuses a fund a caller has no grant for with the SAME
 * error as one that does not exist, so that the console cannot be used to
 * enumerate other people's funds. Collapsing both to 404 here is what keeps
 * that true on this side; distinguishing them would undo it.
 */
export async function or404<T>(p: Promise<T>): Promise<T> {
  try {
    return await p;
  } catch (e) {
    if (e instanceof NotFound) notFound();
    throw e;
  }
}
