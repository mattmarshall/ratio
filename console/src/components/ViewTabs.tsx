"use client";

import Link from "next/link";
import { useSelectedLayoutSegment } from "next/navigation";

/** The screens under one fund, in the order an operator works them. */
const VIEWS = [
  { segment: "breaks", label: "Exceptions" },
  { segment: "accounts", label: "Trial balance" },
  { segment: "positions", label: "Positions" },
  { segment: "data", label: "Data" },
  { segment: "strikes", label: "NAV" },
  { segment: "config", label: "Configuration" },
  { segment: "rules", label: "Rules" },
  { segment: "changes", label: "Change log" },
] as const;

/**
 * ⚠ EIGHT, WHERE THERE USED TO BE FOUR. The old console's four tabs were the
 * whole of it, and everything else — strikes, configuration versions, the
 * change log — was stacked below the current view where it could be scrolled
 * past. Each of those is a resource on the contract with a name of its own, so
 * each gets a URL. That is the trade this migration is for.
 */
export function ViewTabs({
  fund,
  pending,
}: {
  fund: string;
  pending: string;
}) {
  const here = useSelectedLayoutSegment();
  return (
    <nav className="views" aria-label="View">
      {VIEWS.map((v) => (
        <Link
          key={v.segment}
          href={`/funds/${fund}/${v.segment}`}
          aria-current={here === v.segment ? "page" : undefined}
        >
          {v.label}
          {/* The count an operator would otherwise only see after clicking. */}
          {v.segment === "data" && pending !== "0" ? (
            <b className="pip">{pending}</b>
          ) : null}
        </Link>
      ))}
    </nav>
  );
}
