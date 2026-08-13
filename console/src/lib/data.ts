import "server-only";

import { cache } from "react";
import { listFunds } from "@/wire/client";
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
