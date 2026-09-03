"use server";

import { redirect } from "next/navigation";
import { caller } from "@/lib/caller";
import { AuthError, createBook, Refused } from "@/wire/client";
import type { BookKind } from "@/wire/types";

export type Result = { ok: false; error: string } | null;

/**
 * Create an independent book.
 *
 * ⛔ ERRORS COME BACK AS VALUES, NOT AS THROWS. Next replaces an uncaught
 * Server Action error with an opaque digest; the sentence the server wrote is
 * what this form must show.
 */
export async function submit(_prev: Result, form: FormData): Promise<Result> {
  const bookId = String(form.get("bookId") ?? "").trim();
  const displayName = String(form.get("displayName") ?? "").trim();
  const kind = String(form.get("kind") ?? "") as BookKind;

  if (!bookId) return { ok: false, error: "Choose an id — letters, digits, hyphen or underscore." };
  if (!/^[A-Za-z0-9_-]+$/.test(bookId)) {
    return { ok: false, error: "An id is letters, digits, hyphen or underscore." };
  }
  if (kind !== "PERSONAL" && kind !== "INVESTMENT" && kind !== "PROJECT") {
    return { ok: false, error: "Choose what this book is for." };
  }

  try {
    const c = await caller();
    const book = await createBook(c, bookId, {
      displayName: displayName || bookId,
      kind,
    });
    const id = book.name.replace(/^books\//, "");
    redirect(`/books/${id}`);
  } catch (e) {
    if (e instanceof AuthError) {
      return { ok: false, error: "Sign in to create a book." };
    }
    if (e instanceof Refused) {
      return { ok: false, error: e.message };
    }
    throw e;
  }
}
