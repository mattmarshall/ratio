import Link from "next/link";
import { caller } from "@/lib/caller";
import { listChangeLogEntries, listRules } from "@/wire/client";

export const dynamic = "force-dynamic";

/**
 * The rules this fund posts under, and the drafts nobody has approved.
 *
 * ⛔ TWO LISTS, AND THEY DO NOT MERGE. The gap between them is exactly what a
 * person's approval bought, and a screen that showed one list would erase it.
 *
 * ⛔ AND THERE IS NO APPROVE BUTTON HERE. `approve_rule` is deliberately absent
 * from the model's tool list — the fence is enforced by the tool not existing,
 * not by a permission check — and a console offering a second way round it
 * would make the fence worth nothing. Approval is `ratio approve` at a
 * terminal, by a person.
 *
 * ⚠ WHERE THE DRAFTS COME FROM IS NOT OBVIOUS. `ratio.console.v1.Rule` carries
 * no proposal state at all; `ListRules` returns the ACTIVE set. Unapproved
 * drafts reach this contract as change-log entries — `ratio-console` emits them
 * with `configDigest: "proposal"` and `actorKind: MODEL`, and drops any already
 * approved. So the split below needs no contract change. What it cannot show is
 * a draft's rendered FORM and its check findings: those exist only in the
 * legacy `/rules.json` screen, which reads a different book from this one. That
 * is a contract change, and deliberately not made here.
 */
export default async function Rules({
  params,
}: {
  params: Promise<{ fund: string }>;
}) {
  const { fund } = await params;
  const c = await caller();
  const { rules } = await listRules(c, fund);
  const { changeLogEntries } = await listChangeLogEntries(c, fund);
  const proposals = changeLogEntries.filter((e) => e.configDigest === "proposal");

  return (
    <>
      <section className="log" aria-label="Active rules">
        <div className="loghead">
          <span>Active</span>
          <span className="sortnote">
            {rules.length ? "approved, and posting" : ""}
          </span>
        </div>
        {rules.length === 0 ? (
          <div className="empty">This book has no approved rules.</div>
        ) : null}
        {rules.map((r) => {
          const id = r.name.split("/").pop()!;
          return (
            <Link className="logrow" key={r.name} href={`/funds/${fund}/rules/${id}`}>
              <span className="t num">{r.ruleId}</span>
              <span className="w">
                <b>{r.description || r.ruleId}</b>
                <div className="cfg">
                  {r.kind.toLowerCase()} · {r.accounts.join(" → ")}
                </div>
              </span>
            </Link>
          );
        })}
      </section>

      <section className="log" aria-label="Unapproved proposals">
        <div className="loghead">
          <span>Awaiting a person</span>
          <span className="sortnote">
            {proposals.length ? "drafted, not approved" : ""}
          </span>
        </div>
        {proposals.length === 0 ? (
          <div className="empty">
            No drafts are waiting. A model proposes; a person runs{" "}
            <code>ratio approve</code>.
          </div>
        ) : null}
        {proposals.map((p) => (
          <div className="logrow pend" key={p.name}>
            <span className="sev med" />
            <span className="w">
              <b>{p.subject}</b> <span className="tagx">drafted</span>
              <div className="why">{p.detail}</div>
              <div className="p2 num">
                by {p.actor} · a model · approve with{" "}
                <code>ratio approve {p.subject}</code>
              </div>
            </span>
          </div>
        ))}
      </section>
    </>
  );
}
