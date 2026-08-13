import type { ReactNode } from "react";
import { Stat } from "@/components/Stat";
import { ViewTabs } from "@/components/ViewTabs";
import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { getFund } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * One fund's heading and the four figures that decide whether its NAV can be
 * struck.
 *
 * ⭐ ONE `GetFund` FOR EVERY SCREEN UNDER THIS FUND. A layout renders once per
 * fund and persists across navigation within it, so a child page never repeats
 * this call — which is what keeps every page inside the three-upstream-call
 * budget `//console:route_manifest_test` enforces. Two 15-second timeouts stack
 * behind each call, and `watch.rs` closes the connection after every response,
 * so what decides whether a page renders is the NUMBER of calls.
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
          <ViewTabs fund={fund} pending={f.pendingFactCount} />
        </div>
      </div>

      <div className="stats">
        <Stat k="Net asset value" v={money(f.netAssetValue)} sub={f.currencyCode} />
        <Stat
          k="Trial balance"
          v={money(f.trialBalanceDifference)}
          sub="difference"
          tone={f.trialBalanceDifference === "0" ? "tied" : undefined}
        />
        <Stat
          k="Pending"
          v={count(f.pendingFactCount)}
          sub={f.pendingFactCount === "1" ? "fact unresolved" : "facts unresolved"}
          tone={f.pendingFactCount === "0" ? "tied" : "at-risk"}
        />
        <Stat
          k="Open difference"
          v={money(f.openDifference)}
          sub={f.currencyCode}
          tone={f.openDifference === "0" ? undefined : "at-risk"}
        />
      </div>

      {children}
    </main>
  );
}
