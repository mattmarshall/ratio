import { caller } from "@/lib/caller";
import { listTemplates } from "@/wire/client";
import { IngestForm } from "./IngestForm";

export const dynamic = "force-dynamic";

/** Read a delivered file into facts, and admit the ones that resolve. */
export default async function Ingest({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { templates } = await listTemplates(c, fund);

  return <IngestForm fund={fund} templates={templates} />;
}
