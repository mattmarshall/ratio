import type { ReactNode } from "react";
import { Stat } from "@/components/Stat";
import { caller } from "@/lib/caller";
import { basisOf, count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getView } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * The four figures that decide whether this view's NAV can be struck.
 *
 * ⛔ EVERY ONE OF THEM CARRIES ITS VIEW, THE WAY IT CARRIES ITS CURRENCY.
 * `134,439,187.51 USD` is not an answer until a reader also knows whether the
 * trades settling on Tuesday are in it. HANDOFF.md's failure table already has
 * the version of this that shipped: the console and the CLI reported different
 * NAVs for one book, neither saying which.
 *
 * ⚠ `journalPosition` IS THE SAME NUMBER ON EVERY VIEW OF THIS FUND, because
 * one pass over the journal feeds all of them. That is what makes the
 * reconciliation screen honest — a difference between two views is a
 * recognition convention and never one of them being three entries behind.
 * `//tla:views_check`'s `EveryViewFoldsTheSamePrefix`.
 */
export default async function ViewLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ fund: string; view: string }>;
}) {
  const { fund, view } = await params;
  const c = await caller();
  const v = await or404(getView(c, fund, view));
  const basis = basisOf(v.basis, v.settlesIn);

  return (
    <>
      <div className="stats">
        <Stat k="Net asset value" v={money(v.netAssetValue)} sub={basis} />
        <Stat
          k="Open difference"
          v={money(v.openDifference)}
          sub={basis}
          tone={v.openDifference === "0" ? undefined : "at-risk"}
        />
        <Stat
          k="Open breaks"
          v={count(v.openBreakCount)}
          sub={v.openBreakCount === "1" ? "exception" : "exceptions"}
          tone={v.openBreakCount === "0" ? "tied" : "at-risk"}
        />
        {/* ⛔ REPORTED, NOT HIDDEN. An entry this view cannot place — no trade
            date, or a configuration that does not declare it — contributes to
            no figure above. Leaving it off the screen would make a NAV look
            complete when it is short of entries nobody was told about. */}
        <Stat
          k="Unplaceable"
          v={count(v.unplaceableEntryCount)}
          sub={
            v.unplaceableEntryCount === "1"
              ? "entry this view cannot date"
              : "entries this view cannot date"
          }
          tone={v.unplaceableEntryCount === "0" ? "tied" : "at-risk"}
        />
      </div>

      {children}
    </>
  );
}
