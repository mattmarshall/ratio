"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";
import { screensFor, SCREEN_GROUPS, screenHref } from "@/lib/screens";
import type { BookKind } from "@/wire/types";

/**
 * Places under one book, grouped — figures vs what was agreed.
 *
 * ⛔ NOT AN EIGHT-TAB STRIP. That warehouse is what made a personal book
 * look like a fund shell. Each job is a page; this is how you get there.
 * `scoped` still decides whether the URL names a book of record.
 */
export function ScreenTabs({
  fund,
  view,
  pending,
  kind = "INVESTMENT",
}: {
  fund: string;
  view: string;
  pending: string;
  kind?: BookKind;
}) {
  const segments = useSelectedLayoutSegments();
  const here = segments[0] === "views" ? segments[2] : segments[0];
  const screens = screensFor(kind);

  return (
    <nav className="places" aria-label="Places">
      {SCREEN_GROUPS.map((g) => (
        <div key={g.id} className="placegroup">
          <span className="placehead">{g.label}</span>
          {screens.filter((s) => s.group === g.id).map((s) => (
            <Link
              key={s.segment}
              href={screenHref(fund, view, s, "books")}
              aria-current={here === s.segment ? "page" : undefined}
            >
              {s.label}
              {s.segment === "data" && pending !== "0" ? (
                <b className="pip">{pending}</b>
              ) : null}
            </Link>
          ))}
        </div>
      ))}
    </nav>
  );
}
