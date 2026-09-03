import { caller } from "@/lib/caller";
import { listRules } from "@/wire/client";
import { TransferForm } from "./TransferForm";

export const dynamic = "force-dynamic";

/**
 * Move value between personal accounts without claiming lot relief.
 *
 * ⭐ THE WRITE IS ApplyEvent WITH NO INSTRUMENT AND NO QUANTITY. The
 * household template's `xfer_*` rules have no per-instrument leg, so the
 * walk that opens lots skips them. Cash → investments is a transfer, not a
 * sale. The date is required: an undated entry is in no period P&L.
 */
export default async function Transfer({
  params,
}: {
  params: Promise<{ book: string }>;
}) {
  const { book } = await params;
  const c = await caller();
  const { rules } = await listRules(c, book);
  const xfer = rules.filter((r) => r.ruleId.startsWith("xfer_"));

  return <TransferForm fund={book} rules={xfer} />;
}
