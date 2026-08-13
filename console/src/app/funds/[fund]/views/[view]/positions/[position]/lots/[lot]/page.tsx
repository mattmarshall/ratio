import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { isoDate } from "@/lib/dates";
import { or404 } from "@/lib/or404";
import { getLot } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One tax lot: what was bought, when, and at what cost. */
export default async function LotDetail({
  params,
}: {
  params: Promise<{ fund: string; view: string; position: string; lot: string }>;
}) {
  const { fund, view, position, lot } = await params;
  const c = await caller();
  const l = await or404(getLot(c, fund, view, position, lot));

  return (
    <aside className="detail" aria-label="Lot detail">
      <div className="dhead">
        <h2>Lot {l.sequence}</h2>
        <div className="sub">{position}</div>
      </div>
      <div className="dsec">
        <h3>The lot</h3>
        <dl className="kv">
          <dt>Units</dt>
          <dd className="num">{count(l.units)}</dd>
          <dt>Cost</dt>
          <dd className="num">{money(l.cost)}</dd>
          <dt>Acquired</dt>
          <dd>{isoDate(l.acquired)}</dd>
        </dl>
        {/* ⛔ ACQUISITION ORDER IS THE SEQUENCE, NOT THE DATE. `relieveFifo`
            takes the head of the list, so it is FIFO exactly when the caller
            supplied acquisition order — and a projection keyed by instrument
            does not. A lot with no acquisition date cannot be classified
            long or short and lands in unclassified rather than being guessed. */}
        <p className="note">
          Relief order follows the sequence, which is acquisition order. A lot
          with no trade date cannot be classified as long or short.
        </p>
      </div>
    </aside>
  );
}
