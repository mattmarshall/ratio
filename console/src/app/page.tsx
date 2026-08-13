import { redirect } from "next/navigation";
import { caller } from "@/lib/caller";
import { listFunds } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * The front door.
 *
 * ⭐ THE BLOCKED FUND FIRST, NOT THE ALPHABETICALLY FIRST ONE. An operator
 * opening this at eight in the morning wants the fund that cannot strike, and
 * the old console picked it for the same reason. What is new is that the answer
 * is a redirect to a URL rather than a piece of component state — so the fund
 * they land on is one they can then send to somebody.
 */
export default async function Home() {
  const c = await caller();
  const { funds } = await listFunds(c);
  if (!funds.length) redirect("/funds");
  const first = funds.find((f) => f.state === "BLOCKED") ?? funds[0]!;
  redirect(`/${first.name}/breaks`);
}
