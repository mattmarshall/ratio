import Link from "next/link";
import { books as booksForRequest } from "@/lib/data";
import { count } from "@/lib/format";

export const dynamic = "force-dynamic";

const KIND_LABEL: Record<string, string> = {
  PERSONAL: "Personal",
  INVESTMENT: "Investment",
  PROJECT: "Project",
  UNSPECIFIED: "Book",
};

/** Every book this operator may open — funds are an optional layer, not a parent. */
export default async function Books() {
  const books = await booksForRequest();

  return (
    <main className="queue">
      <div className="qhead">
        <h1>Your books</h1>
        <div className="subhead">
          <span>{count(String(books.length))} open to you</span>
          <Link href="/books/new">New book</Link>
        </div>
      </div>

      <ul className="rows">
        {books.length === 0 ? (
          <li>
            <div className="empty">
              No books yet. Create one without a fund or an organization —
              membership is a line in <code>MEMBERSHIP.tsv</code>, not a parent
              resource.
            </div>
          </li>
        ) : null}
        {books.map((b) => {
          const id = b.name.replace(/^books\//, "");
          const href = b.defaultView
            ? `/books/${id}/views/${b.defaultView}/breaks`
            : `/books/${id}`;
          return (
            <li key={b.name}>
              <Link className="row" href={href}>
                <span className={`sev ${b.fund ? "low" : "high"}`} />
                <span>
                  <div className="title">{b.displayName}</div>
                  <div className="why">
                    {KIND_LABEL[b.kind] ?? b.kind}
                    {b.fund ? " · filed as a fund" : " · independent"}
                    {" · "}
                    {count(b.entryCount)} entries
                  </div>
                </span>
                <span className="amt num">
                  {b.configDigest ? b.configDigest.slice(0, 7) : "none"}
                  <small>configuration</small>
                </span>
              </Link>
            </li>
          );
        })}
      </ul>
    </main>
  );
}
