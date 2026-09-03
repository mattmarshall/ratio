import Link from "next/link";
import type { ReactNode } from "react";
import { Brand } from "@/components/Brand";
import { CommandHint } from "@/components/CommandHint";
import { Palette } from "@/components/Palette";
import { Who } from "@/components/Who";
import { Unavailable } from "@/components/Unavailable";
import { books as booksForRequest, funds as fundsForRequest } from "@/lib/data";

export const dynamic = "force-dynamic";

/**
 * Chrome for the book collection and every job under a book.
 *
 * ⭐ THE JOBS LIVE HERE. `/books/{book}/views/…` is the real URL, not a
 * rewrite onto a fund page. A personal book never travels through `/funds`.
 * The palette mounts here so ⌘K reaches those jobs; ListFunds stays on
 * `/funds` and is funds-only.
 */
export default async function BooksLayout({
  children,
}: {
  children: ReactNode;
}) {
  const booksRead = await booksForRequest();
  const fundsRead = await fundsForRequest();
  if (booksRead.unavailable !== null) {
    return <Unavailable why={booksRead.unavailable} />;
  }
  if (fundsRead.unavailable !== null) {
    return <Unavailable why={fundsRead.unavailable} />;
  }
  const books = booksRead.value;
  const funds = fundsRead.value;

  return (
    <Palette funds={funds}>
      <div className="app">
        <header className="top">
          <Link href="/books" className="brandlink" aria-label="Your books">
            <Brand />
          </Link>
          <span className="crumb">
            Books <span aria-hidden="true">/</span> <b>{books.length}</b>
          </span>
          <span className="spacer" />
          <Link href="/funds" className="crumb">
            Funds
          </Link>
          <Link href="/projects" className="crumb">
            Projects
          </Link>
          <CommandHint />
          <Who />
        </header>
        <div className="body norail">{children}</div>
      </div>
    </Palette>
  );
}
