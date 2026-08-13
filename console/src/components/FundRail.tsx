"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";
import { STATE_CLASS, STATE_LABEL } from "@/lib/format";
import type { Fund } from "@/wire/types";

/**
 * The funds this operator is responsible for.
 *
 * ⚠ A CLIENT COMPONENT FOR ONE REASON: to know which fund is open. The list
 * itself is fetched on the server and handed down as a prop — nothing here
 * calls the API. A layout cannot see its child segment's params, and
 * `useSelectedLayoutSegments` can, which is the whole of why this crosses the
 * boundary.
 *
 * ⚠ EVERY FIELD ON `Fund` IS A STRING, INCLUDING THE COUNTS. That is what makes
 * this prop safe to serialize: the moment a caller turns `openBreakCount` into a
 * BigInt to compare it, passing it across this boundary throws at runtime.
 */
export function FundRail({ funds }: { funds: Fund[] }) {
  // ["<fund>", "breaks", …] — the first segment under /funds is the fund id.
  const open = useSelectedLayoutSegments()[0];

  return (
    <nav className="funds" aria-label="Funds">
      <div className="railhead">
        <span>Your funds</span>
        <span>{funds.length || ""}</span>
      </div>
      {funds.length === 0 ? (
        <div className="empty">No funds are granted to you.</div>
      ) : null}
      {funds.map((f) => {
        const id = f.name.replace(/^funds\//, "");
        return (
          <Link
            key={f.name}
            href={`/funds/${id}/breaks`}
            className="fund"
            aria-current={id === open ? "page" : undefined}
          >
            <div className="name">{f.displayName}</div>
            <div className="meta">
              <span className={`state ${STATE_CLASS[f.state]}`}>
                {STATE_LABEL[f.state]}
              </span>
              <span className="num">
                {f.openBreakCount === "0" ? "—" : `${f.openBreakCount} open`}
              </span>
            </div>
          </Link>
        );
      })}
    </nav>
  );
}
