import Link from "next/link";
import { caller } from "@/lib/caller";
import {
  accountsReceivable,
  collectedAgainstBilled,
  creditShown,
  debitShown,
  figure,
  outstandingAgainstBilled,
  phaseApproved,
  projectRollup,
  remainingToBill,
} from "@/lib/project";
import { getBook, listAccounts, listRules, projectProgress } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";
import { BillingPostForm } from "./BillingPostForm";

export const dynamic = "force-dynamic";

/**
 * Billed vs earned, retainage outstanding, and cost by work-package account.
 *
 * ⭐ NOT A SECOND LEDGER. Billed is the Progress billings credit; earned is
 * Project revenue's credit; retainage is the hold accounts. Cost by phase
 * is the expense accounts `chart_for(Project)` seeded — site, structure,
 * finishes — not Positions wearing a project label.
 *
 * ⭐ THE BILLING BASIS IS THE REVISED CONTRACT. Original is `[project]
 * budget`. Approved change orders are the work-package equity pair. Revised
 * = original + approved when the original is set. Remaining to bill is
 * revised − billed. Collections vs billed is cash against AR. Mutating
 * the baseline would lose the audit trail this page exists to cite.
 *
 * ⛔ UNSET STAYS UNSET. A book that has never billed does not show billed
 * 0.00, remaining-to-bill equal to the whole contract, or collected 0.00.
 * A phase with no `[project.phase] budget` shows — not a fake
 * authorization of zero. A seeded phase with no postings shows cost 0.00,
 * which is a true zero. An unposted change order is — not a silent zero
 * against that phase. Billed but uncollected is a real zero collected.
 *
 * ⭐ A PROJECT POSTS A COLLECTION HERE. `collect_receivable` is already
 * in force. Cash against AR moves collections vs billed after it posts.
 * Facts stay unset until billed and receivable can support the cut —
 * not a silent zero collected. Stripe / ACH stay Connect.
 *
 * ⚠ CUMULATIVE ON PURPOSE. Milestone-gated close is still out of scope —
 * same period gap as #26. Client portal / CRM / Gantt / AIA G702 product
 * UI are refused. Payment-processor settlement is a Connect app.
 */
async function Billing({
  params,
}: {
  params: Promise<{ book: string; view: string }>;
}) {
  const { book, view } = await params;
  const c = await caller();
  const b = await getBook(c, book);
  const p = await projectProgress(c, book, view);
  const { accounts } = await listAccounts(c, book, view);
  const rules =
    b.kind === "PROJECT" ? (await listRules(c, book)).rules : [];
  const contract = projectRollup(accounts, b.budget);
  const remaining = remainingToBill(contract.revised, p.billed);
  const ar = accountsReceivable(accounts);
  const collected = collectedAgainstBilled(p.billed, ar, p.retainageReceivable);
  const outstanding = outstandingAgainstBilled(p.billed, ar, p.retainageReceivable);

  return (
    <>
      <div className="tb" role="table" aria-label="Progress billing">
        <div className="posgroup">
          <div className="posacct">Contract</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Original contract
              <span className="at">
                [project] budget — the baseline a change order must not rewrite
              </span>
            </span>
            <span role="cell" className="num">
              {contract.baseline === null ? "—" : debitShown(contract.baseline)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Approved change orders
              <span className="at">
                {contract.approved === null
                  ? "unset — no approved change order has posted, not a silent zero"
                  : "work-package grain, same accounts cost-by-phase uses"}
              </span>
            </span>
            <span role="cell" className="num">
              {contract.approved === null ? "—" : debitShown(contract.approved)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Billing basis
              <small>
                {contract.revised === null
                  ? contract.approved === null
                    ? "unset until [project] budget is set — not a priced zero"
                    : "cannot price an unknown baseline"
                  : contract.approved === null
                    ? "equals the original — no approved change order has posted"
                    : "revised contract when priced — original plus approved changes"}
              </small>
            </span>
            <span role="cell" className="num">
              {contract.revised === null ? "—" : debitShown(contract.revised)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Remaining to bill
              <small>
                {remaining === null
                  ? contract.revised === null
                    ? "unset until [project] budget is set — not a priced remainder"
                    : "unset until a progress bill posts — not the whole contract as a fake remainder"
                  : "revised minus billed — the citeable leftover, not a spreadsheet rebuild"}
              </small>
            </span>
            <span role="cell" className="num">
              {remaining === null ? "—" : debitShown(remaining)}
            </span>
          </div>
        </div>
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
          <div className="posacct">Collections vs billed</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Collected
              <span className="at">
                {collected === null
                  ? p.billed === ""
                    ? "unset until a progress bill posts — not a fake zero collected"
                    : "unset — accounts receivable has not posted, so cash against AR cannot be cited"
                  : "cash against AR — billed minus outstanding receivable and retainage held"}
              </span>
            </span>
            <span role="cell" className="num">
              {collected === null ? "—" : debitShown(collected)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Outstanding receivable
              <small>
                {outstanding === null
                  ? "unset until billed and accounts receivable can support the cut"
                  : "AR plus retainage receivable — the uncollected billed"}
              </small>
            </span>
            <span role="cell" className="num">
              {outstanding === null ? "—" : debitShown(outstanding)}
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
            const approved = phaseApproved(accounts, ph.displayName);
            const original = ph.budget === "" ? null : BigInt(ph.budget);
            const revised =
              original === null ? null : original + (approved ?? 0n);
            const auth =
              original === null
                ? approved === null
                  ? "budget unset — not a silent zero"
                  : `budget unset — approved changes ${debitShown(approved)} (cannot revise an unknown baseline)`
                : approved === null
                  ? `authorized ${debitShown(original)} — no approved change order`
                  : `original ${debitShown(original)} · approved changes ${debitShown(approved)} · revised ${debitShown(revised!)}`;
            return (
              <Link
                key={ph.account}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${ph.account}`}
              >
                <span role="cell">
                  {ph.displayName}
                  <span className="at">{auth}</span>
                </span>
                <span role="cell" className="num">
                  {debitShown(ph.cost)}
                </span>
              </Link>
            );
          })}
        </div>
      </div>
      {b.kind === "PROJECT" ? (
        <>
          <p className="note">
            A collection stays unset on this page until billed and
            accounts receivable can support the cut — not a silent zero
            collected. The same <code>collect_receivable</code>{" "}
            <code>/record</code> already uses; this is not a payment
            processor.
          </p>
          <BillingPostForm fund={book} rules={rules} />
        </>
      ) : null}
      <p className="note">
        <Link href={`/books/${book}/record`}>Record a bill, retainage hold, cost, or change order</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/budget`}>Remaining to spend and original vs revised contract</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Billing);
