import Link from "next/link";
import { caller } from "@/lib/caller";
import { listPositions, listRules } from "@/wire/client";
import { TradeTicket } from "./TradeTicket";

export const dynamic = "force-dynamic";

/**
 * Place a trade by hand.
 *
 * ⚠ TWO READS, AND BOTH ARE THE TICKET'S. The rules in force are what a trade
 * can book under, and the positions are what it can be a trade IN — a picker
 * over the instrument master would be a third call this page has no budget for,
 * and what the fund holds is the better default anyway. With `applyEvent` that
 * is three against the ceiling `//console:route_manifest_test` enforces, which
 * is why the fund header and its four tiles are the layout's `GetFund` and not
 * repeated here.
 */
export default async function Trade({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const [{ rules }, { positions }] = await Promise.all([
    listRules(c, fund),
    listPositions(c, fund),
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

      <TradeTicket fund={fund} rules={tradeRules} positions={positions} />
    </>
  );
}
