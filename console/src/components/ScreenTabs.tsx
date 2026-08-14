"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";
import { SCREENS, screenHref } from "@/lib/screens";

/**
 * The screens under one fund, in the order an operator works them.
 *
 * ⛔ RENAMED FROM `ViewTabs`, AND THE RENAME IS THE POINT. A "view" is now a
 * book of record — ABOR, IBOR — and leaving the screen tabs called views put
 * two different things under one word in one header. A console that is vague
 * about which figure it is showing is the failure HANDOFF.md already records:
 * the console and the CLI reported different NAVs for one book, neither saying
 * which. The `.views` CSS class went with it, to `.screens`.
 *
 * ⚠ THE LIST ITSELF MOVED TO `lib/screens.ts` when the command palette began
 * offering the same eight screens. Two copies would be two answers to what
 * `scoped` means, and `scoped` decides whether a URL tells the truth about
 * which book of record produced the figures on it.
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
          href={screenHref(fund, view, s)}
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
