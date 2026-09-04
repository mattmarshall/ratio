import Link from "next/link";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import { hrefForResourceName } from "@/lib/deeplink";
import { or404 } from "@/lib/or404";
import { getEntry } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * One journal entry — the thing every posting cites.
 *
 * ⭐ THIS IS THE PAGE #52 ASKED FOR. `funds/{fund}/entries/{entry}` is a real
 * resource (the AIP name; the URL is `/books/{book}/entries/{entry}` after
 * #64) and used to be an identifier printed as text on the posting screen.
 * The URL is now the citation.
 *
 * ⛔ NOT `/books/{book}/changes/{entry}`. That is the change log.
 */
async function EntryDetail({
  params,
}: {
  params: Promise<{ book: string; entry: string }>;
}) {
  const { book, entry } = await params;
  const c = await caller();
  const e = await or404(getEntry(c, book, entry));

  return (
    <aside className="detail" aria-label="Journal entry">
      <div className="dhead">
        <h2>{e.memo || e.entryId}</h2>
        <div className="sub">{e.entryId}</div>
      </div>
      <div className="dsec">
        <h3>Provenance</h3>
        <dl className="kv">
          <dt>Configuration</dt>
          <dd>
            <Link href={`/books/${book}/config/${e.configDigest}`}>
              {e.configDigest.slice(0, 12)}…
            </Link>
          </dd>
          <dt>Identified lots</dt>
          <dd>
            {/* ⛔ NOT A SILENT FIFO. A sale that names nothing is not
                SpecID. An empty list that is elected is unnamed and
                refuses. `lot_method = "specific_id"` stays refused. */}
            {e.identifiedLotsDeclared
              ? e.identifiedLots.length
                ? e.identifiedLots.join(", ")
                : "unnamed"
              : "—"}
            <span className="sub">
              {e.identifiedLotsDeclared
                ? e.identifiedLots.length
                  ? " named on this sale — not a lot method"
                  : " elected and unnamed — the engine refuses"
                : " this sale does not name lots"}
            </span>
          </dd>
        </dl>
      </div>
      <div className="dsec postings">
        <h3>The postings it produced</h3>
        {e.postings.length === 0 ? (
          <div className="sub">This entry moved no accounts.</div>
        ) : (
          e.postings.map((p, i) => {
            const href = hrefForResourceName(p.account);
            const label = p.displayName || p.account;
            return (
              <div className="posting" key={`${p.account}-${i}`}>
                <span>
                  <div className="p1">
                    {href ? <Link href={href}>{label}</Link> : label}
                  </div>
                  <div className="p2">{p.amount.startsWith("-") ? "credit" : "debit"}</div>
                </span>
                <span className="num">{money(p.amount)}</span>
              </div>
            );
          })
        )}
      </div>
      <p className="note">
        <Link href={`/books/${book}/entries`}>Journal</Link>
        {" · "}
        <Link href={`/books/${book}`}>Book</Link>
      </p>
    </aside>
  );
}

export default withRefusal(EntryDetail);
