import Link from "next/link";
import type { ReactNode } from "react";
import { Brand } from "@/components/Brand";
import { CommandHint } from "@/components/CommandHint";
import { Palette } from "@/components/Palette";
import { Who } from "@/components/Who";
import { books as booksForRequest, funds as fundsForRequest } from "@/lib/data";

export const dynamic = "force-dynamic";

/**
 * Chrome for the project collection.
 *
 * ⭐ A PROJECT IS A BOOK. The jobs stay at `/books/{id}/…`; this layout
 * only lists them. No FundRail — a project book has no fund to rail.
 */
export default async function ProjectsLayout({
  children,
}: {
  children: ReactNode;
}) {
  const books = await booksForRequest();
  const funds = await fundsForRequest();
  const projects = books.filter((b) => b.kind === "PROJECT");

  return (
    <Palette funds={funds}>
      <div className="app">
        <header className="top">
          <Link href="/projects" className="brandlink" aria-label="Your projects">
            <Brand />
          </Link>
          <span className="crumb">
            Projects <span aria-hidden="true">/</span> <b>{projects.length}</b>
          </span>
          <span className="spacer" />
          <Link href="/books" className="crumb">
            Books
          </Link>
          <Link href="/funds" className="crumb">
            Funds
          </Link>
          <CommandHint />
          <Who />
        </header>
        <div className="body norail">{children}</div>
      </div>
    </Palette>
  );
}
