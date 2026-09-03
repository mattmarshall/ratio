"use client";

import Link from "next/link";
import { useSelectedLayoutSegments } from "next/navigation";
import { BASIS_LABEL } from "@/lib/format";
import type { View } from "@/wire/types";

/**
 * Which book of record the figures on this screen come from.
 *
 * ⭐ ON FIGURE PAGES ONLY, BESIDE THE CURRENCY, NEVER AMONG JOB LINKS. A
 * view is a property of the ANSWER, exactly as the reporting currency is —
 * every figure below it means something different depending on which one is
 * selected. Rendering it as a ninth tab, or on Configuration, would say a
 * view is a place — or that a rule set belongs to ABOR. Both are lies.
 *
 * ⚠ A CLIENT COMPONENT FOR ONE REASON: to know which view is open and which
 * screen to stay on. The list is fetched on the server and handed down; nothing
 * here calls the API.
 */
export function ViewSwitch({
  fund,
  views,
  defaultView,
}: {
  fund: string;
  views: View[];
  defaultView: string;
}) {
  // ["views", "<view>", "breaks"] under a view; ["config"] at fund level.
  const segments = useSelectedLayoutSegments();
  const underView = segments[0] === "views";
  const open = underView ? segments[1] : defaultView;
  // Stay on the same screen when switching views. A control-plane screen is not
  // view-scoped, so this control is not shown there.
  const screen = underView && segments[2] ? segments[2] : "";

  // ⚠ A single view is still reported, not hidden. A book with one view has
  // one, and a reader who cannot see which basis produced a figure is the
  // reader this whole split exists for.
  return (
    <nav className="viewswitch" aria-label="Book of record">
      {views.map((v) => {
        const id = v.name.split("/").pop()!;
        return (
          <Link
            key={v.name}
            href={
              screen
                ? `/books/${fund}/views/${id}/${screen}`
                : `/books/${fund}/views/${id}`
            }
            aria-current={id === open ? "page" : undefined}
            title={BASIS_LABEL[v.basis]}
          >
            {v.displayName}
            {/* ⛔ AN UNDECLARED VIEW SAYS SO. A book whose configuration
                declares nothing has exactly one view, recognising entries in
                the journal's own order — and labelling that as an elected
                basis asserts a decision nobody made. Same trap, and same fix,
                as `lotMethodDeclared`. */}
            {v.declared ? null : <b className="pip">default</b>}
          </Link>
        );
      })}
    </nav>
  );
}
