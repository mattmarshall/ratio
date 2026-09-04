import Link from "next/link";
import { caller } from "@/lib/caller";
import { isoDate } from "@/lib/dates";
import { requireFundOps } from "@/lib/requireFundOps";
import { listCorporateActions } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * Corporate actions announced on this fund, applied or not.
 *
 * ⛔ `applied` IS NOT A STATUS FIELD; IT IS THE IDEMPOTENCE. Applying an action
 * twice doubles a position while the trial balance goes on tying — conservation
 * holds and the number is somebody else's. It is derived from the journal on
 * every read for exactly that reason: there is no client-side state that could
 * disagree with the book about whether an action went through.
 */
export default async function Actions({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  await requireFundOps(fund);
  const c = await caller();
  const { corporateActions } = await listCorporateActions(c, fund);

  return (
    <section className="log" aria-label="Corporate actions">
      <div className="loghead">
        <span>Corporate actions</span>
        <span className="sortnote">
          {corporateActions.length ? "announced, in journal order" : ""}
        </span>
      </div>
      {corporateActions.length === 0 ? (
        <div className="empty">Nothing has been announced on this fund.</div>
      ) : null}
      {corporateActions.map((a) => {
        const id = a.name.split("/").pop()!;
        return (
          <Link className="logrow" key={a.name} href={`/books/${fund}/actions/${id}`}>
            <span className="t num">
              {a.numerator}:{a.denominator}
            </span>
            <span className="w">
              <b>{a.instrument}</b>
              <div className="cfg">
                {a.form} · ex {isoDate(a.exDate)} · announced at position{" "}
                {a.announcePosition}
              </div>
            </span>
            <span className={`amt num${a.applied ? "" : " pos"}`}>
              {a.applied ? "applied" : "announced"}
            </span>
          </Link>
        );
      })}
    </section>
  );
}
