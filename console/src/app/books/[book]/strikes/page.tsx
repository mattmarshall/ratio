import { redirect } from "next/navigation";
import { requireFundOps } from "@/lib/requireFundOps";

export const dynamic = "force-dynamic";

/**
 * Where `/books/<book>/strikes` used to be.
 *
 * ⛔ A REDIRECT RATHER THAN A DELETION, BECAUSE THESE URLS HAVE BEEN SENT TO
 * PEOPLE. The whole argument for this console is that a figure can be sent
 * rather than described — a link that 404s a month later argues against the
 * product more effectively than the screen argues for it.
 *
 * ⚠ IT CANNOT BE A `next.config.ts` REDIRECT: the destination depends on the
 * book's default view, which is a value only the API knows.
 *
 * Personal / Project / Operating 404 — leftover fund-ops chrome (#175).
 */
export default async function Legacystrikes({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  const b = await requireFundOps(fund);
  redirect(`/books/${fund}/views/${b.defaultView}/strikes`);
}
