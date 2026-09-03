import { caller } from "@/lib/caller";
import { count } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getDelivery } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One file that arrived, and what came out of it. */
export default async function DeliveryDetail({
  params,
}: {
  params: Promise<{ book: string; delivery: string }>;
}) {
  const { book: fund, delivery } = await params;
  const c = await caller();
  const d = await or404(getDelivery(c, fund, delivery));

  return (
    <aside className="detail" aria-label="Delivery">
      <div className="dhead">
        <h2>{d.origin.split("/").pop()}</h2>
        <div className="sub">{d.receiveTime.slice(0, 16).replace("T", " ")}</div>
      </div>
      <div className="dsec">
        <h3>What arrived</h3>
        <dl className="kv">
          <dt>Facts</dt>
          <dd className="num">{count(d.factCount)}</dd>
          <dt>Pending</dt>
          <dd className="num">{count(d.pendingFactCount)}</dd>
          <dt>Bytes</dt>
          <dd className="num">{count(d.byteCount)}</dd>
        </dl>
      </div>
      <div className="dsec">
        <h3>Provenance</h3>
        {/* ⭐ HASHED ON ARRIVAL. The digest is what makes a fact traceable to a
            row of a file somebody sent, rather than to a claim about one. */}
        <div className="prov">
          digest <span className="g">{d.digest.slice(0, 12)}…</span>
          <br />
          {d.origin}
        </div>
      </div>
    </aside>
  );
}
