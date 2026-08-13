import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { listPositions } from "@/wire/client";
import { MarkForm } from "./MarkForm";

export const dynamic = "force-dynamic";

/** Mark this fund's positions to market. */
export default async function Mark({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { positions } = await listPositions(c, fund);

  return (
    <>
      <MarkForm fund={fund} />
      <section className="log" aria-label="What would be marked">
        <div className="loghead">
          <span>Held now</span>
          <span className="sortnote">
            {positions.length} position{positions.length === 1 ? "" : "s"}
          </span>
        </div>
        {positions.map((p) => (
          <div className="logrow" key={p.name}>
            <span className="t num">{count(p.quantity)}</span>
            <span className="w">
              <b>{p.instrumentLabel || p.instrument}</b>
              <div className="cfg">{p.accountLabel || p.account}</div>
            </span>
            <span className="amt num">{money(p.value)}</span>
          </div>
        ))}
      </section>
    </>
  );
}
