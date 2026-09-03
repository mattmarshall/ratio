import Link from "next/link";
import { caller } from "@/lib/caller";
import { isoDate } from "@/lib/dates";
import { count, money } from "@/lib/format";
import { listLots } from "@/wire/client";

/**
 * The open tax lots behind one position.
 *
 * ⛔ ITS OWN COMPONENT SO IT CAN BE PUT IN A `<Suspense>`, AND THAT IS NOT
 * COSMETIC. `nextPageToken` is declared on `ListLotsResponse` and left empty by
 * every implementation, so this reads the whole lot book — and the benchmark
 * this product leads with goes to twenty million of them. The old console only
 * fetched them when somebody expanded a position; a position with its own URL
 * fetches them on load, so the guard has to become "the page renders without
 * waiting for this".
 */
export async function LotBook({
  fund,
  view,
  position,
}: {
  fund: string;
  view: string;
  position: string;
}) {
  const c = await caller();
  const { lots } = await listLots(c, fund, view, position);
  if (lots.length === 0) {
    return <div className="empty">No open lots behind this position.</div>;
  }
  return (
    <>
      {lots.map((l) => {
        const id = l.name.split("/").pop()!;
        return (
          <Link
            className="lotrow"
            key={l.name}
            href={`/books/${fund}/views/${view}/positions/${position}/lots/${id}`}
          >
            <span className="num">{l.sequence}</span>
            <span className="num">{count(l.units)}</span>
            <span className="num">{money(l.cost)}</span>
            {/* A lot with no acquisition date cannot be classified long or
                short. It says so rather than showing a guessed date. */}
            <span>{l.acquired ? isoDate(l.acquired) : "no trade date"}</span>
          </Link>
        );
      })}
    </>
  );
}
