import type { ReactNode } from "react";
import { FundActions } from "@/components/FundActions";
import { PlaceHead } from "@/components/PlaceHead";
import { caller } from "@/lib/caller";
import { count } from "@/lib/format";
import { getBook, listViews } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * Identity of the open book. Each job under it is its own page — this
 * layout names the book and, on a figure page, which book of record
 * produced the numbers. It does not list the jobs.
 *
 * ⭐ ONE `GetBook` AND ONE `ListViews` FOR EVERY SCREEN UNDER THIS BOOK.
 * A layout renders once per book and persists across navigation within it,
 * so no child repeats either. Two 15-second timeouts stack behind each call
 * and `watch.rs` closes the connection after every response, so what decides
 * whether a page renders is the NUMBER of calls —
 * `//console:route_manifest_test` caps it at three per file.
 *
 * ⛔ THE FIGURES STAY A LEVEL DOWN. A NAV, a realized gain, a lot count and
 * an open difference all depend on WHICH ENTRIES ARE RECOGNISED, so they
 * belong to a view and are rendered by `views/[view]/layout.tsx`. Showing
 * them here would mean showing them on the configuration and change-log
 * screens too, where there is no view selected to qualify them — which is a
 * figure that does not say which question it answers.
 *
 * ⚠ `listViews` STILL SPEAKS `funds/*` ON THE WIRE. That is the AIP parent
 * the contract kept; the console URL is `/books/{book}/…`. GetFund still
 * answers for any book directory, including one CreateBook wrote with no
 * fund sidecar.
 */
export default async function BookLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ book: string }>;
}) {
  const { book } = await params;
  const c = await caller();
  const b = await getBook(c, book);
  const { views } = await listViews(c, book);
  const defaultView = b.defaultView;

  return (
    <main className="queue">
      {/* ⚠ RENDERS NOTHING. It registers this book's screens, books of record,
          tickets and id deep-links with the palette that `books/layout.tsx`
          mounts one level up — which is where the provider has to be, and this
          is where `listViews` has already been called. Neither costs a request. */}
      <FundActions fund={book} views={views} defaultView={defaultView} kind={b.kind} />
      <PlaceHead
        fund={book}
        displayName={b.displayName}
        views={views}
        defaultView={defaultView}
        identity="crumb"
        meta={
          <>
            <span>
              {b.currencyCode} · {count(b.entryCount)} entries
            </span>
            {b.configDigest ? (
              <span>
                configuration <code>{b.configDigest.slice(0, 7)}</code>
              </span>
            ) : null}
          </>
        }
      />

      {children}
    </main>
  );
}
