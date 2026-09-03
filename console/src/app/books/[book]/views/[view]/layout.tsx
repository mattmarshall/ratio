import type { ReactNode } from "react";
import { Refusal } from "@/components/Refusal";
import { Stat } from "@/components/Stat";
import { bookOf, viewOf } from "@/lib/data";
import { basisOf, count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { orRefused } from "@/lib/refusal";

export const dynamic = "force-dynamic";

/**
 * The figures that belong on every child of this book of record.
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
 *
 * ⭐ A PERSONAL BOOK DOES NOT WEAR NAV CHROME. The same `netAssetValue` is
 * assets minus liabilities — net worth — and open breaks are a fund-ops
 * queue. Unplaceable stays: an undated entry is in no period P&L.
 */
export default async function ViewLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ book: string; view: string }>;
}) {
  const { book: fund, view } = await params;
  // ⛔ A REFUSAL RENDERS ITS SENTENCE, AND THE CHILDREN DO NOT FETCH. Thrown,
  // this reached production as `Minified React error #441` and a digest — the
  // API's one explanatory sentence redacted to a number, on every screen of
  // the dual-basis demo fund. The layout is the gate: if the view itself
  // refuses, every child's fetch would refuse identically, so answering here
  // once is both the fix and the cheaper page.
  //
  // ⭐ `viewOf` IS THE SAME GETVIEW THE PAGE READS. Asking twice would be two
  // Lambdas for one URL; React `cache` is the door `funds` already uses.
  const r = await orRefused(or404(viewOf(fund, view)));
  if (r.refused !== null) {
    return <Refusal why={r.refused} />;
  }
  const v = r.value;
  const basis = basisOf(v.basis, v.settlementOpenDays);
  const book = await or404(bookOf(fund));
  const personal = book.kind === "PERSONAL";

  return (
    <>
      <div className="stats">
        {personal ? (
          <>
            <Stat k="Net worth" v={money(v.netAssetValue)} sub={basis} />
            <Stat
              k="Unplaceable"
              v={count(v.unplaceableEntryCount)}
              sub={
                v.unplaceableEntryCount === "1"
                  ? "entry with no date — in no period"
                  : "entries with no date — in no period"
              }
              tone={v.unplaceableEntryCount === "0" ? "tied" : "at-risk"}
            />
          </>
        ) : (
          <>
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
          </>
        )}
      </div>

      {children}
    </>
  );
}
