import Link from "next/link";
import { caller } from "@/lib/caller";
import { creditShown, debitShown, figure } from "@/lib/project";
import { projectProgress } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Billed vs earned, retainage outstanding, and cost by work-package account.
 *
 * ⭐ NOT A SECOND LEDGER. Billed is the Progress billings credit; earned is
 * Project revenue's credit; retainage is the hold accounts. Cost by phase
 * is the expense accounts `chart_for(Project)` seeded — site, structure,
 * finishes — not Positions wearing a project label.
 *
 * ⛔ UNSET STAYS UNSET. A book that has never billed does not show billed
 * 0.00. A phase with no `[project.phase] budget` shows — not a fake
 * authorization of zero. A seeded phase with no postings shows cost 0.00,
 * which is a true zero.
 *
 * ⚠ CUMULATIVE ON PURPOSE. Milestone-gated close is still out of scope —
 * same period gap as #26. Client portal / CRM / Gantt are refused.
 */
async function Billing({
  params,
}: {
  params: Promise<{ book: string; view: string }>;
}) {
  const { book, view } = await params;
  const c = await caller();
  const p = await projectProgress(c, book, view);

  return (
    <>
      <div className="tb" role="table" aria-label="Progress billing">
        <div className="posgroup">
          <div className="posacct">Billed vs earned</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Billed to date
              <span className="at">
                Progress billings credit — empty until a progress bill posts
              </span>
            </span>
            <span role="cell" className="num">
              {creditShown(p.billed)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Earned to date
              <span className="at">
                Project revenue credit — independent of billings, so the two
                can diverge
              </span>
            </span>
            <span role="cell" className="num">
              {creditShown(p.earned)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Billed minus earned
              <small>
                {p.billedMinusEarned === ""
                  ? "unset until both billed and earned have posted — not a fake caught-up zero"
                  : "overbilling when positive; underbilling when negative"}
              </small>
            </span>
            <span role="cell" className="num">
              {figure(p.billedMinusEarned)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Retainage outstanding</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Receivable
              <span className="at">held from a progress bill until a milestone clears</span>
            </span>
            <span role="cell" className="num">
              {debitShown(p.retainageReceivable)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Payable
              <span className="at">held from a vendor invoice — a different account</span>
            </span>
            <span role="cell" className="num">
              {creditShown(p.retainagePayable)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Cost by work package</div>
          {p.phases.map((ph) => {
            return (
              <Link
                key={ph.account}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${ph.account}`}
              >
                <span role="cell">
                  {ph.displayName}
                  <span className="at">
                    {ph.budget === ""
                      ? "budget unset — not a silent zero"
                      : `authorized ${debitShown(ph.budget)}`}
                  </span>
                </span>
                <span role="cell" className="num">
                  {debitShown(ph.cost)}
                </span>
              </Link>
            );
          })}
        </div>
      </div>
      <p className="note">
        <Link href={`/books/${book}/record`}>Record a bill, retainage hold, or cost</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Billing);
