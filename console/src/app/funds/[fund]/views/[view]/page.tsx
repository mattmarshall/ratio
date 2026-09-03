import Link from "next/link";
import { caller } from "@/lib/caller";
import { basisOf, count, money } from "@/lib/format";
import { or404 } from "@/lib/or404";
import { getView } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * A book of record — the view itself, not a child screen under it.
 *
 * ⭐ THIS IS THE PAGE #53 ASKED FOR. `funds/{fund}/views/{view}` is a real
 * resource and used to land on the exceptions queue because the segment had a
 * layout and no page. The URL is now the citation.
 */
export default async function ViewPage({
  params,
}: {
  params: Promise<{ fund: string; view: string }>;
}) {
  const { fund, view } = await params;
  const c = await caller();
  const v = await or404(getView(c, fund, view));
  const basis = basisOf(v.basis, v.settlementOpenDays);

  return (
    <section className="lots">
      <div className="loghead">
        <span>{v.displayName}</span>
        <span className="sortnote">{basis}</span>
      </div>
      <dl className="kv">
        <dt>Net asset value</dt>
        <dd className="num">{money(v.netAssetValue)}</dd>
        <dt>Open difference</dt>
        <dd className="num">{money(v.openDifference)}</dd>
        <dt>Open breaks</dt>
        <dd className="num">{count(v.openBreakCount)}</dd>
        <dt>Declared</dt>
        <dd>{v.declared ? "yes — a term of the configuration" : "no — the journal's own order"}</dd>
      </dl>
      <p className="note">
        <Link href={`/funds/${fund}/views/${view}/breaks`}>Exceptions</Link>
        {" · "}
        <Link href={`/funds/${fund}/views/${view}/accounts`}>Trial balance</Link>
        {" · "}
        <Link href={`/books/${fund}`}>Book</Link>
      </p>
    </section>
  );
}
