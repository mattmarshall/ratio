import Link from "next/link";
import { caller } from "@/lib/caller";
import { or404 } from "@/lib/or404";
import { getFact } from "@/wire/client";

export const dynamic = "force-dynamic";

/** One recorded fact, and the provenance a figure cites. */
export default async function FactDetail({
  params,
}: {
  params: Promise<{ book: string; fact: string }>;
}) {
  const { book: fund, fact } = await params;
  const c = await caller();
  const f = await or404(getFact(c, fund, fact));

  return (
    <aside className="detail" aria-label="Fact">
      <div className="dhead">
        <h2>{f.assertion || f.reference}</h2>
        <div className="sub">
          {f.kind}
          {f.superseded ? " · superseded" : ""}
        </div>
      </div>
      {f.superseded ? (
        <div className="dsec">
          <h3>A later fact superseded this one</h3>
          <p className="note">
            A correction is a new fact, never an edit. This row is still the
            evidence a figure struck that morning cited; the later row is the
            one in force now.
          </p>
        </div>
      ) : null}
      <div className="dsec">
        <h3>Where it came from</h3>
        <dl className="kv">
          <dt>Delivery</dt>
          <dd className="num">
            <Link
              href={`/books/${fund}/data/deliveries/${f.deliveryDigest.slice(0, 16)}`}
            >
              {f.deliveryDigest.slice(0, 12)}
            </Link>
          </dd>
          <dt>Row</dt>
          <dd className="num">{f.row}</dd>
          <dt>Template</dt>
          <dd>{f.templateId}</dd>
          <dt>Config</dt>
          <dd className="num">
            {f.configDigest ? (
              <Link href={`/books/${fund}/config/${f.configDigest}`}>
                {f.configDigest.slice(0, 12)}
              </Link>
            ) : (
              "—"
            )}
          </dd>
        </dl>
        <p className="note">
          The config digest is the one the ingest run pinned, not whichever is
          active now. Promoting a new rule set does not rewrite this fact.
        </p>
      </div>
    </aside>
  );
}
