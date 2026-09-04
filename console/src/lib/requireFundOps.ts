import "server-only";

import { notFound } from "next/navigation";
import { caller } from "./caller";
import { or404 } from "./or404";
import { wearsFundOps } from "./screens";
import { getBook } from "@/wire/client";
import type { Book } from "@/wire/types";

/**
 * Fund-ops chrome (Exceptions / Positions / NAV / trade / mark /
 * corporate actions / dual-view recon) is Investment plus the
 * unspecified operations surface. Personal / Project / Operating
 * 404 rather than wear a fake label on fund screens (#175).
 */
export async function requireFundOps(book: string): Promise<Book> {
  const c = await caller();
  const b = await or404(getBook(c, book));
  if (!wearsFundOps(b.kind)) notFound();
  return b;
}
