import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { WORKSPACE_COOKIE, workspaceHome } from "@/lib/workspace";

export const dynamic = "force-dynamic";

/**
 * After sign-in, the chosen collection.
 *
 * ⭐ A COOKIE, DEFAULT `/books`. Kind is a template on CreateBook, not a
 * reason to invent a second ledger. An operator who lives in fund admin
 * or project finance sets that on the collection chrome.
 */
export default async function Home() {
  const raw = (await cookies()).get(WORKSPACE_COOKIE)?.value;
  redirect(workspaceHome(raw));
}
