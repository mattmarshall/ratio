import Link from "next/link";
import type { ReactNode } from "react";
import { Brand } from "@/components/Brand";
import { Who } from "@/components/Who";
import { books as booksForRequest } from "@/lib/data";

export const dynamic = "force-dynamic";

/**
 * Chrome for the book collection. Nested /books/:id/views/... screens rewrite
 * onto the existing fund pages, so they keep the fund frame.
 */
export default async function BooksLayout({
  children,
}: {
  children: ReactNode;
}) {
  const books = await booksForRequest();

  return (
    <div className="app">
      <header className="top">
        <Link href="/books" className="brandlink" aria-label="Your books">
          <Brand />
        </Link>
        <span className="crumb">
          Books <span aria-hidden="true">/</span> <b>{books.length}</b>
        </span>
        <span className="spacer" />
        <Who />
      </header>
      <div className="body">{children}</div>
    </div>
  );
}
