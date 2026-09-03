import Link from "next/link";
import { WorkspaceSwitch } from "@/components/WorkspaceSwitch";
import { books as booksForRequest } from "@/lib/data";
import { count } from "@/lib/format";
import { KIND_SHORT } from "@/lib/templates";

export const dynamic = "force-dynamic";

/**
 * Books of kind PROJECT. Still a Book — jobs live at `/books/{id}/…`.
 *
 * ⛔ NOT A SECOND LEDGER. ListBooks already returns every book; this is
 * the same list filtered by the template CreateBook wrote.
 */
export default async function Projects() {
  const books = (await booksForRequest()).filter((b) => b.kind === "PROJECT");

  return (
    <main className="queue">
      <div className="qhead">
        <h1>Your projects</h1>
        <div className="subhead">
          <span>{count(String(books.length))} open to you</span>
          <Link href="/books">All books</Link>
          <Link href="/funds">Funds</Link>
          <Link href="/books/new">New book</Link>
          <WorkspaceSwitch current="projects" />
        </div>
      </div>

      <ul className="rows">
        {books.length === 0 ? (
          <li>
            <div className="empty">
              No project books yet. Create one from New book — a project is a
              Book with the project chart, not a second ledger.
            </div>
          </li>
        ) : null}
        {books.map((b) => {
          const id = b.name.replace(/^books\//, "");
          return (
            <li key={b.name}>
              <Link className="row" href={`/books/${id}`}>
                <span className="sev high" />
                <span>
                  <div className="title">{b.displayName}</div>
                  <div className="why">
                    {KIND_SHORT[b.kind] ?? b.kind}
                    {" · independent"}
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
