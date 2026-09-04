"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";
import type { ReactNode } from "react";
import { placeOf } from "@/lib/screens";
import type { View } from "@/wire/types";
import { ViewSwitch } from "./ViewSwitch";

/**
 * Identity of the open book, and the title of the place you are in.
 *
 * ⛔ NOT A TAB STRIP. The eight jobs are pages. This names the one you
 * opened. ViewSwitch appears only on a book of record — putting it on
 * Configuration would say a rule set belongs to ABOR, which is a lie.
 */
export function PlaceHead({
  fund,
  displayName,
  views,
  defaultView,
  meta,
  identity = "heading",
}: {
  fund: string;
  displayName: string;
  views: View[];
  defaultView: string;
  meta: ReactNode;
  /**
   * On a fund filing page the identity IS the heading. On a book the hub
   * page already titles it, so this layout only crumbs — otherwise a
   * personal book would render its name twice.
   */
  identity?: "heading" | "crumb";
}) {
  const segs = useSelectedLayoutSegments();
  const underView = segs[0] === "views";
  const here = underView ? segs[2] : segs[0];
  const place = placeOf(here);
  const title = place
    ? place.label
    : underView
      ? null
      : identity === "heading"
        ? displayName
        : null;

  // Hub (`identity="crumb"`, no place): the page already titles the book.
  // Rendering this qhead too was Household-the-crumb on Household-the-h1.
  if (!title && identity === "crumb" && !underView) {
    return null;
  }

  return (
    <div className="qhead">
      <Link href={`/books/${fund}`} className="bookcrumb">
        {displayName}
      </Link>
      {title ? <h1>{title}</h1> : null}
      <div className="subhead">
        {meta}
        {underView ? (
          <ViewSwitch fund={fund} views={views} defaultView={defaultView} />
        ) : null}
      </div>
    </div>
  );
}
