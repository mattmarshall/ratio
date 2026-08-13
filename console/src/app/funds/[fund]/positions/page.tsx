import Link from "next/link";
import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { isoDate } from "@/lib/dates";
import { listPositions } from "@/wire/client";

export const dynamic = "force-dynamic";

/** What the fund holds, grouped by the account the value sits in. */
export default async function Positions({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { positions } = await listPositions(c, fund);

  const byAccount = new Map<string, typeof positions>();
  for (const p of positions) {
    const k = p.accountLabel || p.account;
    byAccount.set(k, [...(byAccount.get(k) ?? []), p]);
  }

  return (
    <>
      <div className="qbar" role="group" aria-label="Positions">
        <span className="spacer" />
        <span className="sortnote">
          <Link href={`/funds/${fund}/mark`}>Mark to market</Link>
        </span>
      </div>

      {positions.length === 0 ? (
        <div className="empty">This fund holds nothing.</div>
      ) : null}

      {[...byAccount.entries()].map(([label, rows]) => (
        <section className="log" key={label} aria-label={label}>
          <div className="loghead">
            <span>{label}</span>
            <span className="sortnote">
              {/* ⚠ A COUNT, NOT A SUM. An instrument is a PARTITIONING
                  dimension: it says where value sits, not how much there is.
                  Quantity is MEASURED — buying 100 shares creates 100 in the
                  book — so a total over units would be a figure about nothing. */}
              {rows.length} position{rows.length === 1 ? "" : "s"}
            </span>
          </div>
          {rows.map((p) => {
            const id = p.name.split("/").pop()!;
            return (
              <Link
                className="logrow"
                key={p.name}
                href={`/funds/${fund}/positions/${id}`}
              >
                <span className="t num">{count(p.quantity)}</span>
                <span className="w">
                  <b>{p.instrumentLabel || p.instrument}</b>
                  <div className="cfg">
                    {count(p.openLotCount)} open lot
                    {p.openLotCount === "1" ? "" : "s"} · marked{" "}
                    {isoDate(p.markDate)}
                  </div>
                </span>
                <span className="amt num">{money(p.value)}</span>
              </Link>
            );
          })}
        </section>
      ))}
    </>
  );
}
