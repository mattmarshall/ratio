import { caller } from "@/lib/caller";
import { listTemplates } from "@/wire/client";
import { IngestForm } from "./IngestForm";

export const dynamic = "force-dynamic";

/** Read a delivered file into facts, and admit the ones that resolve. */
export default async function Ingest({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book: fund } = await params;
  const c = await caller();
  // The book's configuration is already kind-aware (CreateBook writes one
  // mapping per kind). The mock subsets the mixed fixture the same way.
  const { templates } = await listTemplates(c, fund);

  return <IngestForm fund={fund} templates={templates} />;
}
