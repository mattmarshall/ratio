import Link from "next/link";
import { caller } from "@/lib/caller";
import { listRules } from "@/wire/client";
import { TradeTicket } from "./TradeTicket";

export const dynamic = "force-dynamic";

/**
 * Place a trade by hand.
 *
 * ⚠ NO POSITION PICKER, AND THAT IS THE VIEW SPLIT'S DOING. The instrument used
 * to be chosen from what the fund holds, with the units and carrying value of a
 * sale's holding shown beside it. `listPositions` is now view-scoped — quantity
 * and value depend on which entries a view recognises — so those figures cannot
 * be shown without naming the view they were read in, and naming it costs a
 * `getFund` for the default. That is four upstream calls against the ceiling of
 * three `//console:route_manifest_test` enforces, and the ceiling is the right
 * one: two 15-second timeouts stack behind every call.
 *
 * ⛔ AND THE WRITE IS NOT VIEW-SCOPED, so this screen must not move under
 * `views/[view]/` to get one for free. An entry goes to the journal; a view is a
 * lens over the journal. `/record`, `/ingest` and `/mark` are all fund-level for
 * the same reason. The way back to a picker is a `<Suspense>` child segment that
 * reads the default view for itself — worth doing, and not worth blocking a
 * first cut on.
 */
export default async function Trade({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { rules } = await listRules(c, fund);

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

      <TradeTicket fund={fund} rules={tradeRules} />
    </>
  );
}
