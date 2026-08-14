"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";

/**
 * The screens under one fund, in the order an operator works them.
 *
 * ⛔ RENAMED FROM `ViewTabs`, AND THE RENAME IS THE POINT. A "view" is now a
 * book of record — ABOR, IBOR — and leaving the screen tabs called views put
 * two different things under one word in one header. A console that is vague
 * about which figure it is showing is the failure HANDOFF.md already records:
 * the console and the CLI reported different NAVs for one book, neither saying
 * which. The `.views` CSS class went with it, to `.screens`.
 */
const SCREENS = [
  { segment: "breaks", label: "Exceptions", scoped: true },
  { segment: "accounts", label: "Trial balance", scoped: true },
  { segment: "positions", label: "Positions", scoped: true },
  { segment: "data", label: "Data", scoped: false },
  { segment: "strikes", label: "NAV", scoped: true },
  { segment: "config", label: "Configuration", scoped: false },
  { segment: "rules", label: "Rules", scoped: false },
  { segment: "changes", label: "Change log", scoped: false },
] as const;

/**
 * ⚠ `scoped` IS NOT COSMETIC. A screen showing figures folded from a journal
 * prefix belongs to one view and lives under `views/<id>/`; one showing what was
 * AGREED — the configuration, the rules, the change log, the deliveries — does
 * not, because a rule set is the same document whichever way you recognise the
 * entries it produced. Getting this wrong in either direction is a URL that
 * lies about what it is showing.
 */
export function ScreenTabs({
  fund,
  view,
  pending,
}: {
  fund: string;
  view: string;
  pending: string;
}) {
  // ["views", "<view>", "breaks"] under a view; ["config"] at fund level.
  const segments = useSelectedLayoutSegments();
  const here = segments[0] === "views" ? segments[2] : segments[0];

  return (
    <nav className="screens" aria-label="Screen">
      {SCREENS.map((s) => (
        <Link
          key={s.segment}
          href={
            s.scoped
              ? `/funds/${fund}/views/${view}/${s.segment}`
              : `/funds/${fund}/${s.segment}`
          }
          aria-current={here === s.segment ? "page" : undefined}
        >
          {s.label}
          {/* The count an operator would otherwise only see after clicking. */}
          {s.segment === "data" && pending !== "0" ? (
            <b className="pip">{pending}</b>
          ) : null}
        </Link>
      ))}
    </nav>
  );
}
