import type { ReactNode } from "react";
import { ScreenTabs } from "@/components/ScreenTabs";
import { ViewSwitch } from "@/components/ViewSwitch";
import { caller } from "@/lib/caller";
import { count } from "@/lib/format";
import { getFund, listViews } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * One fund's heading: what it is, which books of record it keeps, and the
 * screens under it.
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
  const { views, defaultView } = await listViews(c, fund);

  return (
    <main className="queue">
      <div className="qhead">
        <h1>{f.displayName}</h1>
        <div className="subhead">
          <span>
            {f.currencyCode} · {count(f.entryCount)} entries
          </span>
          {f.configDigest ? (
            <span>
              configuration <code>{f.configDigest.slice(0, 7)}</code>
            </span>
          ) : null}
          <ViewSwitch fund={fund} views={views} defaultView={defaultView} />
          <ScreenTabs
            fund={fund}
            view={defaultView}
            pending={f.pendingFactCount}
          />
        </div>
      </div>

      {children}
    </main>
  );
}
