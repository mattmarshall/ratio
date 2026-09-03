import { redirect } from "next/navigation";
import { caller } from "@/lib/caller";
import { getFund } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * Where `/books/<book>/breaks` used to be.
 *
 * ⛔ A REDIRECT RATHER THAN A DELETION, BECAUSE THESE URLS HAVE BEEN SENT TO
 * PEOPLE. The whole argument for this console is that a figure can be sent
 * rather than described — a link that 404s a month later argues against the
 * product more effectively than the screen argues for it.
 *
 * ⚠ IT CANNOT BE A `next.config.ts` REDIRECT: the destination depends on the
 * fund's default view, which is a value only the API knows.
 */
export default async function Legacybreaks({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  const c = await caller();
  const f = await getFund(c, fund);
  redirect(`/books/${fund}/views/${f.defaultView}/breaks`);
}
