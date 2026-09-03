import { redirect } from "next/navigation";
import { caller } from "@/lib/caller";
import { listBooks } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * The front door.
 *
 * Lands on a book, not a fund. A personal or project book has no
 * `/funds/{fund}/…` URL it must travel through. Seeded fund books still
 * carry a default view, so the demo path is unchanged in substance.
 */
export default async function Home() {
  const c = await caller();
  const { books } = await listBooks(c);
  if (!books.length) redirect("/books");
  const first = books[0]!;
  const id = first.name.replace(/^books\//, "");
  redirect(`/books/${id}`);
}
