import Link from "next/link";
import { caller } from "@/lib/caller";
import { templatesForKind } from "@/lib/templates";
import { getBook, listTemplates } from "@/wire/client";

export const dynamic = "force-dynamic";

/** How a row of a delivered file becomes a fact. */
export default async function Templates({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  const c = await caller();
  const [book, listed] = await Promise.all([
    getBook(c, fund),
    listTemplates(c, fund),
  ]);
  // ⭐ KIND-AWARE. The live API already returns this book's configuration;
  // the filter is what stops a mixed fixture offering a fund snapshot on
  // a household book. An id that belongs to no kind stays if the book holds it.
  const templates = templatesForKind(book.kind, listed.templates);

  return (
    <section className="log" aria-label="Templates">
      <div className="loghead">
        <span>Templates</span>
        <span className="sortnote">
          {templates.length ? "a row in, a fact out" : ""}
        </span>
      </div>
      {templates.length === 0 ? (
        <div className="empty">
          This book has no mapping templates. A model proposes one; a person
          approves it.
        </div>
      ) : null}
      {templates.map((t) => {
        const id = t.name.split("/").pop()!;
        return (
          <Link
            className="logrow"
            key={t.name}
            href={`/books/${fund}/data/templates/${id}`}
          >
            <span className="t num">{t.templateId}</span>
            <span className="w">
              <b>{t.factKind}</b>
              <div className="cfg">{t.form}</div>
            </span>
            {/* Whether reading a file through this template posts, or only
                records a fact for something else to post later. */}
            <span className="amt num">{t.posts ? "posts" : "records"}</span>
          </Link>
        );
      })}
    </section>
  );
}
