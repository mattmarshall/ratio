import { caller } from "@/lib/caller";
import { listRules } from "@/wire/client";
import { RecordForm } from "./RecordForm";

export const dynamic = "force-dynamic";

/** Record an event against one of this fund's approved rules. */
export default async function Record({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { rules } = await listRules(c, fund);

  return <RecordForm fund={fund} rules={rules} />;
}
