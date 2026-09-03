"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";
import type { ReactNode } from "react";
import { SCREENS } from "@/lib/screens";
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
}: {
  fund: string;
  displayName: string;
  views: View[];
  defaultView: string;
  meta: ReactNode;
}) {
  const segs = useSelectedLayoutSegments();
  const underView = segs[0] === "views";
  const here = underView ? segs[2] : segs[0];
  const place = SCREENS.find((s) => s.segment === here);

  return (
    <div className="qhead">
      <Link href={`/books/${fund}`} className="bookcrumb">
        {displayName}
      </Link>
      {place ? <h1>{place.label}</h1> : underView ? null : <h1>{displayName}</h1>}
      <div className="subhead">
        {meta}
        {underView ? (
          <ViewSwitch fund={fund} views={views} defaultView={defaultView} />
        ) : null}
      </div>
    </div>
  );
}
