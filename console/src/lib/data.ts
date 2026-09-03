import "server-only";

import { cache } from "react";
import { getBook, getView, listBooks, listFunds } from "@/wire/client";
import { caller } from "./caller";
import { orAuth } from "./orAuth";
import { orTransient, type OrTransient } from "./orTransient";
import type { Book, Fund } from "@/wire/types";

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
 *
 * ⚠ A 401 AFTER `caller()` IS STILL A MISSING SESSION. `caller()` only
 * redirects when AuthKit has no session. A session the gateway will not
 * accept still reaches the list call; `orTransient` runs `orAuth` so that
 * `AuthError` still becomes `/signin?returnTo=…`.
 *
 * ⛔ A 503 IS NOT A MISSING SESSION. Digest `2106392403` was `Refused: 503`
 * on GET /books while the API deploy was rolling. `orAuth` rethrew it;
 * Next redacted the page to `#441`. `orTransient` turns status ≥ 500
 * into a value the layout can render. It does not redirect — that would
 * be a second AuthError path, and it would send a signed-in operator
 * to `/signin`.
 */
export const funds = cache(async (): Promise<OrTransient<Fund[]>> => {
  const c = await caller();
  const r = await orTransient(listFunds(c));
  if (r.unavailable !== null) return r;
  return { unavailable: null, value: r.value.funds };
});

export const books = cache(async (): Promise<OrTransient<Book[]>> => {
  const c = await caller();
  const r = await orTransient(listBooks(c));
  if (r.unavailable !== null) return r;
  return { unavailable: null, value: r.value.books };
});

/**
 * The book a view layout needs in order to pick chrome.
 *
 * ⭐ KIND SELECTS WHETHER NAV TILES BELONG HERE. A personal book's view still
 * folds a journal prefix; it does not strike a NAV. A project book's view
 * does the same. Asking GetBook twice would be two Lambdas for one URL.
 * `cache` is the same door `viewOf` uses.
 */
export const bookOf = cache(async (id: string) => {
  const c = await caller();
  return orAuth(getBook(c, id));
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
  return orAuth(getView(c, fund, view));
});
