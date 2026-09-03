import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getPosting } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/** One line, and the entry and configuration it came from. */
async function PostingDetail({
  params,
}: {
  params: Promise<{ book: string; view: string; account: string; posting: string }>;
}) {
  const { book: fund, view, account, posting } = await params;
  const c = await caller();
  const p = await or404(getPosting(c, fund, view, account, posting));

  return (
    <aside className="detail" aria-label="Posting detail">
      <div className="dhead">
        <h2>{p.memo || "—"}</h2>
        <div className="sub">{p.entryId}</div>
      </div>
      <div className="dsec">
        <h3>The line</h3>
        <dl className="kv">
          <dt>Amount</dt>
          <dd className="num">{money(p.amount)}</dd>
          <dt>Running balance</dt>
          <dd className="num">{money(p.runningBalance)}</dd>
        </dl>
      </div>
      <div className="dsec">
        <h3>Provenance</h3>
        <div className="prov">
          configuration <span className="g">{p.configDigest.slice(0, 12)}…</span>
          <br />
          entry {p.entryId}
        </div>
      </div>
    </aside>
  );
}

export default withRefusal(PostingDetail);
