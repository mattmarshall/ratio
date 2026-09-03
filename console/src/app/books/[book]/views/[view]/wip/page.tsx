import Link from "next/link";
import { caller } from "@/lib/caller";
import {
  debitShown,
  isWip,
  ofType,
  projectRollup,
  wipFoots,
} from "@/lib/project";
import { listAccounts } from "@/wire/client";
import { withRefusal } from "@/components/Refusal";

export const dynamic = "force-dynamic";

/**
 * WIP capitalization: cost → WIP → recognized.
 *
 * ⭐ THE SAME ACCOUNTS AS THE TRIAL BALANCE. `capitalize_wip` debits Work in
 * progress and credits Project costs; `recognize_wip` reverses it. Both
 * conserve. Currently capitalized plus recognized equals what moved into WIP.
 *
 * Cost incurred is uncapitalized plus WIP — not costs.debit, which would
 * double-count after recognition.
 */
async function Wip({
  params,
}: {
  params: Promise<{ book: string; view: string }>;
}) {
  const { book, view } = await params;
  const c = await caller();
  const { accounts } = await listAccounts(c, book, view);
  const r = projectRollup(accounts, "");
  const costs = ofType(accounts, "EXPENSE");
  const wipAccounts = ofType(accounts, "ASSET").filter(isWip);
  const foots = wipFoots(r);

  return (
    <>
      <div className="tb" role="table" aria-label="WIP capitalization">
        <div className="posgroup">
          <div className="posacct">Project costs</div>
          {costs.length === 0 ? (
            <div className="tbrow static" role="row">
              <span role="cell">None in this chart.</span>
              <span role="cell" className="num">
                {debitShown(0n)}
              </span>
            </div>
          ) : (
            costs.map((a) => {
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
            })
          )}
          <div className="tbfoot static" role="row">
            <span role="cell">
              In expense
              <small>uncapitalized, or recognized back from WIP</small>
            </span>
            <span role="cell" className="num">
              {debitShown(r.costs)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="posacct">Work in progress</div>
          {wipAccounts.length === 0 ? (
            <div className="tbrow static" role="row">
              <span role="cell">No WIP account in this chart.</span>
              <span role="cell" className="num">
                {debitShown(0n)}
              </span>
            </div>
          ) : (
            wipAccounts.map((a) => {
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
            })
          )}
          <div className="tbrow static" role="row">
            <span role="cell">Currently capitalized</span>
            <span role="cell" className="num">
              {debitShown(r.wip)}
            </span>
          </div>
          <div className="tbrow static" role="row">
            <span role="cell">Recognized (out of WIP)</span>
            <span role="cell" className="num">
              {debitShown(r.wipCredit)}
            </span>
          </div>
          <div className="tbfoot static" role="row">
            <span role="cell">
              Moved into WIP
              <small>
                {foots
                  ? "currently capitalized plus recognized"
                  : "does not foot — debit does not equal remaining plus recognized"}
              </small>
            </span>
            <span role="cell" className="num">
              {debitShown(r.wipDebit)}
            </span>
          </div>
        </div>
        <div className="posgroup">
          <div className="tbfoot static" role="row">
            <span role="cell">
              Cost incurred
              <small>uncapitalized plus currently in WIP — not a second ledger</small>
            </span>
            <span role="cell" className="num">
              {debitShown(r.incurred)}
            </span>
          </div>
        </div>
      </div>
      <p className="note">
        <Link href={`/books/${book}/views/${view}/budget`}>Budget vs actual</Link>
        {" · "}
        <Link href={`/books/${book}/record`}>Capitalize or recognize WIP</Link>
        {" · "}
        <Link href={`/books/${book}/views/${view}/accounts`}>Trial balance</Link>
      </p>
    </>
  );
}

export default withRefusal(Wip);
