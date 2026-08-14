import Link from "next/link";
import { caller } from "@/lib/caller";
import { listPositions, listRules } from "@/wire/client";
import type { Position } from "@/wire/types";
import { TradeTicket } from "./TradeTicket";

export const dynamic = "force-dynamic";

/**
 * Place a trade by hand.
 *
 * ⭐ THE VIEW IS A QUERY PARAMETER, AND THAT IS THE WHOLE DESIGN. It scopes the
 * CONTEXT this screen shows — what the fund holds, in units and at a carrying
 * value — and scopes nothing about the write. Three things follow, and each was
 * the point:
 *
 *   * **The write stays fund-level.** `/funds/{fund}/trade` is the path;
 *     `?view=` is a lens on the panel beside the form. Putting the view in the
 *     PATH would say a trade is booked into a view, which is the confusion the
 *     view split exists to prevent — an entry goes to the journal and a view is
 *     a lens over the journal. `/record`, `/ingest` and `/mark` agree.
 *   * **It costs no `getFund`.** The positions screen an operator arrives from
 *     already knows its view, so the link carries it. Reading the fund's default
 *     instead would be a third render-time call, and with `applyEvent` that is
 *     four against the ceiling of three `//console:route_manifest_test`
 *     enforces. ⚠ A `<Suspense>` child would not help: the manifest counts a
 *     child segment's reads against the route that renders it, which is what
 *     `positions/[position]` declaring both `getPosition` and `listLots` says.
 *   * **⛔ NO VIEW NAMED MEANS NO FIGURES SHOWN.** Land here without one — a
 *     bookmark, a typed URL — and the instrument is a text field with no units
 *     and no carrying value beside it. That is the honest degradation rather
 *     than a convenient one: quantity and value depend on which entries a view
 *     recognises, and a figure that depends on a recognition convention and does
 *     not name it is exactly the defect the split was for.
 */
export default async function Trade({
  params,
  searchParams,
}: {
  params: Promise<{ fund: string }>;
  searchParams: Promise<{ view?: string }>;
}) {
  const { fund } = await params;
  const { view = "" } = await searchParams;
  const c = await caller();

  // ⚠ The rules always; the holdings only when a view says which book to read
  // them from. `Promise.all` because the two do not depend on each other and
  // two 15-second timeouts stack behind each.
  const [{ rules }, positions] = await Promise.all([
    listRules(c, fund),
    view
      ? listPositions(c, fund, view).then((r) => r.positions)
      : Promise.resolve([] as Position[]),
  ]);

  // ⛔ TRADE RULES ONLY, AND THE FILTER IS THE CONTRACT'S OWN. `Rule.Kind` says
  // what an event of each kind NEEDS, "which fixes what a caller must supply":
  // an accrual also wants a day count, and a mark is not invoked by recording an
  // event at all — its amount is the difference between carrying value and
  // market, and only a valuation knows that. Offering either here would be
  // offering a field this ticket does not have or a figure it must not compute.
  const tradeRules = rules.filter((r) => r.kind === "TRADE");

  return (
    <>
      <div className="qbar" role="group" aria-label="Writes">
        <span className="spacer" />
        <span className="sortnote">
          <Link href={`/funds/${fund}/record`}>Record another kind of event</Link>
        </span>
      </div>

      <TradeTicket
        fund={fund}
        rules={tradeRules}
        positions={positions}
        view={view}
      />
    </>
  );
}
