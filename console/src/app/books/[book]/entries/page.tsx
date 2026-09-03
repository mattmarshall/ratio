import Link from "next/link";
import { caller } from "@/lib/caller";
import { listEntries } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * The journal — every entry, in the order the book holds them.
 *
 * ⭐ THE LIST AIP-121 REQUIRES ONCE GetEntry EXISTS. A resource with Get and
 * no List is a citation you cannot walk up from. This is that walk: the
 * journal, each row a link to the page #52 asked for.
 *
 * ⛔ NOT THE CHANGE LOG. `/books/{book}/changes` is proposals and approvals.
 */
export default async function Entries({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book } = await params;
  const c = await caller();
  const { entries } = await listEntries(c, book);

  return (
    <section className="log" aria-label="Journal">
      <div className="loghead">
        <span>Journal</span>
        <span className="sortnote">{entries.length ? "journal order" : ""}</span>
      </div>
      {entries.length === 0 ? (
        <div className="empty">Nothing has been posted to this book.</div>
      ) : null}
      {entries.map((e) => (
        <Link
          className="logrow"
          key={e.name}
          href={`/books/${book}/entries/${e.entryId}`}
        >
          <span className="t num">{e.configDigest.slice(0, 7)}</span>
          <span className="w">
            <b>{e.memo || e.entryId}</b>
            <div className="cfg">
              {e.entryId}
              {e.postings.length
                ? ` · ${e.postings.length} posting${e.postings.length === 1 ? "" : "s"}`
                : ""}
            </div>
          </span>
        </Link>
      ))}
    </section>
  );
}
