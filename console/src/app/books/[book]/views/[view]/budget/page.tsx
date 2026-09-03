import Link from "next/link";
import { caller } from "@/lib/caller";
import { money } from "@/lib/format";
import {
  creditShown,
  debitShown,
  ofType,
  projectRollup,
} from "@/lib/project";
import { getBook, listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * Budget vs actual for one project book of record.
 *
 * ⭐ NOT NAV RELABELLED. Baseline is `[project] budget` on the configuration
 * this book pins. Actuals are the trial balance: costs + WIP incurred,
 * payables as commitment, funding and revenue as the other roll-up. A
 * second ledger would be a second answer to a question the journal already
 * answers.
 *
 * ⚠ CUMULATIVE ON PURPOSE. A project's period is the project, not a calendar
 * month. Milestone-gated close is still out of scope — same period gap as
 * #26, named rather than faked with a NAV strike.
 */
async function Budget({
  params,
}: {
  params: Promise<{ book: string; view: string }>;
}) {
  const { book, view } = await params;
  const c = await caller();
  const b = await getBook(c, book);
  const { accounts } = await listAccounts(c, book, view);
  const r = projectRollup(accounts, b.budget);
  const costs = ofType(accounts, "EXPENSE");
  const wip = ofType(accounts, "ASSET").filter((a) =>
    /work in progress/i.test(a.displayName),
  );
  const payables = ofType(accounts, "LIABILITY");
  const funding = ofType(accounts, "EQUITY");

  return (
    <>
      <div className="tb" role="table" aria-label="Budget vs actual">
        <div className="posgroup">
          <div className="posacct">Baseline</div>
          <div className="tbrow static" role="row">
            <span role="cell">
              Authorized budget
              <span className="at">
                not a second ledger — journal costs, WIP and payables
                against a configuration total
              </span>
            </span>
            <span role="cell" className="num">
              {r.baseline === null ? "—" : debitShown(r.baseline)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Actual</div>
          {costs.map((a) => {
            const id = a.name.split("/").pop()!;
            return (
              <Link
                key={a.name}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${id}`}
              >
                <span role="cell">{a.displayName}</span>
                <span role="cell" className="num">
                  {debitShown(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          {wip.map((a) => {
            const id = a.name.split("/").pop()!;
            return (
              <Link
                key={a.name}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${id}`}
              >
                <span role="cell">{a.displayName}</span>
                <span role="cell" className="num">
                  {debitShown(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Incurred
              <small>costs plus WIP — recognizing does not double-count</small>
            </span>
            <span role="cell" className="num">
              {debitShown(r.incurred)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Commitment</div>
          {payables.map((a) => {
            const id = a.name.split("/").pop()!;
            return (
              <Link
                key={a.name}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${id}`}
              >
                <span role="cell">{a.displayName}</span>
                <span role="cell" className="num">
                  {creditShown(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">
              Committed
              <small>incurred plus unpaid payables</small>
            </span>
            <span role="cell" className="num">
              {debitShown(r.committed)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              Variance
              <small>
                {r.baseline === null
                  ? "no [project] budget on the configuration in force"
                  : "baseline minus committed — remaining authorization"}
              </small>
            </span>
            <span role="cell" className="num">
              {r.variance === null ? "—" : debitShown(r.variance)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Funding</div>
          {funding.map((a) => {
            const id = a.name.split("/").pop()!;
            return (
              <Link
                key={a.name}
                className="tbrow"
                role="row"
                href={`/books/${book}/views/${view}/accounts/${id}`}
              >
                <span role="cell">{a.displayName}</span>
                <span role="cell" className="num">
                  {creditShown(BigInt(a.balance))}
                </span>
              </Link>
            );
          })}
          <div className="tbfoot static" role="row">
            <span role="cell">Funding received</span>
            <span role="cell" className="num">
              {creditShown(r.funding)}
            </span>
          </div>
        </div>
      </div>
      <p className="note">
        <Link href={`/books/${book}/views/${view}/wip`}>WIP capitalization</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Record a cost or capitalize WIP</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Budget);
