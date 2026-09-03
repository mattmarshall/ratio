import Link from "next/link";
import { caller } from "@/lib/caller";
import { listFacts } from "@/wire/client";

export const dynamic = "force-dynamic";

/** Prices, FX, and every other recorded fact — newest first. */
export default async function Facts({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  const c = await caller();
  const { facts } = await listFacts(c, fund);

  return (
    <section className="log" aria-label="Facts">
      <div className="loghead">
        <span>Facts</span>
        <span className="sortnote">
          {facts.length ? "newest first · a correction is a new row" : ""}
        </span>
      </div>
      {facts.length === 0 ? (
        <div className="empty">
          Nothing recorded yet. <code>ratio ingest</code> reads a file into
          facts.
        </div>
      ) : null}
      {facts.map((f) => {
        const id = f.name.split("/").pop()!;
        return (
          <Link
            className="logrow"
            key={f.name}
            href={`/books/${fund}/data/facts/${id}`}
          >
            <span className="t num">{f.kind}</span>
            <span className="w">
              <b>{f.assertion || f.reference}</b>
              {f.superseded ? <span className="tagx">superseded</span> : null}
              <div className="p2 num">
                row {f.row} of {f.deliveryDigest.slice(0, 12)} · config{" "}
                {f.configDigest.slice(0, 12)}
              </div>
            </span>
            <span className="amt num">{f.templateId}</span>
          </Link>
        );
      })}
    </section>
  );
}
