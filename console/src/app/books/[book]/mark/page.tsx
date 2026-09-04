import { caller } from "@/lib/caller";
import { count, money } from "@/lib/format";
import { requireFundOps } from "@/lib/requireFundOps";
import { listPositions } from "@/wire/client";
import { MarkForm } from "./MarkForm";

export const dynamic = "force-dynamic";

/** Mark this fund's positions to market. */
export default async function Mark({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  // ⛔ A MARK IS FUND-LEVEL AND THE POSITIONS IT LISTS ARE NOT. Marking writes
  // entries to the one shared journal, so the screen stays here — but "held
  // now" is a question only a view can answer, and this one answers it for the
  // book's default view. The heading says so; a list of positions that does not
  // name its basis is the row already in HANDOFF.md's failure table.
  // Personal / Project / Operating 404 — leftover fund-ops chrome (#175).
  const b = await requireFundOps(fund);
  const c = await caller();
  const { positions } = await listPositions(c, fund, b.defaultView);

  return (
    <>
      <MarkForm fund={fund} />
      <section className="log" aria-label="What would be marked">
        <div className="loghead">
          <span>Held now, in {b.defaultView}</span>
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
