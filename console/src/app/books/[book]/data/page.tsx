import Link from "next/link";
import { caller } from "@/lib/caller";
import { count } from "@/lib/format";
import { listDeliveries, listPendingFacts } from "@/wire/client";

export const dynamic = "force-dynamic";

/** Files in, facts out — and the facts that cannot post yet. */
export default async function Data({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  const c = await caller();
  const { deliveries } = await listDeliveries(c, fund);
  const { pendingFacts } = await listPendingFacts(c, fund);

  return (
    <>
      <div className="qbar" role="group" aria-label="Data">
        <span className="spacer" />
        <span className="sortnote">
          <Link href={`/books/${fund}/ingest`}>Read a file</Link>
          {" · "}
          <Link href={`/books/${fund}/data/templates`}>Templates</Link>
        </span>
      </div>

      <section className="log" aria-label="Files received">
        <div className="loghead">
          <span>Files received</span>
          <span className="sortnote">
            {deliveries.length ? "newest first · hashed on arrival" : ""}
          </span>
        </div>
        {deliveries.length === 0 ? (
          <div className="empty">
            Nothing delivered yet. <code>ratio ingest</code> reads a file.
          </div>
        ) : null}
        {deliveries.map((d) => {
          const id = d.name.split("/").pop()!;
          return (
            <Link
              className="logrow strike"
              key={d.name}
              href={`/books/${fund}/data/deliveries/${id}`}
            >
              <span className="t num">{d.digest.slice(0, 7)}</span>
              <span className="w">
                <b>{d.origin.split("/").pop()}</b>{" "}
                <span className="cfg">
                  {count(d.factCount)} fact{d.factCount === "1" ? "" : "s"} ·{" "}
                  {count(d.byteCount)} bytes ·{" "}
                  {d.receiveTime.slice(0, 16).replace("T", " ")}
                </span>
              </span>
              <span
                className={`amt num${d.pendingFactCount === "0" ? "" : " pos"}`}
              >
                {d.pendingFactCount === "0"
                  ? "all clear"
                  : `${d.pendingFactCount} pending`}
              </span>
            </Link>
          );
        })}
      </section>

      <section className="log" aria-label="Pending facts">
        <div className="loghead">
          <span>Pending</span>
          <span className="sortnote">
            {pendingFacts.length ? "these clear when the master does" : ""}
          </span>
        </div>
        {pendingFacts.length === 0 ? (
          <div className="empty">
            Nothing pending — every fact read resolves against the master.
          </div>
        ) : null}
        {pendingFacts.map((p) => {
          const id = p.name.split("/").pop()!;
          return (
            <Link
              className="logrow pend"
              key={p.name}
              href={`/books/${fund}/data/pending/${id}`}
            >
              <span
                className={`sev ${p.blocker === "AMBIGUOUS" ? "med" : "high"}`}
              />
              <span className="w">
                <b>{p.reference}</b>{" "}
                <span className="tagx">{p.blocker.toLowerCase()}</span>
                <div className="why">{p.detail}</div>
                {/* The chain back to the bytes: which row of which file, and
                    what mapped it. */}
                <div className="p2 num">
                  row {p.row} of {p.deliveryDigest.slice(0, 12)} · {p.templateId}
                </div>
              </span>
            </Link>
          );
        })}
      </section>
    </>
  );
}
