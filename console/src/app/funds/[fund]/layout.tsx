import type { ReactNode } from "react";
import { FundActions } from "@/components/FundActions";
import { PlaceHead } from "@/components/PlaceHead";
import { caller } from "@/lib/caller";
import { count } from "@/lib/format";
import { getFund, listViews } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * Identity of the open book. Each job under it is its own page — this
 * layout names the book and, on a figure page, which book of record
 * produced the numbers. It does not list the jobs.
 *
 * ⭐ ONE `GetFund` AND ONE `ListViews` FOR EVERY SCREEN UNDER THIS FUND. A
 * layout renders once per fund and persists across navigation within it, so no
 * child repeats either. Two 15-second timeouts stack behind each call and
 * `watch.rs` closes the connection after every response, so what decides
 * whether a page renders is the NUMBER of calls —
 * `//console:route_manifest_test` caps it at three per file.
 *
 * ⛔ THE FIGURES MOVED DOWN A LEVEL, AND DELIBERATELY. A NAV, a realized gain,
 * a lot count and an open difference all depend on WHICH ENTRIES ARE
 * RECOGNISED, so they belong to a view and are rendered by
 * `views/[view]/layout.tsx`. Showing them here would mean showing them on the
 * configuration and change-log screens too, where there is no view selected to
 * qualify them — which is a figure that does not say which question it answers.
 */
export default async function FundLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const f = await getFund(c, fund);
  const { views } = await listViews(c, fund);

  // ⛔ THE FUND SAYS WHICH VIEW IS DEFAULT, NOT THE COLLECTION. `ListViews`
  // used to answer both; AIP-132 admits only the list, and one source for a
  // fact is the point of the whole feature.
  const defaultView = f.defaultView;

  return (
    <main className="queue">
      {/* ⚠ RENDERS NOTHING. It registers this fund's screens, books of record,
          tickets and id deep-links with the palette that `funds/layout.tsx`
          mounts one level up — which is where the provider has to be, and this is
          where `listViews` has already been called. Neither costs a request. */}
      <FundActions fund={fund} views={views} defaultView={defaultView} />
      <PlaceHead
        fund={fund}
        displayName={f.displayName}
        views={views}
        defaultView={defaultView}
        meta={
          <>
            <span>
              {f.currencyCode} · {count(f.entryCount)} entries
            </span>
            {f.configDigest ? (
              <span>
                configuration <code>{f.configDigest.slice(0, 7)}</code>
              </span>
            ) : null}
          </>
        }
      />

      {children}
    </main>
  );
}
