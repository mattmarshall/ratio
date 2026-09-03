import "server-only";

import { cache } from "react";
import { getBook, getView, listBooks, listFunds } from "@/wire/client";
import { caller } from "./caller";

/**
 * The reads a layout and its page both need, memoized for one request.
 *
 * ⛔ WITHOUT THIS, NESTING COSTS A ROUND TRIP. `/funds` renders inside the
 * layout that draws the rail, and both want the fund list — so the naive
 * version invokes the Lambda twice for one screen. Next memoizes identical
 * `fetch` calls within a render pass, but that is a property of the framework's
 * defaults rather than of this code, and the defaults have moved between
 * majors. React's `cache` says it here, where a reader can see it.
 *
 * ⚠ Memoized PER REQUEST, not across them. Nothing here is a cache in the sense
 * that would let a stale NAV survive a reload.
 */
export const funds = cache(async () => {
  const c = await caller();
  return (await listFunds(c)).funds;
});

export const books = cache(async () => {
  const c = await caller();
  return (await listBooks(c)).books;
});

export const bookOf = cache(async (book: string) => {
  const c = await caller();
  return getBook(c, book);
});

/**
 * The book of record a view layout and its page both render.
 *
 * ⭐ #53. The four Stat tiles live on the layout; the page is the citation of
 * the same figures plus the terms that distinguish this view from its sibling.
 * Asking twice would be two Lambdas for one URL. `cache` is the same door
 * `funds` and `books` already use.
 */
export const viewOf = cache(async (fund: string, view: string) => {
  const c = await caller();
  return getView(c, fund, view);
});
